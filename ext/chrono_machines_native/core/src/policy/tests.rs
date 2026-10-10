use super::*;
use crate::backoff::{BackoffPolicy, ExponentialBackoff};

#[test]
fn test_registry_crud() {
    let mut registry = PolicyRegistry::new();
    assert!(registry.get("missing").is_none());

    let policy = BackoffPolicy::from(ExponentialBackoff::new().max_attempts(5));
    assert!(registry.register("api", policy).is_none());
    assert_eq!(registry.get("api").unwrap().max_attempts(), 5);

    let new_policy = BackoffPolicy::from(ExponentialBackoff::new().max_attempts(3));
    let replaced = registry.register("api", new_policy);
    assert_eq!(replaced.unwrap().max_attempts(), 5);
    assert_eq!(registry.get("api").unwrap().max_attempts(), 3);

    let removed = registry.remove("api");
    assert!(removed.is_some());
    assert!(registry.get("api").is_none());
}

#[cfg(feature = "std")]
#[test]
fn test_global_registry_roundtrip() {
    clear_global_policies();
    assert!(list_global_policies().is_empty());

    let policy = BackoffPolicy::from(ExponentialBackoff::new().max_attempts(4));
    assert!(register_global_policy("workers", policy).is_none());

    let fetched = get_global_policy("workers").unwrap();
    assert_eq!(fetched.max_attempts(), 4);

    let removed = remove_global_policy("workers").unwrap();
    assert_eq!(removed.max_attempts(), 4);
    assert!(get_global_policy("workers").is_none());
}

/// A panic while the global lock is held must not brick the registry for the
/// rest of the process. Read-only on purpose: other tests share the global.
#[cfg(feature = "std")]
#[test]
fn test_global_registry_survives_poisoning() {
    let poisoned = std::panic::catch_unwind(|| {
        let _guard = GLOBAL_POLICIES.write();
        panic!("poison the global policy registry");
    });
    assert!(poisoned.is_err());
    assert!(GLOBAL_POLICIES.is_poisoned());

    // Both the read and the write path keep working.
    assert!(get_global_policy("poison-probe").is_none());
    assert!(remove_global_policy("poison-probe").is_none());
    assert!(
        list_global_policies()
            .iter()
            .all(|(name, _)| name != "poison-probe")
    );

    GLOBAL_POLICIES.clear_poison();
}
