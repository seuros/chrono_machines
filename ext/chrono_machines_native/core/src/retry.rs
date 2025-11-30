//! Retry mechanism with fluent builder API
//!
//! This module provides a fluent retry API for wrapping fallible operations
//! with automatic retries and configurable backoff strategies.

use crate::backoff::BackoffStrategy;
use crate::sleep::Sleeper;
use core::fmt;
use rand::rngs::SmallRng;
use rand::SeedableRng;

/// Type alias for retry builder with default predicate
type DefaultRetryBuilder<F, B, T, E> = RetryBuilder<F, B, T, E, fn(&E) -> bool>;

/// Type alias for boxed notify callback
type NotifyCallback<E> = Box<dyn FnMut(&RetryContext<E>)>;

/// Type alias for boxed failure callback
type FailureCallback<E> = Box<dyn FnMut(&RetryError<E>)>;

/// Reason why a retry operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryErrorKind {
    /// The operation exhausted all retry attempts.
    Exhausted,
    /// The error was rejected by the `when` predicate.
    PredicateRejected,
}

/// Context provided to retry callbacks with observability data.
///
/// This struct provides comprehensive information about the current retry attempt,
/// including timing, delays, and error context.
#[derive(Debug)]
pub struct RetryContext<'a, E> {
    /// Current attempt number (1-indexed)
    pub attempt: u8,
    /// Delay in milliseconds before the next retry attempt (None on success or final failure)
    pub next_delay_ms: Option<u64>,
    /// Total milliseconds spent sleeping between attempts so far
    pub cumulative_delay_ms: u64,
    /// Reference to the error that triggered this retry (None on success)
    pub error: Option<&'a E>,
}

/// Rich retry error that carries execution context.
#[derive(Debug, Clone)]
pub struct RetryError<E> {
    kind: RetryErrorKind,
    attempts: u8,
    max_attempts: u8,
    cumulative_delay_ms: u64,
    cause: Option<E>,
}

