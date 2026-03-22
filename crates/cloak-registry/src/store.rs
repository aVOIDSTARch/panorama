use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::sync::broadcast;

use cloak_core::RegistrationRequest;

/// A registered service entry.
#[derive(Debug, Clone)]
pub struct RegisteredService {
    pub manifest: RegistrationRequest,
    pub session_id: String,
    pub signing_key: Vec<u8>,
    pub registered_at: DateTime<Utc>,
}

/// SSE event sent over halt channels.
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub data: String,
}

/// Thread-safe store of all registered services with their SSE broadcast channels.
#[derive(Debug, Clone)]
pub struct ServiceStore {
    services: Arc<DashMap<String, RegisteredService>>,
    channels: Arc<DashMap<String, broadcast::Sender<SseEvent>>>,
}

impl ServiceStore {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            channels: Arc::new(DashMap::new()),
        }
    }

    /// Register a service and create its SSE broadcast channel.
    /// Returns the broadcast receiver for the SSE halt stream.
    pub fn register(
        &self,
        service: RegisteredService,
    ) -> broadcast::Receiver<SseEvent> {
        let service_id = service.manifest.service_id.clone();
        let (tx, rx) = broadcast::channel(64);
        self.services.insert(service_id.clone(), service);
        self.channels.insert(service_id, tx);
        rx
    }

    pub fn get(&self, service_id: &str) -> Option<RegisteredService> {
        self.services.get(service_id).map(|v| v.clone())
    }

    pub fn is_registered(&self, service_id: &str) -> bool {
        self.services.contains_key(service_id)
    }

    pub fn list_ids(&self) -> Vec<String> {
        self.services.iter().map(|e| e.key().clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.services.len()
    }

    pub fn deregister(&self, service_id: &str) {
        self.services.remove(service_id);
        self.channels.remove(service_id);
    }

    /// Send an SSE event to a specific service's halt channel.
    pub fn send_to(&self, service_id: &str, event: SseEvent) -> bool {
        if let Some(tx) = self.channels.get(service_id) {
            tx.send(event).is_ok()
        } else {
            false
        }
    }

    /// Broadcast an SSE event to ALL registered services.
    pub fn broadcast(&self, event: SseEvent) {
        for entry in self.channels.iter() {
            let _ = entry.value().send(event.clone());
        }
    }

    /// Get a broadcast receiver for a specific service's halt stream.
    pub fn subscribe(&self, service_id: &str) -> Option<broadcast::Receiver<SseEvent>> {
        self.channels.get(service_id).map(|tx| tx.subscribe())
    }
}

impl Default for ServiceStore {
    fn default() -> Self {
        Self::new()
    }
}
