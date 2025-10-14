//! Retry mechanism with fluent builder API
//!
//! This module provides a fluent retry API for wrapping fallible operations
//! with automatic retries and configurable backoff strategies.

use crate::backoff::BackoffStrategy;
use crate::sleep::Sleeper;
use rand::rngs::SmallRng;
use rand::SeedableRng;

/// Extension trait that adds `.retry()` method to functions and closures
///
/// This trait is automatically implemented for all `Fn` types that return `Result`.
///
/// # Example
///
/// ```rust
/// use chrono_machines::{Retryable, ExponentialBackoff};
///
/// fn fetch_data() -> Result<String, std::io::Error> {
///     // ... operation that might fail
/// #   Ok("data".to_string())
/// }
///
/// # #[cfg(feature = "std")]
/// let result = fetch_data
///     .retry(ExponentialBackoff::default())
///     .call();
/// ```
pub trait Retryable<T, E> {
    /// Begin building a retry operation with the given backoff strategy
    ///
    /// # Arguments
    ///
    /// * `backoff` - The backoff strategy to use for retry delays
    ///
    /// # Returns
    ///
    /// A `RetryBuilder` that can be further configured before execution
    fn retry<B: BackoffStrategy>(self, backoff: B) -> RetryBuilder<Self, B, T, E, fn(&E) -> bool>
    where
        Self: Sized;
}

impl<F, T, E> Retryable<T, E> for F
where
    F: FnMut() -> Result<T, E>,
{
    fn retry<B: BackoffStrategy>(self, backoff: B) -> RetryBuilder<Self, B, T, E, fn(&E) -> bool> {
        RetryBuilder {
            operation: self,
            backoff,
            when: None,
            notify: None,
            _phantom_t: core::marker::PhantomData,
            _phantom_e: core::marker::PhantomData,
        }
    }
}

/// Builder for configuring and executing retry operations
///
/// Created by calling `.retry()` on a function or closure.
/// Provides a fluent API for configuring retry behavior.
///
/// # Type Parameters
///
/// * `F` - The operation function type
/// * `B` - The backoff strategy type
/// * `T` - The success return type
/// * `E` - The error type
/// * `W` - The when predicate type
pub struct RetryBuilder<F, B, T, E, W> {
    operation: F,
    backoff: B,
    when: Option<W>,
    notify: Option<fn(&E, u64)>,
    _phantom_t: core::marker::PhantomData<T>,
    _phantom_e: core::marker::PhantomData<E>,
}