impl<E> RetryError<E> {
    fn new(
        kind: RetryErrorKind,
        attempts: u8,
        max_attempts: u8,
        cumulative_delay_ms: u64,
        cause: Option<E>,
    ) -> Self {
        Self {
            kind,
            attempts,
            max_attempts,
            cumulative_delay_ms,
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

    /// Total time spent in delays before reaching terminal state.
    pub fn cumulative_delay_ms(&self) -> u64 {
        self.cumulative_delay_ms
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

        write!(f, " (cumulative delay {}ms)", self.cumulative_delay_ms)?;

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
    fn retry<B: BackoffStrategy>(self, backoff: B) -> DefaultRetryBuilder<Self, B, T, E>
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

/// Ergonomic extension methods for retry operations
///
/// This trait adds convenience methods that create retry builders with common
/// backoff strategies using their default configurations.
///
/// # Example
///
/// ```rust
/// use chrono_machines::RetryableExt;
///
/// fn fetch_data() -> Result<String, std::io::Error> {
///     // ... operation that might fail
/// #   Ok("data".to_string())
/// }
///
/// # #[cfg(feature = "std")]
/// // Before: fetch_data.retry(ExponentialBackoff::default()).call()?;
/// // After:  fetch_data.with_exponential().call()?;
/// let outcome = fetch_data.with_exponential().call()?;
/// # Ok::<(), chrono_machines::RetryError<std::io::Error>>(())
/// ```
pub trait RetryableExt<T, E>: Retryable<T, E> {
    /// Create a retry builder with exponential backoff using default configuration
    ///
    /// Default configuration:
    /// - max_attempts: 3
    /// - base_delay_ms: 100
    /// - multiplier: 2.0
    /// - max_delay_ms: 10_000
    /// - jitter_factor: 1.0 (full jitter)
    ///
    /// # Returns
    ///
    /// A `RetryBuilder` configured with `ExponentialBackoff::default()`
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::RetryableExt;
    ///
    /// fn fetch_api() -> Result<String, std::io::Error> {
    ///     Ok("response".to_string())
    /// }
    ///
    /// # #[cfg(feature = "std")]
    /// let outcome = fetch_api.with_exponential().call()?;
    /// # Ok::<(), chrono_machines::RetryError<std::io::Error>>(())
    /// ```
    fn with_exponential(self) -> DefaultRetryBuilder<Self, crate::backoff::ExponentialBackoff, T, E>
    where
        Self: Sized,
    {
        self.retry(crate::backoff::ExponentialBackoff::default())
    }

    /// Create a retry builder with constant backoff
    ///
    /// # Arguments
    ///
    /// * `delay_ms` - Fixed delay in milliseconds between retry attempts
    ///
    /// Default configuration (besides delay):
    /// - max_attempts: 3
    /// - jitter_factor: 0.0 (no jitter)
    ///
    /// # Returns
    ///
    /// A `RetryBuilder` configured with `ConstantBackoff` using the specified delay
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::RetryableExt;
    ///
    /// fn check_status() -> Result<bool, std::io::Error> {
    ///     Ok(true)
    /// }
    ///
    /// # #[cfg(feature = "std")]
    /// // Retry with fixed 500ms delay between attempts
    /// let outcome = check_status.with_constant(500).call()?;
    /// # Ok::<(), chrono_machines::RetryError<std::io::Error>>(())
    /// ```
    fn with_constant(self, delay_ms: u64) -> DefaultRetryBuilder<Self, crate::backoff::ConstantBackoff, T, E>
    where
        Self: Sized,
    {
        self.retry(crate::backoff::ConstantBackoff::new().delay_ms(delay_ms))
    }

    /// Create a retry builder with Fibonacci backoff using default configuration
    ///
    /// Default configuration:
    /// - max_attempts: 8
    /// - base_delay_ms: 100
    /// - max_delay_ms: 10_000
    /// - jitter_factor: 1.0 (full jitter)
    ///
    /// Delays follow the Fibonacci sequence: 100ms, 100ms, 200ms, 300ms, 500ms...
    ///
    /// # Returns
    ///
    /// A `RetryBuilder` configured with `FibonacciBackoff::default()`
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono_machines::RetryableExt;
    ///
    /// fn connect_database() -> Result<(), std::io::Error> {
    ///     Ok(())
    /// }
    ///
    /// # #[cfg(feature = "std")]
    /// let outcome = connect_database.with_fibonacci().call()?;
    /// # Ok::<(), chrono_machines::RetryError<std::io::Error>>(())
    /// ```
    fn with_fibonacci(self) -> DefaultRetryBuilder<Self, crate::backoff::FibonacciBackoff, T, E>
    where
        Self: Sized,
    {
        self.retry(crate::backoff::FibonacciBackoff::default())
    }
}

// Blanket implementation for all Retryable types
impl<F, T, E> RetryableExt<T, E> for F where F: Retryable<T, E> {}

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
    notify: Option<NotifyCallback<E>>,
    on_success: Option<NotifyCallback<E>>,
    on_failure: Option<FailureCallback<E>>,
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
    /// The callback receives a [`RetryContext`] with comprehensive retry state information.
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
    ///     .notify(|ctx| {
    ///         if let Some(err) = ctx.error {
    ///             println!(
    ///                 "Attempt {} failed, retrying after {}ms (cumulative: {}ms): {:?}",
    ///                 ctx.attempt,
    ///                 ctx.next_delay_ms.unwrap_or(0),
    ///                 ctx.cumulative_delay_ms,
    ///                 err
    ///             );
    ///         }
    ///     })
    ///     .call();
    /// ```
    pub fn notify<C>(mut self, callback: C) -> Self
    where
        C: FnMut(&RetryContext<E>) + 'static,
    {
        self.notify = Some(Box::new(callback));
        self
    }

    /// Execute a callback after a successful attempt.
    ///
    /// The callback receives a [`RetryContext`] with no error (error field is None).
    pub fn on_success<C>(mut self, callback: C) -> Self
    where
        C: FnMut(&RetryContext<E>) + 'static,
    {
        self.on_success = Some(Box::new(callback));
        self
    }

    /// Execute a callback when the retry process terminates with failure.
    ///
    /// The callback receives the rich [`RetryError`] describing the failure.
    pub fn on_failure<C>(mut self, callback: C) -> Self
    where
        C: FnMut(&RetryError<E>) + 'static,
    {
        self.on_failure = Some(Box::new(callback));
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

        loop {
            match (self.operation)() {
                Ok(_value) => {
                    // Invoke on_success callback with context
                    if let Some(ref mut callback) = self.on_success {
                        let ctx = RetryContext {
                            attempt,
                            next_delay_ms: None,
                            cumulative_delay_ms,
                            error: None,
                        };
                        callback(&ctx);
                    }
                    return Ok(RetryOutcome::new(_value, attempt, cumulative_delay_ms));
                }
                Err(error) => {
                    // Check if this error should be retried
                    if let Some(ref predicate) = self.when
                        && !predicate(&error) {
                            // Error doesn't match predicate, fail immediately
                            let retry_error = RetryError::new(
                                RetryErrorKind::PredicateRejected,
                                attempt,
                                max_attempts,
                                cumulative_delay_ms,
                                Some(error),
                            );
                            if let Some(ref mut callback) = self.on_failure {
                                callback(&retry_error);
                            }
                            return Err(retry_error);
                        }

                    // Check if we have retries remaining
                    if !self.backoff.should_retry(attempt) {
                        let retry_error = RetryError::new(
                            RetryErrorKind::Exhausted,
                            attempt,
                            max_attempts,
                            cumulative_delay_ms,
                            Some(error),
                        );
                        if let Some(ref mut callback) = self.on_failure {
                            callback(&retry_error);
                        }
                        return Err(retry_error);
                    }

                    // Calculate delay
                    match self.backoff.delay(attempt, &mut rng) {
                        Some(delay_ms) => {
                            // Notify if callback is set
                            if let Some(ref mut notify) = self.notify {
                                let ctx = RetryContext {
                                    attempt,
                                    next_delay_ms: Some(delay_ms),
                                    cumulative_delay_ms,
                                    error: Some(&error),
                                };
                                notify(&ctx);
                            }

                            // Sleep before retry
                            sleeper.sleep_ms(delay_ms);
                            cumulative_delay_ms = cumulative_delay_ms.saturating_add(delay_ms);
                            attempt = attempt.saturating_add(1);
                        }
                        None => {
                            // Backoff says no more retries
                            let retry_error = RetryError::new(
                                RetryErrorKind::Exhausted,
                                attempt,
                                max_attempts,
                                cumulative_delay_ms,
                                Some(error),
                            );
                            if let Some(ref mut callback) = self.on_failure {
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
        assert!(err.cumulative_delay_ms() > 0);
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
        use core::cell::{Cell, RefCell};
        #[cfg(feature = "std")]
        use std::rc::Rc;

        #[cfg(not(feature = "std"))]
        use alloc::rc::Rc;

        let attempts = Cell::new(0);
        let notify_calls = Rc::new(RefCell::new(Vec::new()));
        let notify_calls_clone = Rc::clone(&notify_calls);

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
            .notify(move |ctx| {
                // Track notify calls
                notify_calls_clone.borrow_mut().push((
                    ctx.attempt,
                    ctx.next_delay_ms,
                    ctx.cumulative_delay_ms,
                    ctx.error.is_some(),
                ));
                assert!(ctx.attempt >= 1);
                assert!(ctx.next_delay_ms.is_some());
                assert!(ctx.error.is_some());
            })
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 3);

        // Verify notify was called twice (for the two failures)
        let calls = notify_calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, 1); // First attempt
        assert_eq!(calls[1].0, 2); // Second attempt
    }

    #[test]
    fn test_on_success_callback_invoked() {
        use core::cell::Cell;
        use core::sync::atomic::{AtomicUsize, Ordering};

        static SUCCESS_ATTEMPT: AtomicUsize = AtomicUsize::new(0);
        static SUCCESS_CUMULATIVE_DELAY: AtomicUsize = AtomicUsize::new(0);

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
        SUCCESS_CUMULATIVE_DELAY.store(0, Ordering::SeqCst);

        let outcome = operation
            .retry(ExponentialBackoff::default().max_attempts(3))
            .on_success(|ctx| {
                SUCCESS_ATTEMPT.store(ctx.attempt as usize, Ordering::SeqCst);
                SUCCESS_CUMULATIVE_DELAY.store(ctx.cumulative_delay_ms as usize, Ordering::SeqCst);
                assert!(ctx.error.is_none());
                assert!(ctx.next_delay_ms.is_none());
            })
            .call_with_sleeper(FnSleeper(|_| {}))
            .expect("retry should succeed");

        assert_eq!(outcome.into_inner(), 7);
        assert_eq!(SUCCESS_ATTEMPT.load(Ordering::SeqCst), 2);
        // Should have some cumulative delay from the first retry
        assert!(SUCCESS_CUMULATIVE_DELAY.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_on_failure_callback_invoked() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static FAILURE_KIND: AtomicUsize = AtomicUsize::new(0);
        static FAILURE_CUMULATIVE_DELAY: AtomicUsize = AtomicUsize::new(0);

        fn always_fails() -> Result<(), TestError> {
            Err(TestError::Retryable)
        }

        FAILURE_KIND.store(0, Ordering::SeqCst);
        FAILURE_CUMULATIVE_DELAY.store(0, Ordering::SeqCst);

        let result = always_fails
            .retry(ExponentialBackoff::default().max_attempts(2))
            .on_failure(|err| {
                let marker = match err.kind() {
                    RetryErrorKind::Exhausted => 1,
                    RetryErrorKind::PredicateRejected => 2,
                };
                FAILURE_KIND.store(marker, Ordering::SeqCst);
                FAILURE_CUMULATIVE_DELAY.store(err.cumulative_delay_ms() as usize, Ordering::SeqCst);
            })
            .call_with_sleeper(FnSleeper(|_| {}));

        assert!(result.is_err());
        assert_eq!(FAILURE_KIND.load(Ordering::SeqCst), 1);
        // Should have cumulative delay from retry attempt
        assert!(FAILURE_CUMULATIVE_DELAY.load(Ordering::SeqCst) > 0);
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

    #[test]
    fn test_retry_context_comprehensive() {
        use core::cell::{Cell, RefCell};
        #[cfg(feature = "std")]
        use std::rc::Rc;

        #[cfg(not(feature = "std"))]
        use alloc::rc::Rc;

        let attempts = Cell::new(0);
        let notify_contexts = Rc::new(RefCell::new(Vec::new()));
        let notify_contexts_clone = Rc::clone(&notify_contexts);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 3 {
                Err(TestError::Retryable)
            } else {
                Ok(42)
            }
        };

        let result = operation
            .retry(ConstantBackoff::new().delay_ms(100).max_attempts(5).jitter_factor(0.0))
            .notify(move |ctx| {
                // Capture context for verification
                notify_contexts_clone.borrow_mut().push((
                    ctx.attempt,
                    ctx.next_delay_ms,
                    ctx.cumulative_delay_ms,
                ));

                // Verify error is present during notify
                assert!(ctx.error.is_some());
                if let Some(err) = ctx.error {
                    assert_eq!(err, &TestError::Retryable);
                }
            })
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 4);

        // Verify notify contexts
        let contexts = notify_contexts.borrow();
        assert_eq!(contexts.len(), 3); // Three failures before success

        // First failure
        assert_eq!(contexts[0].0, 1);
        assert_eq!(contexts[0].1, Some(100));
        assert_eq!(contexts[0].2, 0); // No cumulative delay yet

        // Second failure
        assert_eq!(contexts[1].0, 2);
        assert_eq!(contexts[1].1, Some(100));
        assert_eq!(contexts[1].2, 100); // One delay accumulated

        // Third failure
        assert_eq!(contexts[2].0, 3);
        assert_eq!(contexts[2].1, Some(100));
        assert_eq!(contexts[2].2, 200); // Two delays accumulated
    }

    #[test]
    fn test_retry_context_on_success() {
        use core::cell::{Cell, RefCell};
        #[cfg(feature = "std")]
        use std::rc::Rc;

        #[cfg(not(feature = "std"))]
        use alloc::rc::Rc;

        let attempts = Cell::new(0);
        let success_context = Rc::new(RefCell::new(None));
        let success_context_clone = Rc::clone(&success_context);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 2 {
                Err(TestError::Retryable)
            } else {
                Ok(100)
            }
        };

        let result = operation
            .retry(ConstantBackoff::new().delay_ms(50).max_attempts(5).jitter_factor(0.0))
            .on_success(move |ctx| {
                success_context_clone.borrow_mut().replace((
                    ctx.attempt,
                    ctx.next_delay_ms,
                    ctx.cumulative_delay_ms,
                    ctx.error.is_none(),
                ));
            })
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 3);
        assert_eq!(outcome.cumulative_delay_ms(), 100); // 2 delays of 50ms

        // Verify success context
        let ctx = success_context.borrow();
        assert!(ctx.is_some());
        let (attempt, next_delay, cumulative, no_error) = ctx.unwrap();
        assert_eq!(attempt, 3);
        assert_eq!(next_delay, None); // No next delay on success
        assert_eq!(cumulative, 100);
        assert!(no_error); // Error should be None
    }

    #[test]
    fn test_retry_context_cumulative_accuracy() {
        use core::cell::{Cell, RefCell};
        #[cfg(feature = "std")]
        use std::rc::Rc;

        #[cfg(not(feature = "std"))]
        use alloc::rc::Rc;

        let attempts = Cell::new(0);
        let cumulative_progression = Rc::new(RefCell::new(Vec::new()));
        let cumulative_progression_clone = Rc::clone(&cumulative_progression);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);
            Err::<(), TestError>(TestError::Retryable)
        };

        let _result = operation
            .retry(ConstantBackoff::new().delay_ms(25).max_attempts(4).jitter_factor(0.0))
            .notify(move |ctx| {
                cumulative_progression_clone.borrow_mut().push(ctx.cumulative_delay_ms);
            })
            .call_with_sleeper(FnSleeper(|_| {}));

        // Verify cumulative delay progression
        let progression = cumulative_progression.borrow();
        assert_eq!(progression.len(), 3); // 3 retries before exhaustion
        assert_eq!(progression[0], 0);    // Before first sleep
        assert_eq!(progression[1], 25);   // After first sleep
        assert_eq!(progression[2], 50);   // After second sleep
    }

    // ============================================================================
    // RetryableExt Tests
    // ============================================================================

    #[test]
    fn test_with_exponential_success() {
        use super::RetryableExt;

        fn always_succeeds() -> Result<i32, TestError> {
            Ok(42)
        }

        let result = always_succeeds
            .with_exponential()
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 1);
        assert_eq!(outcome.into_inner(), 42);
    }

    #[test]
    fn test_with_exponential_retry_behavior() {
        use super::RetryableExt;
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 2 {
                Err(TestError::Retryable)
            } else {
                Ok(100)
            }
        };

        let result = operation
            .with_exponential()
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 3);
        assert_eq!(outcome.into_inner(), 100);
        // Verify default max_attempts was used (3)
        assert_eq!(attempts.get(), 3);
    }

