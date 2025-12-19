use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionHandle {
    pub id: String,
    pub injector_name: String,
    pub target: crate::target::Target,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub metadata: serde_json::Value,
}

impl InjectionHandle {
    pub fn new(
        injector_name: impl Into<String>,
        target: crate::target::Target,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            injector_name: injector_name.into(),
            target,
            started_at: chrono::Utc::now(),
            metadata,
        }
    }

    pub fn duration(&self) -> chrono::Duration {
        chrono::Utc::now() - self.started_at
    }
}

#[derive(Debug, Clone)]
pub struct InjectionState {
    handle: InjectionHandle,
    active: Arc<RwLock<bool>>,
}

impl InjectionState {
    pub fn new(handle: InjectionHandle) -> Self {
        Self {
            handle,
            active: Arc::new(RwLock::new(true)),
        }
    }

    pub async fn is_active(&self) -> bool {
        *self.active.read().await
    }

    pub async fn deactivate(&self) {
        let mut active = self.active.write().await;
        *active = false;
    }

    pub fn handle(&self) -> &InjectionHandle {
        &self.handle
    }
}
