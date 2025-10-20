//! Named policy management utilities.
//!
//! This module introduces a lightweight registry for `BackoffPolicy` values.
//! Registries can be instantiated locally (requires `alloc`) or accessed via a
//! global registry when the `std` feature is enabled. The goal is to provide a
//! convenient way to organise retry policies by name, mirroring the global
//! configuration style found in higher-level frameworks.

use crate::backoff::BackoffPolicy;

#[cfg(any(feature = "std", feature = "alloc"))]
use alloc::string::String;
#[cfg(any(feature = "std", feature = "alloc"))]
use alloc::vec::Vec;

/// In-memory registry for named [`BackoffPolicy`] values.
///
/// This registry performs simple linear lookups over an internal vector. The
/// design keeps the implementation `no_std`-friendly (when the `alloc` feature
/// is available) while remaining ergonomic for typical workloads where only a
/// handful of retry policies are defined.
#[cfg(any(feature = "std", feature = "alloc"))]
#[derive(Debug, Clone, Default)]
pub struct PolicyRegistry {
    entries: Vec<(String, BackoffPolicy)>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl PolicyRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a policy under the given name.
    ///
    /// Returns the previously registered policy if one existed.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        policy: BackoffPolicy,
    ) -> Option<BackoffPolicy> {
        let name = name.into();
        if let Some((_, existing)) = self
            .entries
            .iter_mut()
            .find(|(existing_name, _)| *existing_name == name)
        {
            let previous = *existing;
            *existing = policy;
            Some(previous)
        } else {
            self.entries.push((name, policy));
            None
        }
    }

    /// Retrieve a policy by name.
    pub fn get(&self, name: &str) -> Option<BackoffPolicy> {
        self.entries
            .iter()
            .find(|(existing_name, _)| existing_name == name)
            .map(|(_, policy)| *policy)
    }

    /// Remove a policy by name.
    ///
    /// Returns the removed policy when it existed.
    pub fn remove(&mut self, name: &str) -> Option<BackoffPolicy> {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(existing_name, _)| existing_name == name)
        {
            Some(self.entries.swap_remove(index).1)
        } else {
            None
        }
    }

    /// Return all registered policies as `(name, policy)` tuples.
    pub fn all(&self) -> Vec<(String, BackoffPolicy)> {
        self.entries.iter().cloned().collect()
    }

    /// Clear the registry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(feature = "std")]
use std::sync::{OnceLock, RwLock};

#[cfg(feature = "std")]
fn global_registry() -> &'static RwLock<PolicyRegistry> {
    static GLOBAL_POLICIES: OnceLock<RwLock<PolicyRegistry>> = OnceLock::new();
    GLOBAL_POLICIES.get_or_init(|| RwLock::new(PolicyRegistry::new()))
}

/// Register a policy in the global registry (requires `std`).
#[cfg(feature = "std")]
pub fn register_global_policy(
    name: impl Into<String>,
    policy: BackoffPolicy,
) -> Option<BackoffPolicy> {
    let mut guard = global_registry()
        .write()
        .expect("chronomachines global policy registry poisoned");
    guard.register(name, policy)
}

/// Fetch a policy from the global registry (requires `std`).
#[cfg(feature = "std")]
pub fn get_global_policy(name: &str) -> Option<BackoffPolicy> {
    let guard = global_registry()
        .read()
        .expect("chronomachines global policy registry poisoned");
    guard.get(name)
}

/// Remove a policy from the global registry (requires `std`).
#[cfg(feature = "std")]
pub fn remove_global_policy(name: &str) -> Option<BackoffPolicy> {
    let mut guard = global_registry()
        .write()
        .expect("chronomachines global policy registry poisoned");
    guard.remove(name)
}

/// List all policies from the global registry (requires `std`).
#[cfg(feature = "std")]
pub fn list_global_policies() -> Vec<(String, BackoffPolicy)> {
    let guard = global_registry()
        .read()
        .expect("chronomachines global policy registry poisoned");
    guard.all()
}

/// Clear all entries from the global registry (requires `std`).
#[cfg(feature = "std")]
pub fn clear_global_policies() {
    let mut guard = global_registry()
        .write()
        .expect("chronomachines global policy registry poisoned");
    guard.clear();
}

#[cfg(all(test, any(feature = "std", feature = "alloc")))]
mod tests {
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
}