    #[test]
    fn test_with_exponential_exhausted() {
        use super::RetryableExt;

        fn always_fails() -> Result<i32, TestError> {
            Err(TestError::Retryable)
        }

        let result = always_fails
            .with_exponential()
            .call_with_sleeper(FnSleeper(|_| {}));

        let err = result.expect_err("retry should exhaust");
        assert_eq!(err.kind(), RetryErrorKind::Exhausted);
        assert_eq!(err.attempts(), 3); // Default max_attempts
        assert_eq!(err.max_attempts(), 3);
    }

    #[test]
    fn test_with_exponential_chaining() {
        use super::RetryableExt;
        use core::cell::Cell;

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

        // Test that .with_exponential() can be chained with other builder methods
        let result = operation
            .with_exponential()
            .when(|e| matches!(e, TestError::Retryable))
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 2);
        assert_eq!(outcome.into_inner(), 7);
    }

    #[test]
    fn test_with_constant_success() {
        use super::RetryableExt;

        fn always_succeeds() -> Result<i32, TestError> {
            Ok(99)
        }

        let result = always_succeeds
            .with_constant(250)
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 1);
        assert_eq!(outcome.into_inner(), 99);
    }

    #[test]
    fn test_with_constant_uses_correct_delay() {
        use super::RetryableExt;
        use core::cell::{Cell, RefCell};
        #[cfg(feature = "std")]
        use std::rc::Rc;

        #[cfg(not(feature = "std"))]
        use alloc::rc::Rc;

        let attempts = Cell::new(0);
        let delays = Rc::new(RefCell::new(Vec::new()));
        let delays_clone = Rc::clone(&delays);

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
            .with_constant(500)
            .notify(move |ctx| {
                if let Some(delay) = ctx.next_delay_ms {
                    delays_clone.borrow_mut().push(delay);
                }
            })
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 3);

        // Verify constant delay was used (ConstantBackoff default has no jitter)
        let recorded_delays = delays.borrow();
        assert_eq!(recorded_delays.len(), 2); // Two retries
        assert_eq!(recorded_delays[0], 500);
        assert_eq!(recorded_delays[1], 500);
    }

    #[test]
    fn test_with_constant_chaining() {
        use super::RetryableExt;
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 1 {
                Err(TestError::Fatal)
            } else {
                Ok(123)
            }
        };

        // Test that .with_constant() can be chained with predicate
        let result = operation
            .with_constant(100)
            .when(|e| matches!(e, TestError::Retryable))
            .call_with_sleeper(FnSleeper(|_| {}));

        // Should fail immediately due to predicate
        let err = result.expect_err("retry should fail due to predicate");
        assert_eq!(err.kind(), RetryErrorKind::PredicateRejected);
        assert_eq!(err.attempts(), 1);
    }

    #[test]
    fn test_with_fibonacci_success() {
        use super::RetryableExt;

        fn always_succeeds() -> Result<String, TestError> {
            Ok("success".to_string())
        }

        let result = always_succeeds
            .with_fibonacci()
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 1);
        assert_eq!(outcome.into_inner(), "success");
    }

    #[test]
    fn test_with_fibonacci_retry_behavior() {
        use super::RetryableExt;
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 3 {
                Err(TestError::Retryable)
            } else {
                Ok(777)
            }
        };

        let result = operation
            .with_fibonacci()
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 4);
        assert_eq!(outcome.into_inner(), 777);
    }

    #[test]
    fn test_with_fibonacci_exhausted() {
        use super::RetryableExt;

        fn always_fails() -> Result<i32, TestError> {
            Err(TestError::Retryable)
        }

        let result = always_fails
            .with_fibonacci()
            .call_with_sleeper(FnSleeper(|_| {}));

        let err = result.expect_err("retry should exhaust");
        assert_eq!(err.kind(), RetryErrorKind::Exhausted);
        assert_eq!(err.attempts(), 8); // Fibonacci default max_attempts
        assert_eq!(err.max_attempts(), 8);
    }

    #[test]
    fn test_with_fibonacci_chaining() {
        use super::RetryableExt;
        use core::cell::Cell;
        use core::sync::atomic::{AtomicUsize, Ordering};

        static SUCCESS_COUNT: AtomicUsize = AtomicUsize::new(0);
        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 2 {
                Err(TestError::Retryable)
            } else {
                Ok(555)
            }
        };

        SUCCESS_COUNT.store(0, Ordering::SeqCst);

        // Test that .with_fibonacci() can be chained with callbacks
        let result = operation
            .with_fibonacci()
            .on_success(|_ctx| {
                SUCCESS_COUNT.fetch_add(1, Ordering::SeqCst);
            })
            .call_with_sleeper(FnSleeper(|_| {}));

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 3);
        assert_eq!(outcome.into_inner(), 555);
        assert_eq!(SUCCESS_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_all_extension_methods_produce_working_retries() {
        use super::RetryableExt;
        use core::cell::Cell;

        let attempts_exp = Cell::new(0);
        let op_exp = || {
            let current = attempts_exp.get();
            attempts_exp.set(current + 1);
            if current < 1 {
                Err(TestError::Retryable)
            } else {
                Ok(1)
            }
        };

        let attempts_const = Cell::new(0);
        let op_const = || {
            let current = attempts_const.get();
            attempts_const.set(current + 1);
            if current < 1 {
                Err(TestError::Retryable)
            } else {
                Ok(2)
            }
        };

        let attempts_fib = Cell::new(0);
        let op_fib = || {
            let current = attempts_fib.get();
            attempts_fib.set(current + 1);
            if current < 1 {
                Err(TestError::Retryable)
            } else {
                Ok(3)
            }
        };

        // All extension methods should produce working retry builders
        let r1 = op_exp
            .with_exponential()
            .call_with_sleeper(FnSleeper(|_| {}))
            .expect("exponential retry works");

        let r2 = op_const
            .with_constant(100)
            .call_with_sleeper(FnSleeper(|_| {}))
            .expect("constant retry works");

        let r3 = op_fib
            .with_fibonacci()
            .call_with_sleeper(FnSleeper(|_| {}))
            .expect("fibonacci retry works");

        assert_eq!(r1.into_inner(), 1);
        assert_eq!(r2.into_inner(), 2);
        assert_eq!(r3.into_inner(), 3);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_with_exponential_std_sleeper() {
        use super::RetryableExt;
        use core::cell::Cell;

        let attempts = Cell::new(0);

        let operation = || {
            let current = attempts.get();
            attempts.set(current + 1);

            if current < 1 {
                Err(TestError::Retryable)
            } else {
                Ok(999)
            }
        };

        // Test with std sleeper
        let result = operation.with_exponential().call();

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.attempts(), 2);
        assert_eq!(outcome.into_inner(), 999);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_with_constant_std_sleeper() {
        use super::RetryableExt;

        fn always_succeeds() -> Result<i32, TestError> {
            Ok(888)
        }

        // Test with std sleeper
        let result = always_succeeds.with_constant(50).call();

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.into_inner(), 888);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_with_fibonacci_std_sleeper() {
        use super::RetryableExt;

        fn always_succeeds() -> Result<i32, TestError> {
            Ok(444)
        }

        // Test with std sleeper
        let result = always_succeeds.with_fibonacci().call();

        let outcome = result.expect("retry should succeed");
        assert_eq!(outcome.into_inner(), 444);
    }
}
