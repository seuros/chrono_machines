//! Blocking retry example
//!
//! Demonstrates using chrono_machines for synchronous retry operations
//! with the standard library sleeper.
//!
//! Run with: cargo run --example blocking_retry --features std

use chrono_machines::{ConstantBackoff, ExponentialBackoff, FibonacciBackoff, Retryable};

#[derive(Debug)]
enum ApiError {
    Timeout,
    RateLimited,
    ServerError,
    NotFound,
}

fn main() {
    println!("=== ChronoMachines Blocking Retry Examples ===\n");

    // Example 1: Exponential backoff with success after retries
    println!("1. Exponential Backoff - Success after retries:");
    let mut attempt_count = 0;
    let result = (|| {
        attempt_count += 1;
        println!("   Attempt {}", attempt_count);

        if attempt_count < 3 {
            Err(ApiError::Timeout)
        } else {
            Ok("Success!")
        }
    })
    .retry(
        ExponentialBackoff::new()
            .base_delay_ms(100)
            .multiplier(2.0)
            .max_attempts(5)
            .jitter_factor(0.1), // 10% jitter
    )
    .notify(|err, delay_ms| {
        println!("   → Retrying after {}ms due to: {:?}", delay_ms, err);
    })
    .call();

    println!("   Result: {:?}\n", result);

    // Example 2: Constant backoff with conditional retry
    println!("2. Constant Backoff - Conditional retry (only timeout):");
    attempt_count = 0;
    let result: Result<&str, _> = (|| {
        attempt_count += 1;
        println!("   Attempt {}", attempt_count);

        if attempt_count == 1 {
            Err(ApiError::Timeout)
        } else {
            Err(ApiError::NotFound) // Non-retryable
        }
    })
    .retry(
        ConstantBackoff::new()
            .delay_ms(50)
            .max_attempts(3)
            .jitter_factor(0.0),
    )
    .when(|e| matches!(e, ApiError::Timeout | ApiError::RateLimited))
    .notify(|err, delay_ms| {
        println!("   → Retrying after {}ms due to: {:?}", delay_ms, err);
    })
    .call();

    println!("   Result: {:?}\n", result);

    // Example 3: Fibonacci backoff
    println!("3. Fibonacci Backoff - Progressive delays:");
    attempt_count = 0;
    let result = (|| {
        attempt_count += 1;
        println!("   Attempt {}", attempt_count);

        if attempt_count < 4 {
            Err(ApiError::ServerError)
        } else {
            Ok(42)
        }
    })
    .retry(
        FibonacciBackoff::new()
            .base_delay_ms(50) // 50ms, 50ms, 100ms, 150ms, 250ms...
            .max_attempts(5)
            .jitter_factor(0.0), // No jitter for predictable timing
    )
    .notify(|err, delay_ms| {
        println!("   → Retrying after {}ms due to: {:?}", delay_ms, err);
    })
    .call();

    println!("   Result: {:?}\n", result);

    // Example 4: Retry exhausted
    println!("4. Retry Exhausted - Max attempts reached:");
    attempt_count = 0;
    let result: Result<&str, _> = (|| {
        attempt_count += 1;
        println!("   Attempt {}", attempt_count);
        Err(ApiError::ServerError)
    })
    .retry(
        ExponentialBackoff::new()
            .base_delay_ms(10)
            .max_attempts(3)
            .jitter_factor(0.0),
    )
    .notify(|err, delay_ms| {
        println!("   → Retrying after {}ms due to: {:?}", delay_ms, err);
    })
    .call();

    println!("   Result: {:?}", result);

    println!("\n=== All examples completed ===");
}
