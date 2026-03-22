pub mod cloak;
pub mod cortex;
pub mod stub_service;
pub mod tokens;

pub use cloak::TestCloak;
pub use cortex::TestCortex;

use std::sync::Once;

static INIT_TRACING: Once = Once::new();

/// Initialize tracing once for the entire test process.
/// Uses `with_test_writer()` so output is captured by `cargo test`.
pub fn init_tracing() {
    INIT_TRACING.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("warn")
            .with_test_writer()
            .try_init()
            .ok();
    });
}
