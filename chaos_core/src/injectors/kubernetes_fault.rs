use crate::{
    error::{ChaosError, Result},
    handle::InjectionHandle,
    injectors::{Injector, InjectorStatus},
    target::{KubernetesWorkloadKind, Target},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::info;

const RUN_LABEL: &str = "chaos-engineering-rs/run";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KubernetesFaultMode {
    NetworkIsolation,
    ScaleToZero,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KubernetesFaultConfig {
    pub mode: KubernetesFaultMode,
    #[serde(default = "default_blast_radius")]
    pub blast_radius_percent: u8,
    #[serde(default = "default_max_pods")]
    pub max_pods: usize,
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "default_true")]
    pub deny_ingress: bool,
    #[serde(default = "default_true")]
    pub deny_egress: bool,
}

fn default_blast_radius() -> u8 {
    25
}

fn default_max_pods() -> usize {
    1
}

fn default_true() -> bool {
    true
}

impl Default for KubernetesFaultConfig {
    fn default() -> Self {
        Self {
            mode: KubernetesFaultMode::NetworkIsolation,
            blast_radius_percent: default_blast_radius(),
            max_pods: default_max_pods(),
            seed: 0,
            deny_ingress: true,
            deny_egress: true,
        }
    }
}

impl KubernetesFaultConfig {
    pub fn validate(&self) -> Result<()> {
        if !(1..=100).contains(&self.blast_radius_percent) {
            return Err(ChaosError::InvalidConfig(
                "Kubernetes blast_radius_percent must be between 1 and 100".into(),
            ));
        }
        if self.max_pods == 0 {
            return Err(ChaosError::InvalidConfig(
                "Kubernetes max_pods must be greater than zero".into(),
            ));
        }
        if self.mode == KubernetesFaultMode::NetworkIsolation
            && !self.deny_ingress
            && !self.deny_egress
        {
            return Err(ChaosError::InvalidConfig(
                "Kubernetes network isolation must deny ingress, egress, or both".into(),
            ));
        }
        Ok(())
    }
}

pub struct KubernetesFaultInjector {
    config: KubernetesFaultConfig,
}

impl KubernetesFaultInjector {
    pub fn new(config: KubernetesFaultConfig) -> Self {
        Self { config }
    }
}

impl Default for KubernetesFaultInjector {
    fn default() -> Self {
        Self::new(KubernetesFaultConfig::default())
    }
}

#[async_trait]
impl Injector for KubernetesFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.config.validate()?;
        let target = KubernetesTarget::from_target(target)?;
        validate_target(&target, self.config.mode)?;
        validate_access(&target, self.config.mode).await?;
        match self.config.mode {
            KubernetesFaultMode::NetworkIsolation => {
                inject_network_isolation(target, &self.config).await
            }
            KubernetesFaultMode::ScaleToZero => inject_scale_to_zero(target).await,
        }
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let mode = handle
            .metadata
            .get("mode")
            .and_then(Value::as_str)
            .ok_or_else(|| ChaosError::CleanupFailed("Kubernetes handle has no mode".into()))?;
        let target = KubernetesTarget::from_handle(&handle)?;
        match mode {
            "network_isolation" => recover_network_isolation(&target, &handle).await,
            "scale_to_zero" => recover_scale(&target, &handle).await,
            value => Err(ChaosError::CleanupFailed(format!(
                "Unknown Kubernetes recovery mode '{value}'"
            ))),
        }
    }

    fn name(&self) -> &str {
        "kubernetes_fault"
    }

    fn status(&self) -> InjectorStatus {
        InjectorStatus::Experimental
    }

    async fn validate(&self) -> Result<()> {
        self.config.validate()?;
        run_kubectl(&["version".into(), "--client".into(), "--output=json".into()])
            .await
            .map(|_| ())
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec![
            "kubectl".into(),
            "Kubernetes API access".into(),
            "RBAC for pods and network policies or workload scaling".into(),
        ]
    }
}

