//! ChronoMachines - Pure Rust exponential backoff and retry library
//!
//! This crate provides a lightweight, `no_std` compatible implementation of
//! exponential backoff with full jitter for retry mechanisms.
//!
//! # Features
//!
//! - **Full Jitter**: Prevents thundering herd problem
//! - **no_std compatible**: Works in embedded environments
//! - **Zero allocation**: Uses stack-only data structures
//! - **Fast**: Minimal overhead for delay calculations
//!
//! # Example
//!
//! ```rust
//! use chrono_machines::Policy;
//!
//! let policy = Policy {
//!     max_attempts: 5,
//!     base_delay_ms: 100,
//!     multiplier: 2.0,
//!     max_delay_ms: 10_000,
//! };
//!
//! // Use full jitter (1.0) - recommended for distributed systems
//! let delay = policy.calculate_delay(1, 1.0);
//! println!("Wait {}ms before retry", delay);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(rust_2024_compatibility)]
#![warn(clippy::all)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod backoff;
#[cfg(feature = "std")]
pub mod dsl;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod policy;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod retry;
pub mod sleep;

/// The `rand` crate this one was built against.
///
/// [`BackoffStrategy::delay`] is generic over [`rand::Rng`], which puts that
/// trait in the public API: a downstream strategy cannot be written without
/// naming it. Re-exporting it here means implementors do not have to add their
/// own `rand` dependency and keep its version in lockstep with this crate's by
/// hand — a mismatch there produces two incompatible `Rng` traits and an error
/// that says nothing about the real cause.
///
/// ```rust
/// use chrono_machines::{rand::Rng, BackoffStrategy};
///
/// struct Immediate;
///
/// impl BackoffStrategy for Immediate {
///     fn delay<R: Rng>(&self, attempt: u8, _rng: &mut R) -> Option<u64> {
///         self.should_retry(attempt).then_some(0)
///     }
///     fn should_retry(&self, attempt: u8) -> bool { attempt < self.max_attempts() }
///     fn max_attempts(&self) -> u8 { 3 }
/// }
/// ```
pub use ::rand;

pub use backoff::{
    BackoffPolicy, BackoffStrategy, ConstantBackoff, ExponentialBackoff, FibonacciBackoff,
    fibonacci,
};
#[cfg(feature = "std")]
pub use dsl::{DslError, builder_for_policy, retry_with_policy};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use policy::PolicyRegistry;
#[cfg(feature = "std")]
pub use policy::{
    clear_global_policies, get_global_policy, list_global_policies, register_global_policy,
    remove_global_policy,
};
#[cfg(feature = "async")]
pub use retry::AsyncRetryable;
#[cfg(any(feature = "std", feature = "alloc"))]
pub use retry::{
    DelayHint, RetryBuilder, RetryContext, RetryError, RetryOutcome, Retryable, RetryableExt,
};
#[cfg(feature = "async")]
pub use sleep::AsyncSleeper;
#[cfg(feature = "std")]
pub use sleep::StdSleeper;
pub use sleep::{FnSleeper, Sleeper};

use rand::RngExt;
#[cfg(feature = "std")]
use rand::rngs::StdRng;

use rand::Rng;

/// Retry policy configuration
///
/// Defines the parameters for exponential backoff with jitter.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    /// Maximum number of retry attempts
    pub max_attempts: u8,

    /// Base delay in milliseconds
    pub base_delay_ms: u64,

    /// Exponential backoff multiplier
    pub multiplier: f64,

    /// Maximum delay cap in milliseconds
    pub max_delay_ms: u64,
}

impl Policy {
    /// Create a new policy with default values
    ///
    /// # Default values
    ///
    /// - `max_attempts`: 3
    /// - `base_delay_ms`: 100
    /// - `multiplier`: 2.0
    /// - `max_delay_ms`: 10_000
    pub fn new() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 10_000,
        }
    }

    /// Calculate delay with jitter for the given attempt
    ///
    /// Applies exponential backoff with configurable jitter to prevent
    /// thundering herd problems in distributed systems.
    ///
    /// # Arguments
    ///
    /// * `attempt` - Current attempt number (1-indexed)
    /// * `jitter_factor` - Jitter multiplier (0.0 = no jitter, 1.0 = full jitter)
    ///
    /// # Returns
    ///
    /// Delay in milliseconds as a `u64`
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::Policy;
    ///
    /// let policy = Policy::new();
    /// // Full jitter (default behavior)
    /// let delay = policy.calculate_delay(1, 1.0);
    /// assert!(delay <= 100); // First attempt, max is base_delay_ms
    ///
    /// // 10% jitter - delay will be 90-100% of base_delay_ms
    /// let delay = policy.calculate_delay(1, 0.1);
    /// assert!(delay >= 90 && delay <= 100);
    /// ```
    #[cfg(feature = "std")]
    pub fn calculate_delay(&self, attempt: u8, jitter_factor: f64) -> u64 {
        let mut rng: StdRng = rand::make_rng();
        self.calculate_delay_with_rng(attempt, jitter_factor, &mut rng)
    }

    /// Calculate delay with a provided RNG and custom jitter factor
    ///
    /// This method allows for custom RNG implementations and jitter control, useful for:
    /// - Deterministic testing
    /// - `no_std` environments with custom RNG sources
    /// - Performance optimization with specific RNG types
    /// - Fine-tuning jitter behavior
    ///
    /// # Arguments
    ///
    /// * `attempt` - Current attempt number (1-indexed)
    /// * `jitter_factor` - Jitter multiplier (0.0 = no jitter, 1.0 = full jitter)
    /// * `rng` - Random number generator implementing `Rng`
    ///
    /// # Returns
    ///
    /// Delay in milliseconds as a `u64`
    pub fn calculate_delay_with_rng<R: Rng>(
        &self,
        attempt: u8,
        jitter_factor: f64,
        rng: &mut R,
    ) -> u64 {
        // Normalize jitter factor to the inclusive range [0.0, 1.0]
        let mut jitter_factor = jitter_factor;
        if jitter_factor.is_nan() {
            jitter_factor = 1.0;
        } else {
            jitter_factor = jitter_factor.clamp(0.0, 1.0);
        }

        // Calculate base exponential backoff
        let exponent = attempt.saturating_sub(1) as i32;
        let base_exponential =
            (self.base_delay_ms as f64) * crate::backoff::powi_f64(self.multiplier, exponent);

        // Cap at max_delay
        let capped = base_exponential.min(self.max_delay_ms as f64);

        // Apply jitter: blend between deterministic and random delay
        // jitter_factor of 1.0 = full jitter (0 to base), 0.0 = no jitter (exactly base)
        // Formula: base * (1 - jitter_factor + rand * jitter_factor)
        // Example with jitter_factor=0.1: base * (0.9 + rand*0.1) = 90% to 100% of base
        let random_scalar: f64 = rng.random_range(0.0..=1.0);
        let jitter_blend = 1.0 - jitter_factor + random_scalar * jitter_factor;
        let jittered = capped * jitter_blend;

        jittered as u64
    }

    /// Check if another retry should be attempted
    ///
    /// # Arguments
    ///
    /// * `current_attempt` - Current attempt number (1-indexed)
    ///
    /// # Returns
    ///
    /// `true` if another retry is allowed, `false` otherwise
    pub fn should_retry(&self, current_attempt: u8) -> bool {
        current_attempt < self.max_attempts
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
