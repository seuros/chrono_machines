//! Sleep abstraction for no_std compatibility
//!
//! This module provides traits and implementations for sleeping/delaying
//! in various environments (std, async, embedded).

/// Trait for sleep/delay implementations
///
/// This trait abstracts sleep operations to support different runtime environments:
/// - Standard library blocking sleep
/// - Tokio/async-std async sleep
/// - Embassy timer for embedded
/// - Custom implementations
pub trait Sleeper {
    /// Sleep for the specified number of milliseconds
    fn sleep_ms(&self, ms: u64);
}

/// Standard library sleeper using `std::thread::sleep`
///
/// Only available when the `std` feature is enabled.
///
/// # Example
///
/// ```rust
/// use chrono_machines::sleep::StdSleeper;
/// use chrono_machines::sleep::Sleeper;
///
/// let sleeper = StdSleeper;
/// sleeper.sleep_ms(100); // Sleep for 100ms
/// ```
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy)]
pub struct StdSleeper;

#[cfg(feature = "std")]
impl Sleeper for StdSleeper {
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// Function pointer sleeper for custom sleep implementations
///
/// Wraps a function pointer that takes milliseconds and performs sleep.
/// Useful for async runtimes or testing.
///
/// # Example
///
/// ```rust
/// use chrono_machines::sleep::{FnSleeper, Sleeper};
///
/// // Custom sleep function
/// fn my_sleep(ms: u64) {
///     // Custom implementation
///     std::thread::sleep(std::time::Duration::from_millis(ms));
/// }
///
/// let sleeper = FnSleeper(my_sleep);
/// sleeper.sleep_ms(100);
/// ```
#[derive(Clone, Copy)]
pub struct FnSleeper(pub fn(u64));

impl Sleeper for FnSleeper {
    fn sleep_ms(&self, ms: u64) {
        (self.0)(ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_std_sleeper() {
        let sleeper = StdSleeper;
        let start = std::time::Instant::now();
        sleeper.sleep_ms(10);
        let elapsed = start.elapsed();

        // Allow some margin for timing precision
        assert!(elapsed.as_millis() >= 9 && elapsed.as_millis() <= 20);
    }

    #[test]
    fn test_fn_sleeper() {
        fn test_sleep(ms: u64) {
            // In a real test, we'd need interior mutability
            // For this test, we just verify it compiles and runs
            assert!(ms > 0);
        }

        let sleeper = FnSleeper(test_sleep);
        sleeper.sleep_ms(100);
    }
}