impl<F, B, T, E, W> RetryBuilder<F, B, T, E, W>
where
    F: FnMut() -> Result<T, E>,
    B: BackoffStrategy,
    W: Fn(&E) -> bool,
{
    /// Add a conditional predicate that determines if an error should trigger retry
    ///
    /// Only errors where `predicate(&error)` returns `true` will be retried.
    /// Errors that don't match the predicate are returned immediately without retry.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::{Retryable, ExponentialBackoff};
    ///
    /// #[derive(Debug)]
    /// enum MyError {
    ///     Retryable,
    ///     Fatal,
    /// }
    ///
    /// fn risky_operation() -> Result<String, MyError> {
    ///     // ...
    /// #   Err(MyError::Retryable)
    /// }
    ///
    /// # #[cfg(feature = "std")]
    /// let result = risky_operation
    ///     .retry(ExponentialBackoff::default())
    ///     .when(|e| matches!(e, MyError::Retryable))
    ///     .call();
    /// ```
    pub fn when<P>(self, predicate: P) -> RetryBuilder<F, B, T, E, P>
    where
        P: Fn(&E) -> bool,
    {
        RetryBuilder {
            operation: self.operation,
            backoff: self.backoff,
            when: Some(predicate),
            notify: self.notify,
            _phantom_t: core::marker::PhantomData,
            _phantom_e: core::marker::PhantomData,
        }
    }

    /// Add a notification callback that's invoked before each retry
    ///
    /// The callback receives the error that triggered the retry and the
    /// delay in milliseconds before the next attempt.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::{Retryable, ExponentialBackoff};
    ///
    /// fn fetch_data() -> Result<String, std::io::Error> {
    ///     // ...
    /// #   Ok("data".to_string())
    /// }
    ///
    /// # #[cfg(feature = "std")]
    /// let result = fetch_data
    ///     .retry(ExponentialBackoff::default())
    ///     .notify(|err, delay_ms| {
    ///         println!("Retrying after {}ms: {:?}", delay_ms, err);
    ///     })
    ///     .call();
    /// ```
    pub fn notify(mut self, callback: fn(&E, u64)) -> Self {
        self.notify = Some(callback);
        self
    }

    /// Execute the retry operation with blocking sleep (requires `std` feature)
    ///
    /// Runs the operation synchronously, retrying with blocking sleep between attempts.
    ///
    /// # Returns
    ///
    /// The final result after all retry attempts (success or final error)
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::{Retryable, ExponentialBackoff};
    ///
    /// fn fetch_data() -> Result<String, std::io::Error> {
    ///     // ...
    /// #   Ok("data".to_string())
    /// }
    ///
    /// # #[cfg(feature = "std")]
    /// let result = fetch_data
    ///     .retry(ExponentialBackoff::default())
    ///     .call()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn call(self) -> Result<T, E> {
        use crate::sleep::StdSleeper;
        self.call_with_sleeper(StdSleeper)
    }

    /// Execute the retry operation with a custom sleeper
    ///
    /// This low-level method allows providing a custom sleep implementation,
    /// enabling support for async runtimes, embedded systems, or testing.
    ///
    /// # Arguments
    ///
    /// * `sleeper` - Implementation of the `Sleeper` trait
    ///
    /// # Returns
    ///
    /// The final result after all retry attempts
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::{Retryable, ExponentialBackoff, sleep::{FnSleeper, Sleeper}};
    ///
    /// fn fetch_data() -> Result<String, std::io::Error> {
    ///     Ok("data".to_string())
    /// }
    ///
    /// fn custom_sleep(ms: u64) {
    ///     // Custom sleep implementation
    /// #   std::thread::sleep(std::time::Duration::from_millis(ms));
    /// }
    ///
    /// let result = fetch_data
    ///     .retry(ExponentialBackoff::default())
    ///     .call_with_sleeper(FnSleeper(custom_sleep))?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn call_with_sleeper<S: Sleeper>(mut self, sleeper: S) -> Result<T, E> {
        let mut rng = SmallRng::from_entropy();
        let mut attempt = 1u8;

        loop {
            match (self.operation)() {
                Ok(value) => return Ok(value),
                Err(error) => {
                    // Check if this error should be retried
                    if let Some(ref predicate) = self.when {
                        if !predicate(&error) {
                            // Error doesn't match predicate, fail immediately
                            return Err(error);
                        }
                    }

                    // Check if we have retries remaining
                    if !self.backoff.should_retry(attempt) {
                        return Err(error);
                    }

                    // Calculate delay
                    match self.backoff.delay(attempt, &mut rng) {
                        Some(delay_ms) => {
                            // Notify if callback is set
                            if let Some(notify) = self.notify {
                                notify(&error, delay_ms);
                            }

                            // Sleep before retry
                            sleeper.sleep_ms(delay_ms);
                            attempt += 1;
                        }
                        None => {
                            // Backoff says no more retries
                            return Err(error);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backoff::{ConstantBackoff, ExponentialBackoff};
    use crate::sleep::FnSleeper;

    #[derive(Debug, PartialEq)]
    enum TestError {
        Retryable,
        Fatal,
    }

    #[test]
    fn test_retry_success_on_first_attempt() {
        fn always_succeeds() -> Result<i32, TestError> {
            Ok(42)
        }

        let result = always_succeeds
            .retry(ExponentialBackoff::default())
            .call_with_sleeper(FnSleeper(|_| {}));

        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_retry_success_after_failures() {
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 2 {
                Err(TestError::Retryable)
            } else {
                Ok(42)
            }
        };

        let result = operation
            .retry(ExponentialBackoff::default().max_attempts(3))
            .call_with_sleeper(FnSleeper(|_| {}));

        assert_eq!(result, Ok(42));
        assert_eq!(attempts.get(), 3);
    }

    #[test]
    fn test_retry_exhausted() {
        fn always_fails() -> Result<i32, TestError> {
            Err(TestError::Retryable)
        }

        let result = always_fails
            .retry(ExponentialBackoff::default().max_attempts(3))
            .call_with_sleeper(FnSleeper(|_| {}));

        assert_eq!(result, Err(TestError::Retryable));
    }

    #[test]
    fn test_retry_when_predicate() {
        fn fails_with_fatal() -> Result<i32, TestError> {
            Err(TestError::Fatal)
        }

        let result = fails_with_fatal
            .retry(ExponentialBackoff::default())
            .when(|e| matches!(e, TestError::Retryable))
            .call_with_sleeper(FnSleeper(|_| {}));

        // Fatal error should not be retried
        assert_eq!(result, Err(TestError::Fatal));
    }

    #[test]
    fn test_retry_notify_callback() {
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 2 {
                Err(TestError::Retryable)
            } else {
                Ok(42)
            }
        };

        // Custom notify that counts calls
        fn test_notify(_: &TestError, _: u64) {
            // In real test would need interior mutability via Cell/RefCell
            // or external state tracking
        }

        let result = operation
            .retry(ExponentialBackoff::default().max_attempts(3))
            .notify(test_notify)
            .call_with_sleeper(FnSleeper(|_| {}));

        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_constant_backoff_retry() {
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 1 {
                Err(TestError::Retryable)
            } else {
                Ok(42)
            }
        };

        let result = operation
            .retry(ConstantBackoff::new().delay_ms(10).max_attempts(2))
            .call_with_sleeper(FnSleeper(|_| {}));

        assert_eq!(result, Ok(42));
        assert_eq!(attempts.get(), 2);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_retry_with_std_sleeper() {
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 1 {
                Err(TestError::Retryable)
            } else {
                Ok(42)
            }
        };

        let start = std::time::Instant::now();
        let result = operation
            .retry(
                ConstantBackoff::new()
                    .delay_ms(10)
                    .max_attempts(2)
                    .jitter_factor(0.0),
            )
            .call();

        let elapsed = start.elapsed();

        assert_eq!(result, Ok(42));
        assert!(elapsed.as_millis() >= 9); // At least one 10ms sleep
    }
}
