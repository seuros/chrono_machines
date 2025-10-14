//! Ruby FFI binding for chrono_machines
//!
//! This crate provides a Magnus-based Ruby binding for the chrono_machines library.
//! It exposes a simple helper function for calculating delays with exponential backoff.

use magnus::{function, Error, Ruby};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
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
    // Normalize jitter factor similar to the Ruby implementation
    let mut jitter_factor = jitter_factor;
    if jitter_factor.is_nan() {
        jitter_factor = 1.0;
    } else {
        jitter_factor = jitter_factor.clamp(0.0, 1.0);
    }

    let attempt_u8 = attempt.min(255).max(1) as u8;
    let exponent = attempt_u8.saturating_sub(1) as i32;

    // Calculate base exponential backoff directly in seconds to preserve precision
    let base_exponential = base_delay * multiplier.powi(exponent);
    let capped = base_exponential.min(max_delay);

    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        let random_scalar: f64 = rng.gen_range(0.0..=1.0);
        let jitter_blend = 1.0 - jitter_factor + random_scalar * jitter_factor;

        capped * jitter_blend
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_sub_millisecond_precision() {
        RNG.with(|rng| {
            *rng.borrow_mut() = SmallRng::seed_from_u64(1337);
        });

        let delay = calculate_delay_native(1, 0.0004, 1.0, 0.001, 0.5);

        assert!(
            delay >= 0.0002 && delay <= 0.0004,
            "expected delay in [0.0002, 0.0004], got {delay}"
        );
    }
}
