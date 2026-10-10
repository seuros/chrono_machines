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
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
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
    #[must_use]
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
    #[must_use]
    pub fn all(&self) -> Vec<(String, BackoffPolicy)> {
        self.entries.clone()
    }

    /// Clear the registry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(feature = "std")]
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Process-wide registry behind the `*_global_*` functions.
#[cfg(feature = "std")]
static GLOBAL_POLICIES: RwLock<PolicyRegistry> = RwLock::new(PolicyRegistry::new());

// Poisoning is deliberately ignored by both accessors. No `PolicyRegistry`
// method can unwind half-way through a mutation, so a lock poisoned by an
// unrelated panic still guards a consistent registry; refusing service would
// only turn that one panic into a panic on every later call.
#[cfg(feature = "std")]
fn read_global() -> RwLockReadGuard<'static, PolicyRegistry> {
    GLOBAL_POLICIES
        .read()
        .unwrap_or_else(PoisonError::into_inner)
}

#[cfg(feature = "std")]
fn write_global() -> RwLockWriteGuard<'static, PolicyRegistry> {
    GLOBAL_POLICIES
        .write()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Register a policy in the global registry (requires `std`).
#[cfg(feature = "std")]
pub fn register_global_policy(
    name: impl Into<String>,
    policy: BackoffPolicy,
) -> Option<BackoffPolicy> {
    // Allocate the key before taking the write lock, not inside it.
    let name = name.into();
    write_global().register(name, policy)
}

/// Fetch a policy from the global registry (requires `std`).
#[cfg(feature = "std")]
#[must_use]
pub fn get_global_policy(name: &str) -> Option<BackoffPolicy> {
    read_global().get(name)
}

/// Remove a policy from the global registry (requires `std`).
#[cfg(feature = "std")]
#[expect(
    clippy::must_use_candidate,
    reason = "removal is the point; discarding the returned policy is normal use"
)]
pub fn remove_global_policy(name: &str) -> Option<BackoffPolicy> {
    write_global().remove(name)
}

/// List all policies from the global registry (requires `std`).
#[cfg(feature = "std")]
#[must_use]
pub fn list_global_policies() -> Vec<(String, BackoffPolicy)> {
    read_global().all()
}

/// Clear all entries from the global registry (requires `std`).
#[cfg(feature = "std")]
pub fn clear_global_policies() {
    write_global().clear();
}

#[cfg(test)]
mod tests;