#[derive(Clone)]
struct KubernetesTarget {
    context: Option<String>,
    namespace: String,
    kind: KubernetesWorkloadKind,
    name: Option<String>,
    selector: Option<String>,
}

impl KubernetesTarget {
    fn from_target(target: &Target) -> Result<Self> {
        let Target::Kubernetes {
            context,
            namespace,
            kind,
            name,
            selector,
        } = target
        else {
            return Err(ChaosError::InvalidConfig(
                "kubernetes_fault requires a Kubernetes target".into(),
            ));
        };
        Ok(Self {
            context: context.clone(),
            namespace: namespace.clone(),
            kind: *kind,
            name: name.clone(),
            selector: selector.clone(),
        })
    }

    fn from_handle(handle: &InjectionHandle) -> Result<Self> {
        Self::from_target(&handle.target).map_err(|error| {
            ChaosError::CleanupFailed(format!("Invalid Kubernetes recovery target: {error}"))
        })
    }

    fn prefix(&self) -> Vec<String> {
        let mut arguments = Vec::new();
        if let Some(context) = &self.context {
            arguments.extend(["--context".into(), context.clone()]);
        }
        arguments.extend(["--namespace".into(), self.namespace.clone()]);
        arguments
    }
}

fn validate_target(target: &KubernetesTarget, mode: KubernetesFaultMode) -> Result<()> {
    if target.namespace.trim().is_empty() {
        return Err(ChaosError::InvalidConfig(
            "Kubernetes namespace cannot be empty".into(),
        ));
    }
    if target.name.is_none() && target.selector.is_none() {
        return Err(ChaosError::InvalidConfig(
            "Kubernetes target requires a name or label selector".into(),
        ));
    }
    if mode == KubernetesFaultMode::ScaleToZero
        && (!matches!(
            target.kind,
            KubernetesWorkloadKind::Deployment | KubernetesWorkloadKind::StatefulSet
        ) || target.name.is_none())
    {
        return Err(ChaosError::InvalidConfig(
            "Kubernetes scale_to_zero requires a named deployment or statefulset".into(),
        ));
    }
    Ok(())
}

async fn validate_access(target: &KubernetesTarget, mode: KubernetesFaultMode) -> Result<()> {
    let permissions: &[(&str, &str)] = match mode {
        KubernetesFaultMode::NetworkIsolation => &[
            ("list", "pods"),
            ("patch", "pods"),
            ("create", "networkpolicies.networking.k8s.io"),
            ("delete", "networkpolicies.networking.k8s.io"),
        ],
        KubernetesFaultMode::ScaleToZero => &[
            ("get", target.kind.resource()),
            ("patch", target.kind.resource()),
        ],
    };
    for (verb, resource) in permissions {
        let mut arguments = target.prefix();
        arguments.extend([
            "auth".into(),
            "can-i".into(),
            (*verb).into(),
            (*resource).into(),
        ]);
        let output = run_kubectl(&arguments).await?;
        if output.trim() != "yes" {
            return Err(ChaosError::PermissionDenied(format!(
                "kubectl auth can-i {verb} {resource} returned '{}'",
                output.trim()
            )));
        }
    }
    Ok(())
}

async fn inject_network_isolation(
    target: KubernetesTarget,
    config: &KubernetesFaultConfig,
) -> Result<InjectionHandle> {
    let candidates = discover_pods(&target).await?;
    let pods = select_pods(
        candidates,
        config.seed,
        config.blast_radius_percent,
        config.max_pods,
    );
    if pods.is_empty() {
        return Err(ChaosError::TargetNotFound(
            "Kubernetes selector matched no pods".into(),
        ));
    }
    let marker = format!("run-{}", uuid::Uuid::new_v4().simple());
    let policy_name = format!("chaos-{}", &marker[4..16]);
    let handle = InjectionHandle::new(
        "kubernetes_fault",
        Target::kubernetes(
            target.context.clone(),
            target.namespace.clone(),
            target.kind,
            target.name.clone(),
            target.selector.clone(),
        ),
        serde_json::json!({
            "mode": "network_isolation",
            "policy_name": policy_name,
            "marker": marker,
            "pods": pods,
            "deny_ingress": config.deny_ingress,
            "deny_egress": config.deny_egress,
            "blast_radius_percent": config.blast_radius_percent,
            "seed": config.seed,
        }),
    );

    let result = apply_network_isolation(&target, &handle).await;
    if let Err(error) = result {
        let _ = recover_network_isolation(&target, &handle).await;
        return Err(error);
    }
    Ok(handle)
}

