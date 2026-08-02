use crate::{
    error::Result,
    handle::{InjectionHandle, InjectionState},
    injectors::{DynInjector, InjectorInfo, InjectorRegistry, InjectorStatus},
    recovery::RecoveryJournal,
    target::Target,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub struct Executor {
    registry: Arc<InjectorRegistry>,
    active_injections: Arc<RwLock<HashMap<String, ActiveInjection>>>,
    journal: Option<Arc<RecoveryJournal>>,
}

#[derive(Clone)]
struct ActiveInjection {
    state: InjectionState,
    injector: DynInjector,
}

impl Executor {
    pub fn new(registry: InjectorRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
            active_injections: Arc::new(RwLock::new(HashMap::new())),
            journal: None,
        }
    }

    pub fn with_journal(registry: InjectorRegistry, journal: Arc<RecoveryJournal>) -> Self {
        Self {
            registry: Arc::new(registry),
            active_injections: Arc::new(RwLock::new(HashMap::new())),
            journal: Some(journal),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(InjectorRegistry::with_defaults())
    }

    pub fn with_defaults_and_journal(journal: Arc<RecoveryJournal>) -> Self {
        Self::with_journal(InjectorRegistry::with_defaults(), journal)
    }

    pub async fn inject(&self, injector_name: &str, target: &Target) -> Result<InjectionHandle> {
        let injector = self.registry.get(injector_name).cloned().ok_or_else(|| {
            crate::error::ChaosError::InvalidConfig(format!(
                "Injector '{}' not found",
                injector_name
            ))
        })?;

        self.inject_with(injector, target).await
    }

    /// Apply a configured injector and retain it for the matching cleanup call.
    pub async fn inject_with(
        &self,
        injector: DynInjector,
        target: &Target,
    ) -> Result<InjectionHandle> {
        if injector.status() == InjectorStatus::Planned {
            return Err(crate::error::ChaosError::InvalidConfig(format!(
                "Injector '{}' is planned but not implemented yet",
                injector.name()
            )));
        }
        injector.validate().await?;

        info!(
            "Applying injection '{}' to target: {}",
            injector.name(),
            target.description()
        );

        let handle = injector.inject(target).await?;
        if let Some(journal) = &self.journal {
            if let Err(error) = journal.record(&handle).await {
                let _ = injector.remove(handle).await;
                return Err(error);
            }
        }
        let state = InjectionState::new(handle.clone());

        self.active_injections
            .write()
            .await
            .insert(handle.id.clone(), ActiveInjection { state, injector });

        Ok(handle)
    }

    pub async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let active = self.active_injections.read().await.get(&handle.id).cloned();
        let injector = match &active {
            Some(active) => active.injector.clone(),
            None => self
                .registry
                .get(&handle.injector_name)
                .cloned()
                .ok_or_else(|| {
                    crate::error::ChaosError::InvalidConfig(format!(
                        "Injector '{}' not found",
                        handle.injector_name
                    ))
                })?,
        };

        info!("Removing injection '{}'", handle.id);

        injector.remove(handle.clone()).await?;

        if let Some(journal) = &self.journal {
            journal.remove(&handle.id).await?;
        }

        if let Some(active) = self.active_injections.write().await.remove(&handle.id) {
            active.state.deactivate().await;
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
            .map(|active| active.state.handle().clone())
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
            .map(|active| active.state.handle().clone())
            .collect()
    }

    pub async fn get_state(&self, handle_id: &str) -> Option<InjectionState> {
        self.active_injections
            .read()
            .await
            .get(handle_id)
            .map(|active| active.state.clone())
    }

    pub fn list_injectors(&self) -> Vec<String> {
        self.registry.list()
    }

    pub fn list_injector_info(&self) -> Vec<InjectorInfo> {
        self.registry.list_info()
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
