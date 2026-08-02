use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorruptionMode {
    BitFlip { flip_probability: f64 },
    ByteSwap,
    Truncate { max_len: usize },
    ZeroOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketCorruptConfig {
    pub port: u16,
    pub corruption_mode: CorruptionMode,
    pub corruption_rate: f64,
}

impl Default for SocketCorruptConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            corruption_mode: CorruptionMode::BitFlip {
                flip_probability: 0.01,
            },
            corruption_rate: 0.1,
        }
    }
}

pub struct SocketCorruptInjector {
    config: SocketCorruptConfig,
}

impl Default for SocketCorruptInjector {
    fn default() -> Self {
        Self::new(SocketCorruptConfig::default())
    }
}

impl SocketCorruptInjector {
    pub fn new(config: SocketCorruptConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> SocketCorruptBuilder {
        SocketCorruptBuilder::default()
    }

    pub fn corrupt_bytes(&self, data: &mut [u8]) {
        match self.config.corruption_mode {
            CorruptionMode::BitFlip {
                flip_probability: _,
            } => {
                if !data.is_empty() {
                    data[0] ^= 0xFF; // Flip all bits of first byte
                }
            }
            CorruptionMode::ByteSwap => {
                if data.len() >= 2 {
                    data.swap(0, 1);
                }
            }
            CorruptionMode::Truncate { max_len } => {
                if data.len() > max_len {
                    for b in &mut data[max_len..] {
                        *b = 0;
                    }
                }
            }
            CorruptionMode::ZeroOut => {
                for b in data.iter_mut() {
                    *b = 0;
                }
            }
        }
    }
}

#[async_trait]
impl Injector for SocketCorruptInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting Socket Payload Corruption on port {} with mode {:?}",
            self.config.port, self.config.corruption_mode
        );

        let metadata = serde_json::json!({
            "port": self.config.port,
            "mode": format!("{:?}", self.config.corruption_mode),
            "rate": self.config.corruption_rate,
        });

        Ok(InjectionHandle::new(
            "socket_corrupt",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing Socket Payload Corruption on port {}",
            self.config.port
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "socket_corrupt"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_NET_RAW".to_string(), "CAP_NET_ADMIN".to_string()]
    }
}

#[derive(Default)]
pub struct SocketCorruptBuilder {
    port: Option<u16>,
    corruption_mode: Option<CorruptionMode>,
    corruption_rate: Option<f64>,
}

impl SocketCorruptBuilder {
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn corruption_mode(mut self, mode: CorruptionMode) -> Self {
        self.corruption_mode = Some(mode);
        self
    }

    pub fn corruption_rate(mut self, rate: f64) -> Self {
        self.corruption_rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn build(self) -> SocketCorruptInjector {
        SocketCorruptInjector::new(SocketCorruptConfig {
            port: self.port.unwrap_or(8080),
            corruption_mode: self.corruption_mode.unwrap_or(CorruptionMode::ByteSwap),
            corruption_rate: self.corruption_rate.unwrap_or(0.1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corrupt_bytes_bitflip() {
        let injector = SocketCorruptInjector::builder()
            .corruption_mode(CorruptionMode::BitFlip {
                flip_probability: 1.0,
            })
            .build();
        let mut buf = vec![0b00000000u8, 0b11111111u8];
        injector.corrupt_bytes(&mut buf);
        assert_eq!(buf[0], 0b11111111u8);
    }
}
