use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Shared state managed by the Cloak SDK.
///
/// Thread-safe and cheaply cloneable (Arc-wrapped interior).
/// All Rust services hold one of these and pass it to middleware.
#[derive(Clone)]
pub struct CloakState {
    inner: Arc<RwLock<CloakStateInner>>,
}

pub struct CloakStateInner {
    pub session_id: Option<String>,
    pub signing_key: Option<Vec<u8>>,
    pub halted: bool,
    pub halt_reason: Option<String>,
    pub registered: bool,
    pub start_time: Instant,
}

impl CloakState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(CloakStateInner {
                session_id: None,
                signing_key: None,
                halted: false,
                halt_reason: None,
                registered: false,
                start_time: Instant::now(),
            })),
        }
    }

    pub async fn session_id(&self) -> Option<String> {
        self.inner.read().await.session_id.clone()
    }

    pub async fn signing_key(&self) -> Option<Vec<u8>> {
        self.inner.read().await.signing_key.clone()
    }

    pub async fn is_halted(&self) -> bool {
        self.inner.read().await.halted
    }

    pub async fn halt_reason(&self) -> Option<String> {
        self.inner.read().await.halt_reason.clone()
    }

    pub async fn is_registered(&self) -> bool {
        self.inner.read().await.registered
    }

    pub async fn uptime_seconds(&self) -> f64 {
        self.inner.read().await.start_time.elapsed().as_secs_f64()
    }

    pub(crate) async fn set_registered(
        &self,
        session_id: String,
        signing_key: Vec<u8>,
    ) {
        let mut inner = self.inner.write().await;
        inner.session_id = Some(session_id);
        inner.signing_key = Some(signing_key);
        inner.registered = true;
    }

    pub(crate) async fn set_halted(&self, reason: String) {
        let mut inner = self.inner.write().await;
        inner.halted = true;
        inner.halt_reason = Some(reason);
    }

    pub(crate) async fn rotate_key(&self, new_key: Vec<u8>) {
        let mut inner = self.inner.write().await;
        inner.signing_key = Some(new_key);
    }
}

impl Default for CloakState {
    fn default() -> Self {
        Self::new()
    }
}
