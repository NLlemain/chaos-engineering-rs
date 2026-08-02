use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClockDirection {
    Forward,
    Backward,
    Jitter { max_variance: Duration },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockSkewConfig {
    pub offset: Duration,
    pub direction: ClockDirection,
}

impl Default for ClockSkewConfig {
    fn default() -> Self {
        Self {
            offset: Duration::from_secs(3600),
            direction: ClockDirection::Forward,
        }
    }
}

pub struct ClockSkewInjector {
    config: ClockSkewConfig,
}

impl Default for ClockSkewInjector {
    fn default() -> Self {
        Self::new(ClockSkewConfig::default())
    }
}

impl ClockSkewInjector {
    pub fn new(config: ClockSkewConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> ClockSkewBuilder {
        ClockSkewBuilder::default()
    }
}

#[async_trait]
impl Injector for ClockSkewInjector {
    async fn inject(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::InvalidConfig(format!(
            "clock_skew is planned and did not apply the requested {:?} offset",
            self.config.offset
        )))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!("Removing Clock Skew injection - resetting clock offset to zero");
        Ok(())
    }

    fn name(&self) -> &str {
        "clock_skew"
    }

    fn status(&self) -> crate::injectors::InjectorStatus {
        crate::injectors::InjectorStatus::Planned
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_SYS_TIME".to_string()]
    }
}

#[derive(Default)]
pub struct ClockSkewBuilder {
    offset: Option<Duration>,
    direction: Option<ClockDirection>,
}

impl ClockSkewBuilder {
    pub fn offset(mut self, offset: Duration) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn direction(mut self, direction: ClockDirection) -> Self {
        self.direction = Some(direction);
        self
    }

    pub fn build(self) -> ClockSkewInjector {
        ClockSkewInjector::new(ClockSkewConfig {
            offset: self.offset.unwrap_or_else(|| Duration::from_secs(60)),
            direction: self.direction.unwrap_or(ClockDirection::Forward),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clock_skew_builder() {
        let injector = ClockSkewInjector::builder()
            .offset(Duration::from_secs(300))
            .direction(ClockDirection::Backward)
            .build();

        assert_eq!(injector.config.offset, Duration::from_secs(300));
        let target = Target::System;
        assert!(injector.inject(&target).await.is_err());
    }
}
