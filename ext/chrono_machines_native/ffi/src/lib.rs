//! Ruby FFI binding for chrono_machines
//!
//! This crate provides a Magnus-based Ruby binding for the chrono_machines library.
//! It exposes a simple helper function for calculating delays with exponential backoff.

use chrono_machines::Policy;
use magnus::{function, Error, Ruby};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use std::cell::RefCell;

// Thread-local RNG for performance (avoids reseeding from entropy on every call)
thread_local! {
    static RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_entropy());
}

/// Calculate delay using exponential backoff with configurable jitter
///
/// Delegates to chrono_machines::Policy for consistent behavior with the Rust library.
///
/// # Arguments
/// * `attempt` - The current attempt number (1-indexed)
/// * `base_delay` - Base delay in seconds
/// * `multiplier` - Exponential multiplier
/// * `max_delay` - Maximum delay cap in seconds
/// * `jitter_factor` - Jitter multiplier (0.0 = no jitter, 1.0 = full jitter)
///
/// # Returns
/// Calculated delay in seconds with jitter applied
fn calculate_delay_native(
    attempt: i64,
    base_delay: f64,
    multiplier: f64,
    max_delay: f64,
    jitter_factor: f64,
) -> f64 {
    // Convert seconds to milliseconds for Rust core
    let base_delay_ms = (base_delay * 1000.0) as u64;
    let max_delay_ms = (max_delay * 1000.0) as u64;

    // Create policy
    let policy = Policy {
        max_attempts: 255, // Not used in delay calculation
        base_delay_ms,
        multiplier,
        max_delay_ms,
    };

    // Calculate delay using thread-local RNG
    let attempt_u8 = attempt.min(255).max(1) as u8;
    let delay_ms = RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        policy.calculate_delay_with_rng(attempt_u8, jitter_factor, &mut *rng)
    });

    // Convert milliseconds back to seconds for Ruby
    delay_ms as f64 / 1000.0
}

/// Initialize the Ruby extension
#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    // Create ChronoMachinesNative module
    let module = ruby.define_module("ChronoMachinesNative")?;

    // Expose the calculate_delay helper function
    module.define_module_function("calculate_delay", function!(calculate_delay_native, 5))?;

    Ok(())
}
