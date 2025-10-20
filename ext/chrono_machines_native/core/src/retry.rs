//! Retry mechanism with fluent builder API
//!
//! This module provides a fluent retry API for wrapping fallible operations
//! with automatic retries and configurable backoff strategies.

use crate::backoff::BackoffStrategy;
use crate::sleep::Sleeper;
use core::fmt;
use rand::SeedableRng;
use rand::rngs::SmallRng;

/// Reason why a retry operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryErrorKind {
    /// The operation exhausted all retry attempts.
    Exhausted,
    /// The error was rejected by the `when` predicate.
    PredicateRejected,
}

/// Rich retry error that carries execution context.
#[derive(Debug, Clone)]
pub struct RetryError<E> {
    kind: RetryErrorKind,
    attempts: u8,
    max_attempts: u8,
    last_delay_ms: Option<u64>,
    cause: Option<E>,
}

impl<E> RetryError<E> {
    fn new(
        kind: RetryErrorKind,
        attempts: u8,
        max_attempts: u8,
        last_delay_ms: Option<u64>,
        cause: Option<E>,
    ) -> Self {
        Self {
            kind,
            attempts,
            max_attempts,
            last_delay_ms,
            cause,
        }
    }

    /// Retrieve the underlying cause when available.
    pub fn cause(&self) -> Option<&E> {
        self.cause.as_ref()
    }

    /// Consume the error and return the underlying cause when available.
    pub fn into_cause(self) -> Option<E> {
        self.cause
    }

    /// Attempt number that produced the terminal outcome (1-indexed).
    pub fn attempts(&self) -> u8 {
        self.attempts
    }

    /// Maximum attempts allowed by the policy.
    pub fn max_attempts(&self) -> u8 {
        self.max_attempts
    }

    /// Delay used before the last attempt (if any).
    pub fn last_delay_ms(&self) -> Option<u64> {
        self.last_delay_ms
    }

    /// Error category.
    pub fn kind(&self) -> RetryErrorKind {
        self.kind
    }
}

impl<E> fmt::Display for RetryError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            RetryErrorKind::Exhausted => {
                write!(
                    f,
                    "retry exhausted after {} of {} attempts",
                    self.attempts, self.max_attempts
                )?;
            }
            RetryErrorKind::PredicateRejected => {
                write!(f, "retry aborted by predicate on attempt {}", self.attempts)?;
            }
        }

        if let Some(delay) = self.last_delay_ms {
            write!(f, " (last delay {}ms)", delay)?;
        }

        if let Some(cause) = self.cause.as_ref() {
            write!(f, ": {}", cause)?;
        }

        Ok(())
    }
}

#[cfg(feature = "std")]
impl<E> std::error::Error for RetryError<E> where E: std::error::Error {}

/// Successful retry result holding metadata about the execution.
#[derive(Debug)]
pub struct RetryOutcome<T> {
    value: T,
    attempts: u8,
    cumulative_delay_ms: u64,
}

impl<T> RetryOutcome<T> {
    fn new(value: T, attempts: u8, cumulative_delay_ms: u64) -> Self {
        Self {
            value,
            attempts,
            cumulative_delay_ms,
        }
    }

    /// Attempt that succeeded (1-indexed).
    pub fn attempts(&self) -> u8 {
        self.attempts
    }

    /// Total milliseconds spent sleeping between attempts.
    pub fn cumulative_delay_ms(&self) -> u64 {
        self.cumulative_delay_ms
    }

