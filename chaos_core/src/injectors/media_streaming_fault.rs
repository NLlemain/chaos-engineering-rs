use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaFaultType {
    HlsSegmentLatency { segment_delay: Duration },
    DashManifestCorrupt { remove_tags: Vec<String> },
    RtspKeyframeDrop { drop_rate: f64 },
    WebRtcSdpDelay { negotiation_delay: Duration },
    BitrateThrottleKbps { max_kbps: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaStreamingFaultConfig {
    pub fault_type: MediaFaultType,
    pub stream_id: String,
}

impl Default for MediaStreamingFaultConfig {
    fn default() -> Self {
        Self {
            fault_type: MediaFaultType::HlsSegmentLatency {
                segment_delay: Duration::from_millis(2000),
            },
            stream_id: "live_stream_01".to_string(),
        }
    }
}

pub struct MediaStreamingFaultInjector {
    config: MediaStreamingFaultConfig,
}

impl Default for MediaStreamingFaultInjector {
    fn default() -> Self {
        Self::new(MediaStreamingFaultConfig::default())
    }
}

impl MediaStreamingFaultInjector {
    pub fn new(config: MediaStreamingFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> MediaStreamingFaultBuilder {
        MediaStreamingFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for MediaStreamingFaultInjector {
    async fn inject(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::InvalidConfig(
            "media_streaming_fault is planned; no stream traffic was modified".into(),
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing Video Streaming Fault injection for stream '{}'",
            self.config.stream_id
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "media_streaming_fault"
    }

    fn status(&self) -> crate::injectors::InjectorStatus {
        crate::injectors::InjectorStatus::Planned
    }
}

#[derive(Default)]
pub struct MediaStreamingFaultBuilder {
    fault_type: Option<MediaFaultType>,
    stream_id: Option<String>,
}

impl MediaStreamingFaultBuilder {
    pub fn fault_type(mut self, fault: MediaFaultType) -> Self {
        self.fault_type = Some(fault);
        self
    }

    pub fn stream_id(mut self, id: impl Into<String>) -> Self {
        self.stream_id = Some(id.into());
        self
    }

    pub fn build(self) -> MediaStreamingFaultInjector {
        MediaStreamingFaultInjector::new(MediaStreamingFaultConfig {
            fault_type: self
                .fault_type
                .unwrap_or(MediaFaultType::BitrateThrottleKbps { max_kbps: 256 }),
            stream_id: self
                .stream_id
                .unwrap_or_else(|| "default_stream".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_media_streaming_fault_builder() {
        let injector = MediaStreamingFaultInjector::builder()
            .fault_type(MediaFaultType::RtspKeyframeDrop { drop_rate: 0.25 })
            .stream_id("cam_north_01")
            .build();

        assert_eq!(injector.config.stream_id, "cam_north_01");
        let target = Target::System;
        assert!(injector.inject(&target).await.is_err());
    }
}
