//! Ruby FFI binding for chrono_machines
//!
//! This crate provides a Magnus-based Ruby binding for the chrono_machines library.
//! It exposes a simple helper function for calculating delays with exponential backoff.

#![warn(rust_2024_compatibility)]
#![warn(clippy::all)]

use chrono_machines::fibonacci;
use magnus::{Error, Ruby, function};
use rand::RngExt;
use rand::rngs::StdRng;
use std::cell::RefCell;

// Thread-local RNG for performance (avoids reseeding from entropy on every call)
thread_local! {
    static RNG: RefCell<StdRng> = RefCell::new(rand::make_rng());
}

/// Calculate delay using exponential backoff with configurable jitter
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
fn calculate_delay_exponential(
    attempt: i64,
    base_delay: f64,
    multiplier: f64,
    max_delay: f64,
    jitter_factor: f64,
) -> f64 {
    let jitter_factor = normalize_jitter(jitter_factor);
    let attempt_u8 = attempt.clamp(1, 255) as u8;
    let exponent = attempt_u8.saturating_sub(1) as i32;

    let base_exponential = base_delay * multiplier.powi(exponent);
    let capped = base_exponential.min(max_delay);

    apply_jitter(capped, jitter_factor)
}

/// Calculate delay using constant backoff with optional jitter
///
/// # Arguments
/// * `_attempt` - The current attempt number (unused for constant backoff)
/// * `delay` - Constant delay in seconds
/// * `jitter_factor` - Jitter multiplier (0.0 = no jitter, 1.0 = full jitter)
///
/// # Returns
/// Constant delay with jitter applied
fn calculate_delay_constant(_attempt: i64, delay: f64, jitter_factor: f64) -> f64 {
    let jitter_factor = normalize_jitter(jitter_factor);
    apply_jitter(delay, jitter_factor)
}

/// Calculate delay using Fibonacci backoff with optional jitter
///
/// # Arguments
/// * `attempt` - The current attempt number (1-indexed)
/// * `base_delay` - Base delay in seconds (multiplied by Fibonacci number)
/// * `max_delay` - Maximum delay cap in seconds
/// * `jitter_factor` - Jitter multiplier (0.0 = no jitter, 1.0 = full jitter)
///
/// # Returns
/// Fibonacci-based delay with jitter applied
fn calculate_delay_fibonacci(
    attempt: i64,
    base_delay: f64,
    max_delay: f64,
    jitter_factor: f64,
) -> f64 {
    let jitter_factor = normalize_jitter(jitter_factor);
    let attempt_u8 = attempt.clamp(1, 255) as u8;

    let fib = fibonacci(attempt_u8);
    let base = (base_delay * fib as f64).min(max_delay);

    apply_jitter(base, jitter_factor)
}

/// Normalize jitter factor to [0.0, 1.0] range
fn normalize_jitter(jitter_factor: f64) -> f64 {
    if jitter_factor.is_nan() {
        1.0
    } else {
        jitter_factor.clamp(0.0, 1.0)
    }
}

/// Apply jitter to a base delay value
fn apply_jitter(base: f64, jitter_factor: f64) -> f64 {
    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        let random_scalar: f64 = rng.random_range(0.0..=1.0);
        let jitter_blend = 1.0 - jitter_factor + random_scalar * jitter_factor;
        base * jitter_blend
    })
}

/// Initialize the Ruby extension
#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    // Create ChronoMachinesNative module
    let module = ruby.define_module("ChronoMachinesNative")?;

    // Expose backoff strategy functions
    module.define_module_function(
        "exponential_delay",
        function!(calculate_delay_exponential, 5),
    )?;
    module.define_module_function("constant_delay", function!(calculate_delay_constant, 3))?;
    module.define_module_function("fibonacci_delay", function!(calculate_delay_fibonacci, 4))?;

    // Backward compatibility: alias old name to exponential
    module.define_module_function("calculate_delay", function!(calculate_delay_exponential, 5))?;

    Ok(())
}

#[cfg(test)]
mod tests;
