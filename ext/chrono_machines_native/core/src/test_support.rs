//! Fixtures shared by the crate's unit tests.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Serializes tests that use the process-wide policy registry, which
/// `cargo test` would otherwise clear and repopulate from several threads at
/// once.
static GLOBAL_REGISTRY: Mutex<()> = Mutex::new(());

/// Hold for the whole test body. Poisoning is ignored so one failing test
/// doesn't cascade into every other registry test.
pub fn lock_global_registry() -> MutexGuard<'static, ()> {
    GLOBAL_REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}
