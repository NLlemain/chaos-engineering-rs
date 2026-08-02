use crate::{
    error::{ChaosError, Result},
    handle::InjectionHandle,
    injectors::{Injector, InjectorStatus},
    target::Target,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body, Bytes},
    extract::State,
    http::{header, HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode},
    routing::any,
    Router,
};
use futures::stream;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    Generic,
    OpenAi,
    AzureOpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
    Ollama,
    Mistral,
    Groq,
    Cohere,
    Together,
    Vllm,
}

impl std::str::FromStr for AiProvider {
    type Err = ChaosError;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().replace('-', "_").as_str() {
            "generic" => Ok(Self::Generic),
            "openai" | "open_ai" => Ok(Self::OpenAi),
            "azure" | "azure_openai" => Ok(Self::AzureOpenAi),
            "anthropic" | "claude" => Ok(Self::Anthropic),
            "gemini" | "google" => Ok(Self::Gemini),
            "openrouter" | "open_router" => Ok(Self::OpenRouter),
            "ollama" => Ok(Self::Ollama),
            "mistral" => Ok(Self::Mistral),
            "groq" => Ok(Self::Groq),
            "cohere" => Ok(Self::Cohere),
            "together" => Ok(Self::Together),
            "vllm" => Ok(Self::Vllm),
            _ => Err(ChaosError::InvalidConfig(format!(
                "Unknown AI provider '{}'",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HttpFaultType {
    Status { code: u16, body: String },
    Latency { delay: Duration },
    StripHeaders { headers: Vec<String> },
    Slowloris { chunk_delay: Duration },
    TruncateBody { bytes: usize },
    ReplaceBody { body: String, content_type: String },
    MalformedJson,
    MalformedHeaders,
    EmptyResponse,
    StreamDelay { chunk_delay: Duration },
    StreamAbort { after_events: usize },
    MalformedToolCall,
    ContextTruncate { keep_last_items: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpFaultConfig {
    pub listen: SocketAddr,
    pub upstream_url: String,
    pub path_pattern: String,
    pub provider: AiProvider,
    pub faults: Vec<HttpFaultType>,
    pub rate: f64,
}

impl Default for HttpFaultConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:0".parse().expect("valid loopback address"),
            upstream_url: "http://127.0.0.1:1".to_string(),
            path_pattern: "/*".to_string(),
            provider: AiProvider::Generic,
            faults: vec![HttpFaultType::Status {
                code: 503,
                body: String::new(),
            }],
            rate: 1.0,
        }
    }
}

impl HttpFaultConfig {
    pub fn validate(&self) -> Result<()> {
        reqwest::Url::parse(&self.upstream_url).map_err(|error| {
            ChaosError::InvalidConfig(format!("Invalid upstream URL: {}", error))
        })?;
        if self.faults.is_empty() {
            return Err(ChaosError::InvalidConfig(
                "HTTP fault proxy requires at least one fault".to_string(),
            ));
        }
        if !self.rate.is_finite() || !(0.0..=1.0).contains(&self.rate) {
            return Err(ChaosError::InvalidConfig(
                "HTTP fault rate must be between 0.0 and 1.0".to_string(),
            ));
        }
        for fault in &self.faults {
            match fault {
                HttpFaultType::Status { code, .. } if !(100..=599).contains(code) => {
                    return Err(ChaosError::InvalidConfig(format!(
                        "Invalid HTTP status code {}",
                        code
                    )));
                }
                HttpFaultType::StreamAbort { after_events: 0 } => {
                    return Err(ChaosError::InvalidConfig(
                        "Stream abort must allow at least one event".to_string(),
                    ));
                }
                HttpFaultType::ContextTruncate { keep_last_items: 0 } => {
                    return Err(ChaosError::InvalidConfig(
                        "Context truncation must retain at least one item".to_string(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpFaultMetrics {
    pub requests: u64,
    pub injected_requests: u64,
    pub upstream_errors: u64,
    pub stream_events_dropped: u64,
    pub contexts_truncated: u64,
}

#[derive(Default)]
struct HttpMetricsState {
    requests: AtomicU64,
    injected_requests: AtomicU64,
    upstream_errors: AtomicU64,
    stream_events_dropped: AtomicU64,
    contexts_truncated: AtomicU64,
}

impl HttpMetricsState {
    fn snapshot(&self) -> HttpFaultMetrics {
        HttpFaultMetrics {
            requests: self.requests.load(Ordering::Relaxed),
            injected_requests: self.injected_requests.load(Ordering::Relaxed),
            upstream_errors: self.upstream_errors.load(Ordering::Relaxed),
            stream_events_dropped: self.stream_events_dropped.load(Ordering::Relaxed),
            contexts_truncated: self.contexts_truncated.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone)]
struct HttpProxyState {
    config: HttpFaultConfig,
    client: reqwest::Client,
    metrics: Arc<HttpMetricsState>,
}

struct HttpFaultServer {
    listen: SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
    metrics: Arc<HttpMetricsState>,
}

impl HttpFaultServer {
    async fn start(config: HttpFaultConfig) -> Result<Self> {
        config.validate()?;
        let listener = TcpListener::bind(config.listen).await?;
        let listen = listener.local_addr()?;
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let metrics = Arc::new(HttpMetricsState::default());
        let state = HttpProxyState {
            config,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|error| ChaosError::NetworkError(error.to_string()))?,
            metrics: metrics.clone(),
        };
        let router = Router::new().fallback(any(proxy_request)).with_state(state);
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
            {
                debug!("HTTP fault proxy stopped with error: {}", error);
            }
        });

        Ok(Self {
            listen,
            cancellation,
            task,
            metrics,
        })
    }

    async fn shutdown(self) -> Result<HttpFaultMetrics> {
        self.cancellation.cancel();
        self.task.await.map_err(|error| {
            ChaosError::CleanupFailed(format!("HTTP proxy task failed: {}", error))
        })?;
        Ok(self.metrics.snapshot())
    }
}

async fn proxy_request(
    State(state): State<HttpProxyState>,
    request: Request<Body>,
) -> Response<Body> {
    state.metrics.requests.fetch_add(1, Ordering::Relaxed);
    let (parts, body) = request.into_parts();
    let should_inject =
        path_matches(&state.config.path_pattern, parts.uri.path()) && selected(state.config.rate);
    if should_inject {
        state
            .metrics
            .injected_requests
            .fetch_add(1, Ordering::Relaxed);
    }

    if should_inject {
        if let Some(HttpFaultType::Status { code, body }) = state
            .config
            .faults
            .iter()
            .find(|fault| matches!(fault, HttpFaultType::Status { .. }))
        {
            return synthetic_error(state.config.provider, *code, body);
        }
        for fault in &state.config.faults {
            if let HttpFaultType::Latency { delay } = fault {
                tokio::time::sleep(*delay).await;
            }
        }
    }

    let mut request_body = match to_bytes(body, MAX_REQUEST_BYTES).await {
        Ok(bytes) => bytes.to_vec(),
        Err(error) => return text_response(413, format!("Request body error: {}", error)),
    };
    if should_inject {
        for fault in &state.config.faults {
            if let HttpFaultType::ContextTruncate { keep_last_items } = fault {
                if truncate_context(&mut request_body, *keep_last_items) {
                    state
                        .metrics
                        .contexts_truncated
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    let upstream_url = join_upstream_url(&state.config.upstream_url, &parts.uri);
    let method = match reqwest::Method::from_bytes(parts.method.as_str().as_bytes()) {
        Ok(method) => method,
        Err(error) => return text_response(400, error.to_string()),
    };
    let mut upstream = state.client.request(method, &upstream_url);
    for (name, value) in &parts.headers {
        if name == header::HOST || name == header::CONTENT_LENGTH {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            upstream = upstream.header(name, value);
        }
    }

    let upstream_response = match upstream.body(request_body).send().await {
        Ok(response) => response,
        Err(error) => {
            state
                .metrics
                .upstream_errors
                .fetch_add(1, Ordering::Relaxed);
            return synthetic_error(
                state.config.provider,
                502,
                &format!("Upstream request failed: {}", error),
            );
        }
    };

    let status = upstream_response.status().as_u16();
    let mut headers = convert_headers(upstream_response.headers());
    let mut response_body = match upstream_response.bytes().await {
        Ok(bytes) => bytes.to_vec(),
        Err(error) => {
            state
                .metrics
                .upstream_errors
                .fetch_add(1, Ordering::Relaxed);
            return synthetic_error(
                state.config.provider,
                502,
                &format!("Upstream body failed: {}", error),
            );
        }
    };

    if should_inject {
        apply_header_faults(&state.config.faults, &mut headers);
        for fault in &state.config.faults {
            match fault {
                HttpFaultType::TruncateBody { bytes } => {
                    response_body.truncate(*bytes);
                }
                HttpFaultType::ReplaceBody { body, content_type } => {
                    response_body = body.as_bytes().to_vec();
                    if let Ok(value) = HeaderValue::from_str(content_type) {
                        headers.insert(header::CONTENT_TYPE, value);
                    }
                    headers.remove(header::CONTENT_LENGTH);
                }
                HttpFaultType::MalformedJson => response_body = b"{\"chaos\":".to_vec(),
                HttpFaultType::EmptyResponse => response_body.clear(),
                HttpFaultType::MalformedToolCall => {
                    response_body = malformed_tool_call(state.config.provider);
                }
                _ => {}
            }
        }
    }

    let stream_delay = if should_inject {
        state.config.faults.iter().find_map(|fault| match fault {
            HttpFaultType::Slowloris { chunk_delay }
            | HttpFaultType::StreamDelay { chunk_delay } => Some(*chunk_delay),
            _ => None,
        })
    } else {
        None
    };
    let abort_after = if should_inject {
        state.config.faults.iter().find_map(|fault| match fault {
            HttpFaultType::StreamAbort { after_events } => Some(*after_events),
            _ => None,
        })
    } else {
        None
    };

    if stream_delay.is_some() || abort_after.is_some() {
        headers.remove(header::CONTENT_LENGTH);
        let frames = split_stream_frames(response_body, state.config.provider);
        let original_len = frames.len();
        let keep = abort_after.unwrap_or(original_len).min(original_len);
        if keep < original_len {
            state
                .metrics
                .stream_events_dropped
                .fetch_add((original_len - keep) as u64, Ordering::Relaxed);
        }
        let delay = stream_delay.unwrap_or_default();
        let output = stream::unfold((frames, 0usize), move |(frames, index)| async move {
            if index >= keep {
                return None;
            }
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            let bytes = Bytes::from(frames[index].clone());
            Some((Ok::<_, std::io::Error>(bytes), (frames, index + 1)))
        });
        return build_response(status, headers, Body::from_stream(output));
    }

    build_response(status, headers, Body::from(response_body))
}

fn join_upstream_url(base: &str, uri: &axum::http::Uri) -> String {
    format!(
        "{}{}",
        base.trim_end_matches('/'),
        uri.path_and_query().map_or("/", |value| value.as_str())
    )
}

fn path_matches(pattern: &str, path: &str) -> bool {
    pattern == "*"
        || pattern == "/*"
        || pattern == path
        || pattern
            .strip_suffix('*')
            .is_some_and(|prefix| path.starts_with(prefix))
}

fn selected(rate: f64) -> bool {
    rate >= 1.0 || (rate > 0.0 && rand::thread_rng().gen_bool(rate))
}

fn convert_headers(headers: &reqwest::header::HeaderMap) -> HeaderMap {
    let mut converted = HeaderMap::new();
    for (name, value) in headers {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            converted.append(name, value);
        }
    }
    converted
}

fn apply_header_faults(faults: &[HttpFaultType], headers: &mut HeaderMap) {
    for fault in faults {
        match fault {
            HttpFaultType::StripHeaders { headers: names } => {
                for name in names {
                    if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
                        headers.remove(name);
                    }
                }
            }
            HttpFaultType::MalformedHeaders => {
                headers.remove(header::CONTENT_TYPE);
                headers.insert(
                    HeaderName::from_static("x-chaos-content-type"),
                    HeaderValue::from_static("application/json; charset=definitely-not-real"),
                );
                headers.append(
                    header::RETRY_AFTER,
                    HeaderValue::from_static("not-a-duration"),
                );
            }
            _ => {}
        }
    }
}

fn truncate_context(body: &mut Vec<u8>, keep_last_items: usize) -> bool {
    let Ok(mut value) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    let mut changed = false;
    for key in ["messages", "contents", "input"] {
        if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
            if items.len() > keep_last_items {
                let remove = items.len() - keep_last_items;
                items.drain(..remove);
                changed = true;
            }
        }
    }
    if changed {
        *body = serde_json::to_vec(&value).expect("JSON value should serialize");
    }
    changed
}

fn split_stream_frames(body: Vec<u8>, provider: AiProvider) -> Vec<Vec<u8>> {
    let separator: &[u8] = if provider == AiProvider::Ollama {
        b"\n"
    } else {
        b"\n\n"
    };
    let mut frames = Vec::new();
    let mut start = 0usize;
    while let Some(offset) = find_bytes(&body[start..], separator) {
        let end = start + offset + separator.len();
        frames.push(body[start..end].to_vec());
        start = end;
    }
    if start < body.len() {
        frames.push(body[start..].to_vec());
    }
    if frames.len() <= 1 && body.len() > 64 {
        return body.chunks(64).map(<[u8]>::to_vec).collect();
    }
    if frames.is_empty() {
        frames.push(body);
    }
    frames
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn synthetic_error(provider: AiProvider, code: u16, custom_body: &str) -> Response<Body> {
    let body = if custom_body.is_empty() {
        provider_error(provider, code)
    } else {
        custom_body.as_bytes().to_vec()
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if code == 429 || code == 503 {
        headers.insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    }
    build_response(code, headers, Body::from(body))
}

fn provider_error(provider: AiProvider, code: u16) -> Vec<u8> {
    let message = match code {
        429 => "Chaos injected rate limit",
        503 => "Chaos injected provider overload",
        _ => "Chaos injected API failure",
    };
    let value = match provider {
        AiProvider::Anthropic => json!({
            "type": "error",
            "error": {"type": "rate_limit_error", "message": message}
        }),
        AiProvider::Gemini => json!({
            "error": {"code": code, "message": message, "status": "RESOURCE_EXHAUSTED"}
        }),
        AiProvider::OpenRouter => json!({
            "error": {
                "code": code,
                "message": message,
                "metadata": {"error_type": "rate_limit_exceeded"}
            }
        }),
        AiProvider::Ollama => json!({"error": message}),
        AiProvider::Cohere => json!({"message": message}),
        AiProvider::OpenAi
        | AiProvider::AzureOpenAi
        | AiProvider::Mistral
        | AiProvider::Groq
        | AiProvider::Together
        | AiProvider::Vllm => json!({
            "error": {
                "message": message,
                "type": "rate_limit_error",
                "param": null,
                "code": "rate_limit_exceeded"
            }
        }),
        AiProvider::Generic => json!({"error": {"code": code, "message": message}}),
    };
    serde_json::to_vec(&value).expect("provider error should serialize")
}

fn malformed_tool_call(provider: AiProvider) -> Vec<u8> {
    let value = match provider {
        AiProvider::Anthropic => json!({
            "content": [{"type": "tool_use", "name": "chaos_tool", "input": "{broken"}],
            "stop_reason": "tool_use"
        }),
        AiProvider::Gemini => json!({
            "candidates": [{"content": {"parts": [{"functionCall": {
                "name": "chaos_tool", "args": "{broken"
            }}]}}]
        }),
        AiProvider::Ollama => json!({
            "message": {"role": "assistant", "tool_calls": [{"function": {
                "name": "chaos_tool", "arguments": "{broken"
            }}]}, "done": true
        }),
        _ => json!({
            "choices": [{"message": {"role": "assistant", "tool_calls": [{
                "type": "function", "function": {"name": "chaos_tool", "arguments": "{broken"}
            }]}, "finish_reason": "tool_calls"}]
        }),
    };
    serde_json::to_vec(&value).expect("tool response should serialize")
}

fn text_response(code: u16, message: String) -> Response<Body> {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    build_response(code, headers, Body::from(message))
}

fn build_response(code: u16, headers: HeaderMap, body: Body) -> Response<Body> {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = Response::new(body);
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}

pub struct HttpFaultInjector {
    config: HttpFaultConfig,
    active: Arc<Mutex<HashMap<String, HttpFaultServer>>>,
}

impl HttpFaultInjector {
    pub fn new(config: HttpFaultConfig) -> Self {
        Self {
            config,
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn builder() -> HttpFaultBuilder {
        HttpFaultBuilder::default()
    }

    pub async fn metrics(&self, handle_id: &str) -> Option<HttpFaultMetrics> {
        self.active
            .lock()
            .await
            .get(handle_id)
            .map(|server| server.metrics.snapshot())
    }
}

impl Default for HttpFaultInjector {
    fn default() -> Self {
        Self::new(HttpFaultConfig::default())
    }
}

#[async_trait]
impl Injector for HttpFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let server = HttpFaultServer::start(self.config.clone()).await?;
        let metadata = json!({
            "listen": server.listen,
            "upstream_url": self.config.upstream_url,
            "provider": self.config.provider,
            "faults": self.config.faults,
            "path_pattern": self.config.path_pattern,
            "rate": self.config.rate,
            "rootless": true,
        });
        let handle = InjectionHandle::new("http_fault", target.clone(), metadata);
        self.active.lock().await.insert(handle.id.clone(), server);
        Ok(handle)
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let server = self.active.lock().await.remove(&handle.id);
        if let Some(server) = server {
            let metrics = server.shutdown().await?;
            info!("HTTP fault proxy {} stopped: {:?}", handle.id, metrics);
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "http_fault"
    }

    fn status(&self) -> InjectorStatus {
        InjectorStatus::Stable
    }

    async fn validate(&self) -> Result<()> {
        self.config.validate()
    }
}

#[derive(Default)]
pub struct HttpFaultBuilder {
    config: HttpFaultConfig,
}

impl HttpFaultBuilder {
    pub fn listen(mut self, listen: SocketAddr) -> Self {
        self.config.listen = listen;
        self
    }

    pub fn upstream(mut self, upstream: impl Into<String>) -> Self {
        self.config.upstream_url = upstream.into();
        self
    }

    pub fn provider(mut self, provider: AiProvider) -> Self {
        self.config.provider = provider;
        self
    }

    pub fn path_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.config.path_pattern = pattern.into();
        self
    }

    pub fn status(mut self, code: u16, body: impl Into<String>) -> Self {
        self.config.faults = vec![HttpFaultType::Status {
            code,
            body: body.into(),
        }];
        self
    }

    pub fn latency(mut self, delay: Duration) -> Self {
        self.config.faults = vec![HttpFaultType::Latency { delay }];
        self
    }

    pub fn faults(mut self, faults: Vec<HttpFaultType>) -> Self {
        self.config.faults = faults;
        self
    }

    pub fn rate(mut self, rate: f64) -> Self {
        self.config.rate = rate;
        self
    }

    pub fn build(self) -> HttpFaultInjector {
        HttpFaultInjector::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    async fn upstream() -> (String, CancellationToken) {
        async fn handler() -> impl IntoResponse {
            (
                [(header::CONTENT_TYPE, "text/event-stream")],
                "data: one\n\ndata: two\n\ndata: three\n\n",
            )
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        tokio::spawn(async move {
            axum::serve(listener, Router::new().fallback(any(handler)))
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .unwrap();
        });
        (format!("http://{}", address), cancellation)
    }

    #[tokio::test]
    async fn provider_rate_limit_is_observable_and_proxy_is_removed() {
        let (upstream, stop_upstream) = upstream().await;
        let injector = HttpFaultInjector::builder()
            .upstream(upstream)
            .provider(AiProvider::Anthropic)
            .status(429, "")
            .build();
        let handle = injector.inject(&Target::System).await.unwrap();
        let listen = handle.metadata["listen"].as_str().unwrap().to_string();

        let response = reqwest::get(format!("http://{}/v1/messages", listen))
            .await
            .unwrap();
        assert_eq!(response.status(), 429);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["type"], "error");
        assert_eq!(body["error"]["type"], "rate_limit_error");

        injector.remove(handle).await.unwrap();
        assert!(reqwest::get(format!("http://{}/v1/messages", listen))
            .await
            .is_err());
        stop_upstream.cancel();
    }

    #[tokio::test]
    async fn stream_abort_drops_events_and_reports_metrics() {
        let (upstream, stop_upstream) = upstream().await;
        let injector = HttpFaultInjector::builder()
            .upstream(upstream)
            .provider(AiProvider::OpenAi)
            .faults(vec![HttpFaultType::StreamAbort { after_events: 1 }])
            .build();
        let handle = injector.inject(&Target::System).await.unwrap();
        let listen = handle.metadata["listen"].as_str().unwrap();

        let body = reqwest::get(format!("http://{}/v1/responses", listen))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "data: one\n\n");
        assert_eq!(
            injector
                .metrics(&handle.id)
                .await
                .unwrap()
                .stream_events_dropped,
            2
        );

        injector.remove(handle).await.unwrap();
        stop_upstream.cancel();
    }

    #[tokio::test]
    async fn response_body_replacement_is_observable() {
        let (upstream, stop_upstream) = upstream().await;
        let injector = HttpFaultInjector::builder()
            .upstream(upstream)
            .faults(vec![HttpFaultType::ReplaceBody {
                body: "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:7\n".to_string(),
                content_type: "application/vnd.apple.mpegurl".to_string(),
            }])
            .build();
        let handle = injector.inject(&Target::System).await.unwrap();
        let listen = handle.metadata["listen"].as_str().unwrap();

        let response = reqwest::get(format!("http://{}/live.m3u8", listen))
            .await
            .unwrap();
        assert_eq!(
            response.headers()["content-type"],
            "application/vnd.apple.mpegurl"
        );
        assert_eq!(
            response.text().await.unwrap(),
            "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:7\n"
        );

        injector.remove(handle).await.unwrap();
        stop_upstream.cancel();
    }

    #[test]
    fn context_truncation_keeps_the_newest_items() {
        let mut body = serde_json::to_vec(&json!({
            "messages": [
                {"role": "user", "content": "old"},
                {"role": "assistant", "content": "middle"},
                {"role": "user", "content": "new"}
            ]
        }))
        .unwrap();
        assert!(truncate_context(&mut body, 1));
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["messages"].as_array().unwrap().len(), 1);
        assert_eq!(value["messages"][0]["content"], "new");
    }
}