async fn apply_network_isolation(
    target: &KubernetesTarget,
    handle: &InjectionHandle,
) -> Result<()> {
    let pods = metadata_strings(handle, "pods")?;
    let marker = metadata_string(handle, "marker")?;
    for pod in &pods {
        let mut arguments = target.prefix();
        arguments.extend([
            "label".into(),
            "pod".into(),
            pod.clone(),
            format!("{RUN_LABEL}={marker}"),
            "--overwrite".into(),
        ]);
        run_kubectl(&arguments).await?;
    }

    let policy = network_policy_manifest(
        &target.namespace,
        metadata_string(handle, "policy_name")?,
        marker,
        handle.metadata["deny_ingress"].as_bool().unwrap_or(true),
        handle.metadata["deny_egress"].as_bool().unwrap_or(true),
    );
    let mut arguments = target.prefix();
    arguments.extend(["apply".into(), "--filename=-".into()]);
    run_kubectl_with_input(&arguments, &serde_json::to_vec(&policy)?).await?;

    let mut verify = target.prefix();
    verify.extend([
        "get".into(),
        "networkpolicy".into(),
        metadata_string(handle, "policy_name")?.into(),
        "--output=json".into(),
    ]);
    run_kubectl(&verify).await?;
    for pod in &pods {
        let pod_json = get_json(target, "pod", pod).await?;
        if pod_json
            .pointer("/metadata/labels")
            .and_then(Value::as_object)
            .and_then(|labels| labels.get(RUN_LABEL))
            .and_then(Value::as_str)
            != Some(marker)
        {
            return Err(ChaosError::InjectionFailed(format!(
                "Pod '{pod}' did not retain the experiment label"
            )));
        }
    }
    info!(policy = %metadata_string(handle, "policy_name")?, pods = pods.len(), "Kubernetes network isolation applied and verified");
    Ok(())
}

async fn recover_network_isolation(
    target: &KubernetesTarget,
    handle: &InjectionHandle,
) -> Result<()> {
    let policy_name = metadata_string(handle, "policy_name")?;
    let mut delete = target.prefix();
    delete.extend([
        "delete".into(),
        "networkpolicy".into(),
        policy_name.into(),
        "--ignore-not-found=true".into(),
    ]);
    run_kubectl(&delete).await?;
    for pod in metadata_strings(handle, "pods")? {
        let mut arguments = target.prefix();
        arguments.extend([
            "label".into(),
            "pod".into(),
            pod,
            format!("{RUN_LABEL}-"),
            "--overwrite".into(),
        ]);
        run_kubectl(&arguments).await?;
    }
    Ok(())
}

async fn inject_scale_to_zero(target: KubernetesTarget) -> Result<InjectionHandle> {
    let name = target.name.as_deref().expect("validated workload name");
    let workload = get_json(&target, target.kind.resource(), name).await?;
    let replicas = workload
        .pointer("/spec/replicas")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ChaosError::InvalidConfig("Kubernetes workload has no numeric spec.replicas".into())
        })?;
    if replicas == 0 {
        return Err(ChaosError::InvalidConfig(
            "Kubernetes workload is already scaled to zero".into(),
        ));
    }
    let handle = InjectionHandle::new(
        "kubernetes_fault",
        Target::kubernetes(
            target.context.clone(),
            target.namespace.clone(),
            target.kind,
            target.name.clone(),
            target.selector.clone(),
        ),
        serde_json::json!({
            "mode": "scale_to_zero",
            "original_replicas": replicas,
        }),
    );
    scale(&target, 0).await?;
    let updated = get_json(&target, target.kind.resource(), name).await?;
    if updated.pointer("/spec/replicas").and_then(Value::as_u64) != Some(0) {
        let _ = recover_scale(&target, &handle).await;
        return Err(ChaosError::InjectionFailed(
            "Kubernetes workload did not scale to zero".into(),
        ));
    }
    Ok(handle)
}