    /// Borrow the successful value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Consume the outcome and return the successful value.
    pub fn into_inner(self) -> T {
        self.value
    }
}

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
/// let outcome = fetch_data
///     .retry(ExponentialBackoff::default())
///     .call()
///     .expect("retry succeeded");
/// assert!(outcome.attempts() >= 1);
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
            on_success: None,
            on_failure: None,
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
    notify: Option<fn(&E, u8, u64)>,
    on_success: Option<fn(&T, u8)>,
    on_failure: Option<fn(&RetryError<E>)>,
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
            on_success: self.on_success,
            on_failure: self.on_failure,
            _phantom_t: core::marker::PhantomData,
            _phantom_e: core::marker::PhantomData,
        }
    }

    /// Add a notification callback that's invoked before each retry
    ///
    /// The callback receives the error that triggered the retry, the attempt
    /// number that just failed, and the delay in milliseconds before the next attempt.
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
    ///     .notify(|err, attempt, delay_ms| {
    ///         println!(
    ///             "Attempt {} failed, retrying after {}ms: {:?}",
    ///             attempt, delay_ms, err
    ///         );
    ///     })
    ///     .call();
    /// ```
    pub fn notify(mut self, callback: fn(&E, u8, u64)) -> Self {
        self.notify = Some(callback);
        self
    }

    /// Execute a callback after a successful attempt.
    ///
    /// The callback receives the successful value and the attempt number that
    /// succeeded (1-indexed).
    pub fn on_success(mut self, callback: fn(&T, u8)) -> Self {
        self.on_success = Some(callback);
        self
    }

    /// Execute a callback when the retry process terminates with failure.
    ///
    /// The callback receives the rich [`RetryError`] describing the failure.
    pub fn on_failure(mut self, callback: fn(&RetryError<E>)) -> Self {
        self.on_failure = Some(callback);
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
    /// let outcome = fetch_data
    ///     .retry(ExponentialBackoff::default())
    ///     .call()?;
    /// println!("Succeeded after {} attempts", outcome.attempts());
    /// # Ok::<(), chrono_machines::RetryError<std::io::Error>>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn call(self) -> Result<RetryOutcome<T>, RetryError<E>> {
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
    /// let outcome = fetch_data
    ///     .retry(ExponentialBackoff::default())
    ///     .call_with_sleeper(FnSleeper(custom_sleep))?;
    /// println!("Total delay {}ms", outcome.cumulative_delay_ms());
    /// # Ok::<(), chrono_machines::RetryError<std::io::Error>>(())
    /// ```
    pub fn call_with_sleeper<S: Sleeper>(
        mut self,
        sleeper: S,
    ) -> Result<RetryOutcome<T>, RetryError<E>> {
        let mut rng = SmallRng::from_os_rng();
        let mut attempt = 1u8;
        let max_attempts = self.backoff.max_attempts();
        let mut cumulative_delay_ms: u64 = 0;
        let mut last_delay_ms: Option<u64> = None;

        loop {
            match (self.operation)() {
                Ok(value) => {
                    if let Some(callback) = self.on_success {
                        callback(&value, attempt);
                    }
                    return Ok(RetryOutcome::new(value, attempt, cumulative_delay_ms));
                }
                Err(error) => {
                    // Check if this error should be retried
                    if let Some(ref predicate) = self.when {
                        if !predicate(&error) {
                            // Error doesn't match predicate, fail immediately
                            let retry_error = RetryError::new(
                                RetryErrorKind::PredicateRejected,
                                attempt,
                                max_attempts,
                                last_delay_ms,
                                Some(error),
                            );
                            if let Some(callback) = self.on_failure {
                                callback(&retry_error);
                            }
                            return Err(retry_error);
                        }
                    }

                    // Check if we have retries remaining
                    if !self.backoff.should_retry(attempt) {
                        let retry_error = RetryError::new(
                            RetryErrorKind::Exhausted,
                            attempt,
                            max_attempts,
                            last_delay_ms,
                            Some(error),
                        );
                        if let Some(callback) = self.on_failure {
                            callback(&retry_error);
                        }
                        return Err(retry_error);
                    }

                    // Calculate delay
                    match self.backoff.delay(attempt, &mut rng) {
                        Some(delay_ms) => {
                            // Notify if callback is set
                            if let Some(notify) = self.notify {
                                notify(&error, attempt, delay_ms);
                            }

                            // Sleep before retry
                            sleeper.sleep_ms(delay_ms);
                            cumulative_delay_ms = cumulative_delay_ms.saturating_add(delay_ms);
                            last_delay_ms = Some(delay_ms);
                            attempt = attempt.saturating_add(1);
                        }
                        None => {
                            // Backoff says no more retries
                            let retry_error = RetryError::new(
                                RetryErrorKind::Exhausted,
                                attempt,
                                max_attempts,
                                last_delay_ms,
                                Some(error),
                            );
                            if let Some(callback) = self.on_failure {
                                callback(&retry_error);
                            }
                            return Err(retry_error);
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

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 1);
        assert_eq!(outcome.into_inner(), 42);
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

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 3);
        assert_eq!(outcome.into_inner(), 42);
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

        let err = result.expect_err("retry should exhaust");
        assert_eq!(err.kind(), RetryErrorKind::Exhausted);
        assert_eq!(err.attempts(), 3);
        assert_eq!(err.max_attempts(), 3);
        assert!(err.last_delay_ms().is_some());
        if let Some(cause) = err.cause() {
            assert_eq!(cause, &TestError::Retryable);
        } else {
            panic!("expected underlying cause");
        }
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
        let err = result.expect_err("retry should stop due to predicate");
        assert_eq!(err.kind(), RetryErrorKind::PredicateRejected);
        if let Some(cause) = err.cause() {
            assert_eq!(cause, &TestError::Fatal);
        } else {
            panic!("expected underlying cause");
        }
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
        fn test_notify(_: &TestError, attempt: u8, _: u64) {
            // In real test would need interior mutability via Cell/RefCell
            // or external state tracking
            assert!(attempt >= 1);
        }

        let result = operation
            .retry(ExponentialBackoff::default().max_attempts(3))
            .notify(test_notify)
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 3);
    }

    #[test]
    fn test_on_success_callback_invoked() {
        use core::cell::Cell;
        use core::sync::atomic::{AtomicUsize, Ordering};

        static SUCCESS_ATTEMPT: AtomicUsize = AtomicUsize::new(0);

        fn on_success(_: &i32, attempt: u8) {
            SUCCESS_ATTEMPT.store(attempt as usize, Ordering::SeqCst);
        }

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 1 {
                Err(TestError::Retryable)
            } else {
                Ok(7)
            }
        };

        SUCCESS_ATTEMPT.store(0, Ordering::SeqCst);

        let outcome = operation
            .retry(ExponentialBackoff::default().max_attempts(3))
            .on_success(on_success)
            .call_with_sleeper(FnSleeper(|_| {}))
            .expect("retry should succeed");

        assert_eq!(outcome.into_inner(), 7);
        assert_eq!(SUCCESS_ATTEMPT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_on_failure_callback_invoked() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static FAILURE_KIND: AtomicUsize = AtomicUsize::new(0);

        fn on_failure(err: &RetryError<TestError>) {
            let marker = match err.kind() {
                RetryErrorKind::Exhausted => 1,
                RetryErrorKind::PredicateRejected => 2,
            };
            FAILURE_KIND.store(marker, Ordering::SeqCst);
        }

        fn always_fails() -> Result<(), TestError> {
            Err(TestError::Retryable)
        }

        FAILURE_KIND.store(0, Ordering::SeqCst);

        let result = always_fails
            .retry(ExponentialBackoff::default().max_attempts(2))
            .on_failure(on_failure)
            .call_with_sleeper(FnSleeper(|_| {}));

        assert!(result.is_err());
        assert_eq!(FAILURE_KIND.load(Ordering::SeqCst), 1);
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

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 2);
        assert_eq!(outcome.into_inner(), 42);
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

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 2);
        assert_eq!(outcome.into_inner(), 42);
        assert!(elapsed.as_millis() >= 9); // At least one 10ms sleep
    }
}
