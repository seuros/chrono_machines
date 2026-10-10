//! Ruby FFI binding for `chrono_machines`
//!
//! This crate provides a Magnus-based Ruby binding for the `chrono_machines` library.
//! It exposes a simple helper function for calculating delays with exponential backoff.

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
    let exponent = i32::from(attempt_u8(attempt).saturating_sub(1));

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

    // Fibonacci numbers pass 2^53 at n = 79; from there the cast rounds to the
    // nearest f64, an error far below anything a delay in seconds can express.
    #[expect(
        clippy::cast_precision_loss,
        reason = "u64 -> f64 has no lossless conversion; rounding only starts above 2^53"
    )]
    let fib = fibonacci(attempt_u8(attempt)) as f64;
    let base = (base_delay * fib).min(max_delay);

    apply_jitter(base, jitter_factor)
}

/// Clamp a Ruby attempt number into the `1..=255` range the core works in.
fn attempt_u8(attempt: i64) -> u8 {
    // The clamp makes the conversion infallible; `unwrap_or` only spares the
    // FFI boundary a panic path.
    u8::try_from(attempt.clamp(1, i64::from(u8::MAX))).unwrap_or(u8::MAX)
}

/// Normalize jitter factor to [0.0, 1.0] range
const fn normalize_jitter(jitter_factor: f64) -> f64 {
    if jitter_factor.is_nan() {
        1.0
    } else {
        jitter_factor.clamp(0.0, 1.0)
    }
}

/// Apply jitter to a base delay value
fn apply_jitter(base: f64, jitter_factor: f64) -> f64 {
    let random_scalar: f64 = RNG.with_borrow_mut(|rng| rng.random_range(0.0..=1.0));
    // Kept as separate mul + add rather than `mul_add`: results stay
    // bit-identical to earlier releases, and on targets without hardware FMA
    // (the x86_64 baseline) `mul_add` is a libm call, not a speed-up.
    #[expect(
        clippy::suboptimal_flops,
        reason = "bit-identical jitter; mul_add is a libm call without hardware FMA"
    )]
    let jitter_blend = 1.0 - jitter_factor + random_scalar * jitter_factor;
    base * jitter_blend
}

/// Initialize the Ruby extension
#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    // Must precede every method definition: Ruby marks methods Ractor-safe as
    // they are defined. The RNG is thread-local, so Ractors never share it.
    // SAFETY: called on the loading thread during extension initialisation,
    // which is the only context `rb_ext_ractor_safe` is specified for.
    #[allow(unsafe_code)]
    unsafe {
        rb_sys::rb_ext_ractor_safe(true);
    }

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