async fn recover_scale(target: &KubernetesTarget, handle: &InjectionHandle) -> Result<()> {
    let replicas = handle.metadata["original_replicas"]
        .as_u64()
        .ok_or_else(|| {
            ChaosError::CleanupFailed("Kubernetes handle has no original replica count".into())
        })?;
    scale(target, replicas).await?;
    let name = target.name.as_deref().ok_or_else(|| {
        ChaosError::CleanupFailed("Kubernetes recovery target has no workload name".into())
    })?;
    let updated = get_json(target, target.kind.resource(), name).await?;
    if updated.pointer("/spec/replicas").and_then(Value::as_u64) != Some(replicas) {
        return Err(ChaosError::CleanupFailed(format!(
            "Kubernetes workload did not return to {replicas} replicas"
        )));
    }
    Ok(())
}

async fn scale(target: &KubernetesTarget, replicas: u64) -> Result<()> {
    let mut arguments = target.prefix();
    arguments.extend([
        "scale".into(),
        format!(
            "{}/{}",
            target.kind.resource(),
            target.name.as_deref().expect("validated workload name")
        ),
        format!("--replicas={replicas}"),
    ]);
    run_kubectl(&arguments).await.map(|_| ())
}

async fn discover_pods(target: &KubernetesTarget) -> Result<Vec<String>> {
    if target.kind == KubernetesWorkloadKind::Pod {
        if let Some(name) = &target.name {
            get_json(target, "pod", name).await?;
            return Ok(vec![name.clone()]);
        }
    }
    let selector = match &target.selector {
        Some(selector) => selector.clone(),
        None => workload_selector(target).await?,
    };
    let mut arguments = target.prefix();
    arguments.extend([
        "get".into(),
        "pods".into(),
        "--selector".into(),
        selector,
        "--output=json".into(),
    ]);
    let output = run_kubectl(&arguments).await?;
    let value: Value = serde_json::from_str(&output)?;
    Ok(value["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.pointer("/metadata/name").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

async fn workload_selector(target: &KubernetesTarget) -> Result<String> {
    let name = target.name.as_deref().ok_or_else(|| {
        ChaosError::InvalidConfig("Kubernetes workload requires a name or selector".into())
    })?;
    let workload = get_json(target, target.kind.resource(), name).await?;
    let labels = workload
        .pointer("/spec/selector/matchLabels")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            ChaosError::InvalidConfig(
                "Kubernetes workload selector must contain matchLabels".into(),
            )
        })?;
    let mut labels: Vec<_> = labels
        .iter()
        .filter_map(|(key, value)| value.as_str().map(|value| format!("{key}={value}")))
        .collect();
    labels.sort();
    if labels.is_empty() {
        return Err(ChaosError::InvalidConfig(
            "Kubernetes workload selector has no string matchLabels".into(),
        ));
    }
    Ok(labels.join(","))
}

fn select_pods(
    mut pods: Vec<String>,
    seed: u64,
    blast_radius_percent: u8,
    max_pods: usize,
) -> Vec<String> {
    pods.sort_by_key(|pod| {
        let mut hasher = Sha256::new();
        hasher.update(seed.to_be_bytes());
        hasher.update(pod.as_bytes());
        hasher.finalize().to_vec()
    });
    let limit = pods
        .len()
        .saturating_mul(blast_radius_percent as usize)
        .div_ceil(100)
        .max(usize::from(!pods.is_empty()))
        .min(max_pods)
        .min(pods.len());
    pods.truncate(limit);
    pods.sort();
    pods
}

fn network_policy_manifest(
    namespace: &str,
    name: &str,
    marker: &str,
    deny_ingress: bool,
    deny_egress: bool,
) -> Value {
    let mut policy_types = Vec::new();
    if deny_ingress {
        policy_types.push(Value::String("Ingress".into()));
    }
    if deny_egress {
        policy_types.push(Value::String("Egress".into()));
    }
    let mut spec = serde_json::json!({
        "podSelector": {"matchLabels": {(RUN_LABEL): marker}},
        "policyTypes": policy_types,
    });
    if deny_ingress {
        spec["ingress"] = Value::Array(Vec::new());
    }
    if deny_egress {
        spec["egress"] = Value::Array(Vec::new());
    }
    serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": {"app.kubernetes.io/managed-by": "chaos-engineering-rs"}
        },
        "spec": spec,
    })
}

