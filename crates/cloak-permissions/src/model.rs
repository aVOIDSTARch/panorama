use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use cloak_core::OperationClass;

/// A single permission entry defining what an identity pattern can access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEntry {
    /// Pattern matching agent_class or job_id (e.g. "researcher", "*")
    pub identity_pattern: String,
    /// Target service name
    pub service: String,
    /// Maximum operation class allowed
    pub operation_class: OperationClass,
    /// Allowed resource paths, or ["*"] for all
    pub resources: Vec<String>,
}

/// Thread-safe store for permission entries.
#[derive(Debug, Clone)]
pub struct PermissionStore {
    entries: Arc<RwLock<Vec<PermissionEntry>>>,
}

impl PermissionStore {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_entries(entries: Vec<PermissionEntry>) -> Self {
        Self {
            entries: Arc::new(RwLock::new(entries)),
        }
    }

    pub async fn list(&self) -> Vec<PermissionEntry> {
        self.entries.read().await.clone()
    }

    pub async fn add(&self, entry: PermissionEntry) {
        self.entries.write().await.push(entry);
    }

    pub async fn remove(&self, identity_pattern: &str, service: &str) -> bool {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|e| !(e.identity_pattern == identity_pattern && e.service == service));
        entries.len() < before
    }

    pub async fn replace_all(&self, new_entries: Vec<PermissionEntry>) {
        let mut entries = self.entries.write().await;
        *entries = new_entries;
    }
}

impl Default for PermissionStore {
    fn default() -> Self {
        Self::new()
    }
}
