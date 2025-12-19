use crate::{
    error::Result,
    handle::{InjectionHandle, InjectionState},
    injectors::InjectorRegistry,
    target::Target,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub struct Executor {
    registry: Arc<InjectorRegistry>,
    active_injections: Arc<RwLock<HashMap<String, InjectionState>>>,
}

impl Executor {
    pub fn new(registry: InjectorRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
            active_injections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(InjectorRegistry::with_defaults())
    }

    pub async fn inject(&self, injector_name: &str, target: &Target) -> Result<InjectionHandle> {
        let injector = self.registry.get(injector_name).ok_or_else(|| {
            crate::error::ChaosError::InvalidConfig(format!(
                "Injector '{}' not found",
                injector_name
            ))
        })?;

        info!(
            "Applying injection '{}' to target: {}",
            injector_name,
            target.description()
        );

        let handle = injector.inject(target).await?;
        let state = InjectionState::new(handle.clone());

        self.active_injections
            .write()
            .await
            .insert(handle.id.clone(), state);

        Ok(handle)
    }

    pub async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let injector = self.registry.get(&handle.injector_name).ok_or_else(|| {
            crate::error::ChaosError::InvalidConfig(format!(
                "Injector '{}' not found",
                handle.injector_name
            ))
        })?;

        info!("Removing injection '{}'", handle.id);

        injector.remove(handle.clone()).await?;

        if let Some(state) = self.active_injections.write().await.remove(&handle.id) {
            state.deactivate().await;
        }

        Ok(())
    }

    pub async fn remove_all(&self) -> Result<()> {
        info!("Removing all active injections");

        let handles: Vec<InjectionHandle> = self
            .active_injections
            .read()
            .await
            .values()
            .map(|state| state.handle().clone())
            .collect();

        for handle in handles {
            if let Err(e) = self.remove(handle).await {
                tracing::warn!("Failed to remove injection: {}", e);
            }
        }

        Ok(())
    }

    pub async fn list_active(&self) -> Vec<InjectionHandle> {
        self.active_injections
            .read()
            .await
            .values()
            .map(|state| state.handle().clone())
            .collect()
    }

    pub async fn get_state(&self, handle_id: &str) -> Option<InjectionState> {
        self.active_injections.read().await.get(handle_id).cloned()
    }

    pub fn list_injectors(&self) -> Vec<String> {
        self.registry.list()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = Executor::with_defaults();
        let injectors = executor.list_injectors();

        assert!(injectors.contains(&"network_latency".to_string()));
        assert!(injectors.contains(&"cpu_starvation".to_string()));
        assert!(injectors.contains(&"process_kill".to_string()));
    }

    #[tokio::test]
    async fn test_active_injections_tracking() {
        let executor = Executor::with_defaults();
        assert_eq!(executor.list_active().await.len(), 0);
    }
}