async fn get_json(target: &KubernetesTarget, resource: &str, name: &str) -> Result<Value> {
    let mut arguments = target.prefix();
    arguments.extend([
        "get".into(),
        resource.into(),
        name.into(),
        "--output=json".into(),
    ]);
    let output = run_kubectl(&arguments).await?;
    serde_json::from_str(&output).map_err(Into::into)
}

async fn run_kubectl(arguments: &[String]) -> Result<String> {
    let output = Command::new(kubectl_binary())
        .args(arguments)
        .output()
        .await
        .map_err(|error| ChaosError::SystemError(format!("Could not run kubectl: {error}")))?;
    if !output.status.success() {
        return Err(ChaosError::InjectionFailed(format!(
            "kubectl {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn run_kubectl_with_input(arguments: &[String], input: &[u8]) -> Result<String> {
    let mut child = Command::new(kubectl_binary())
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ChaosError::SystemError(format!("Could not run kubectl: {error}")))?;
    child
        .stdin
        .take()
        .ok_or_else(|| ChaosError::SystemError("kubectl stdin was unavailable".into()))?
        .write_all(input)
        .await?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        return Err(ChaosError::InjectionFailed(format!(
            "kubectl {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn kubectl_binary() -> std::ffi::OsString {
    std::env::var_os("CHAOS_KUBECTL_BIN").unwrap_or_else(|| "kubectl".into())
}

fn metadata_string<'a>(handle: &'a InjectionHandle, key: &str) -> Result<&'a str> {
    handle.metadata[key].as_str().ok_or_else(|| {
        ChaosError::CleanupFailed(format!("Kubernetes handle has no '{key}' metadata"))
    })
}

fn metadata_strings(handle: &InjectionHandle, key: &str) -> Result<Vec<String>> {
    handle.metadata[key]
        .as_array()
        .ok_or_else(|| ChaosError::CleanupFailed(format!("Kubernetes handle has no '{key}' list")))?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                ChaosError::CleanupFailed(format!(
                    "Kubernetes handle '{key}' contains a non-string value"
                ))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_selection_is_seeded_and_bounded_by_blast_radius() {
        let pods = (0..10).map(|index| format!("feed-{index}")).collect();
        let first = select_pods(pods, 42, 30, 10);
        let second = select_pods(
            (0..10).map(|index| format!("feed-{index}")).collect(),
            42,
            30,
            10,
        );
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
    }

    #[test]
    fn network_policy_has_an_empty_rule_set_for_selected_directions() {
        let policy = network_policy_manifest("trading", "chaos-run", "run-1", true, false);
        assert_eq!(
            policy.pointer("/spec/podSelector/matchLabels/chaos-engineering-rs~1run"),
            Some(&Value::String("run-1".into()))
        );
        assert_eq!(policy.pointer("/spec/ingress"), Some(&Value::Array(vec![])));
        assert!(policy.pointer("/spec/egress").is_none());
    }

    #[test]
    fn scale_mode_requires_a_named_controller() {
        let target = KubernetesTarget {
            context: None,
            namespace: "default".into(),
            kind: KubernetesWorkloadKind::Pod,
            name: Some("feed-0".into()),
            selector: None,
        };
        assert!(validate_target(&target, KubernetesFaultMode::ScaleToZero).is_err());
    }
}
