//! Ruby FFI binding for chrono_machines
//!
//! This crate provides a Magnus-based Ruby binding for the chrono_machines library.
//! It exposes a simple helper function for calculating delays with exponential backoff.

#![warn(rust_2024_compatibility)]
#![warn(clippy::all)]

use magnus::{function, Error, Ruby};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::cell::RefCell;

// Thread-local RNG for performance (avoids reseeding from entropy on every call)
thread_local! {
    static RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_os_rng());
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

/// Calculate the nth Fibonacci number (1-indexed)
fn fibonacci(n: u8) -> u64 {
    match n {
        0 => 0,
        1 | 2 => 1,
        _ => {
            let mut a = 1u64;
            let mut b = 1u64;
            for _ in 2..n {
                let next = a.saturating_add(b);
                a = b;
                b = next;
            }
            b
        }
    }
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
mod tests {
    use super::*;

    #[test]
    fn test_exponential_delay() {
        RNG.with(|rng| {
            *rng.borrow_mut() = SmallRng::seed_from_u64(1337);
        });

        let delay = calculate_delay_exponential(1, 0.0004, 1.0, 0.001, 0.5);

        assert!(
            delay >= 0.0002 && delay <= 0.0004,
            "expected delay in [0.0002, 0.0004], got {delay}"
        );
    }

    #[test]
    fn test_constant_delay() {
        RNG.with(|rng| {
            *rng.borrow_mut() = SmallRng::seed_from_u64(42);
        });

        // Constant delay with 10% jitter should be 90-100% of base
        let delay = calculate_delay_constant(5, 1.0, 0.1);
        assert!(delay >= 0.9 && delay <= 1.0, "got {delay}");

        // No jitter should return exact value
        let delay = calculate_delay_constant(3, 0.5, 0.0);
        assert_eq!(delay, 0.5);
    }

    #[test]
    fn test_fibonacci_delay() {
        RNG.with(|rng| {
            *rng.borrow_mut() = SmallRng::seed_from_u64(123);
        });

        // Fibonacci sequence: 1, 1, 2, 3, 5, 8, 13...
        // Attempt 1: base_delay * 1 = 0.1
        let delay1 = calculate_delay_fibonacci(1, 0.1, 10.0, 0.0);
        assert_eq!(delay1, 0.1);

        // Attempt 5: base_delay * 5 = 0.5
        let delay5 = calculate_delay_fibonacci(5, 0.1, 10.0, 0.0);
        assert_eq!(delay5, 0.5);

        // Attempt 8: base_delay * 21 = 2.1
        let delay8 = calculate_delay_fibonacci(8, 0.1, 10.0, 0.0);
        assert_eq!(delay8, 2.1);
    }

    #[test]
    fn test_fibonacci_sequence() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(2), 1);
        assert_eq!(fibonacci(3), 2);
        assert_eq!(fibonacci(4), 3);
        assert_eq!(fibonacci(5), 5);
        assert_eq!(fibonacci(6), 8);
        assert_eq!(fibonacci(7), 13);
        assert_eq!(fibonacci(8), 21);
    }
}
