//! Retry mechanism with fluent builder API
//!
//! This module provides a fluent retry API for wrapping fallible operations
//! with automatic retries and configurable backoff strategies.

use crate::backoff::BackoffStrategy;
use crate::sleep::Sleeper;
use core::fmt;
#[cfg(feature = "std")]
use rand::rngs::StdRng;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;

/// Type alias for retry builder with default predicate
type DefaultRetryBuilder<F, B, T, E> = RetryBuilder<F, B, T, E, fn(&E) -> bool>;

/// Type alias for boxed notify callback
///
/// `Send` so that a configured builder — and therefore the future returned by
/// [`RetryBuilder::call_async`] — can cross threads. Without it `tokio::spawn`
/// rejects every retry that has a callback attached.
type NotifyCallback<E> = Box<dyn FnMut(&RetryContext<E>) + Send>;

/// Type alias for boxed failure callback
type FailureCallback<E> = Box<dyn FnMut(&RetryError<E>) + Send>;

/// Type alias for boxed delay_from hook
type DelayFromHook<E> = Box<dyn FnMut(&E, u8) -> DelayHint + Send>;

/// What a [`delay_from`](RetryBuilder::delay_from) hook wants the loop to do
/// next. The Rust counterpart of the Ruby executor's nil / Numeric / :halt
/// return values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayHint {
    /// No opinion - use the backoff strategy.
    Backoff,
    /// Sleep exactly this long, no jitter. Beyond the strategy's max delay
    /// the retry halts with [`RetryErrorKind::HintHalted`].
    Ms(u64),
    /// Stop retrying immediately, failing with the original error.
    Halt,
}

/// Build a terminal [`RetryError`], firing the `on_failure` callback if present.
fn finalize_failure<E>(
    on_failure: Option<&mut FailureCallback<E>>,
    kind: RetryErrorKind,
    attempt: u8,
    max_attempts: u8,
    cumulative_delay_ms: u64,
    error: E,
) -> RetryError<E> {
    let retry_error = RetryError::new(
        kind,
        attempt,
        max_attempts,
        cumulative_delay_ms,
        Some(error),
    );
    if let Some(callback) = on_failure {
        callback(&retry_error);
    }
    retry_error
}

/// Reason why a retry operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryErrorKind {
    /// The operation exhausted all retry attempts.
    Exhausted,
    /// The error was rejected by the `when` predicate.
    PredicateRejected,
    /// A `delay_from` hint halted the retry, either explicitly or by asking
    /// for a delay beyond the strategy's maximum.
    HintHalted,
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
            RetryErrorKind::HintHalted => {
                write!(
                    f,
                    "retry halted by delay_from hint on attempt {}",
                    self.attempts
                )?;
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
            delay_from: None,
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
    fn with_constant(
        self,
        delay_ms: u64,
    ) -> DefaultRetryBuilder<Self, crate::backoff::ConstantBackoff, T, E>
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
    delay_from: Option<DelayFromHook<E>>,
    notify: Option<NotifyCallback<E>>,
    on_success: Option<NotifyCallback<E>>,
    on_failure: Option<FailureCallback<E>>,
    _phantom_t: core::marker::PhantomData<T>,
    _phantom_e: core::marker::PhantomData<E>,
}

/// What the retry loop should do after one attempt.
///
/// Produced by [`RetryBuilder::step`], which holds every retry decision —
/// predicate, exhaustion, delay, callbacks — so the sync and async drivers
/// share one implementation and can never drift apart. The drivers differ
/// only in how they await the operation and the sleep.
enum Step<T, E> {
    /// The attempt succeeded; return this outcome.
    Done(RetryOutcome<T>),
    /// Sleep this many milliseconds, then attempt again.
    Sleep(u64),
    /// Give up and return this error.
    Fail(RetryError<E>),
}

// Configuration methods, plus the shared decision logic. Deliberately not bound
// on `F`: the sync driver wants `FnMut() -> Result<T, E>` and the async one
// wants `FnMut() -> impl Future`, and neither bound belongs on `.when()`.
impl<F, B, T, E, W> RetryBuilder<F, B, T, E, W>
where
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
            delay_from: self.delay_from,
            notify: self.notify,
            on_success: self.on_success,
            on_failure: self.on_failure,
            _phantom_t: core::marker::PhantomData,
            _phantom_e: core::marker::PhantomData,
        }
    }

    /// Let the error dictate the next delay - e.g. HTTP `Retry-After` on 429s.
    ///
    /// Called with `(&error, attempt)` before each retry; the returned
    /// [`DelayHint`] overrides the backoff strategy. A [`DelayHint::Ms`]
    /// beyond the strategy's max delay halts with
    /// [`RetryErrorKind::HintHalted`] - the server asked for more patience
    /// than this policy allows.
    ///
    /// ```rust,ignore
    /// let outcome = fetch
    ///     .retry(ExponentialBackoff::default())
    ///     .delay_from(|e: &ApiError, _attempt| match e.retry_after_ms() {
    ///         Some(ms) => DelayHint::Ms(ms),
    ///         None => DelayHint::Backoff,
    ///     })
    ///     .call()?;
    /// ```
    pub fn delay_from<C>(mut self, hook: C) -> Self
    where
        C: FnMut(&E, u8) -> DelayHint + Send + 'static,
    {
        self.delay_from = Some(Box::new(hook));
        self
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
        C: FnMut(&RetryContext<E>) + Send + 'static,
    {
        self.notify = Some(Box::new(callback));
        self
    }

    /// Execute a callback after a successful attempt.
    ///
    /// The callback receives a [`RetryContext`] with no error (error field is None).
    pub fn on_success<C>(mut self, callback: C) -> Self
    where
        C: FnMut(&RetryContext<E>) + Send + 'static,
    {
        self.on_success = Some(Box::new(callback));
        self
    }

    /// Execute a callback when the retry process terminates with failure.
    ///
    /// The callback receives the rich [`RetryError`] describing the failure.
    pub fn on_failure<C>(mut self, callback: C) -> Self
    where
        C: FnMut(&RetryError<E>) + Send + 'static,
    {
        self.on_failure = Some(Box::new(callback));
        self
    }

    /// Decide what happens after one attempt, firing any callbacks it implies.
    ///
    /// This is the whole retry policy. A driver's only jobs are producing the
    /// `Result` and honouring [`Step::Sleep`].
    fn step<R: rand::Rng>(
        &mut self,
        result: Result<T, E>,
        attempt: u8,
        cumulative_delay_ms: u64,
        rng: &mut R,
    ) -> Step<T, E> {
        let max_attempts = self.backoff.max_attempts();

        let error = match result {
            Ok(value) => {
                if let Some(ref mut callback) = self.on_success {
                    let ctx = RetryContext {
                        attempt,
                        next_delay_ms: None,
                        cumulative_delay_ms,
                        error: None,
                    };
                    callback(&ctx);
                }
                return Step::Done(RetryOutcome::new(value, attempt, cumulative_delay_ms));
            }
            Err(error) => error,
        };

        // An error the caller told us not to retry fails immediately, and is
        // reported as rejected rather than exhausted.
        if let Some(ref predicate) = self.when
            && !predicate(&error)
        {
            return Step::Fail(finalize_failure(
                self.on_failure.as_mut(),
                RetryErrorKind::PredicateRejected,
                attempt,
                max_attempts,
                cumulative_delay_ms,
                error,
            ));
        }

        // Out of attempts? Exhaustion wins over any delay hint.
        if !self.backoff.should_retry(attempt) {
            return Step::Fail(finalize_failure(
                self.on_failure.as_mut(),
                RetryErrorKind::Exhausted,
                attempt,
                max_attempts,
                cumulative_delay_ms,
                error,
            ));
        }

        let hint = match self.delay_from {
            Some(ref mut hook) => hook(&error, attempt),
            None => DelayHint::Backoff,
        };

        let delay_ms = match hint {
            DelayHint::Backoff => {
                // The strategy may still decline to produce a delay.
                let Some(delay_ms) = self.backoff.delay(attempt, rng) else {
                    return Step::Fail(finalize_failure(
                        self.on_failure.as_mut(),
                        RetryErrorKind::Exhausted,
                        attempt,
                        max_attempts,
                        cumulative_delay_ms,
                        error,
                    ));
                };
                delay_ms
            }
            DelayHint::Ms(delay_ms) => {
                if self
                    .backoff
                    .max_delay_ms()
                    .is_some_and(|cap| delay_ms > cap)
                {
                    return Step::Fail(finalize_failure(
                        self.on_failure.as_mut(),
                        RetryErrorKind::HintHalted,
                        attempt,
                        max_attempts,
                        cumulative_delay_ms,
                        error,
                    ));
                }
                delay_ms
            }
            DelayHint::Halt => {
                return Step::Fail(finalize_failure(
                    self.on_failure.as_mut(),
                    RetryErrorKind::HintHalted,
                    attempt,
                    max_attempts,
                    cumulative_delay_ms,
                    error,
                ));
            }
        };

        if let Some(ref mut notify) = self.notify {
            let ctx = RetryContext {
                attempt,
                next_delay_ms: Some(delay_ms),
                cumulative_delay_ms,
                error: Some(&error),
            };
            notify(&ctx);
        }

        Step::Sleep(delay_ms)
    }
}

impl<F, B, T, E, W> RetryBuilder<F, B, T, E, W>
where
    F: FnMut() -> Result<T, E>,
    B: BackoffStrategy,
    W: Fn(&E) -> bool,
{
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
    /// enabling support for embedded systems or testing. The sleeper blocks,
    /// so async runtimes should drive their own loop off
    /// [`Policy::calculate_delay`](crate::Policy::calculate_delay) instead.
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
    #[cfg(feature = "std")]
    pub fn call_with_sleeper<S: Sleeper>(
        self,
        sleeper: S,
    ) -> Result<RetryOutcome<T>, RetryError<E>> {
        let rng: StdRng = rand::make_rng();
        self.call_with_sleeper_and_rng(sleeper, rng)
    }

    /// Execute the retry operation with a custom sleeper and a caller-supplied RNG.
    ///
    /// The `no_std` entry point: callers provide their own [`rand::Rng`] instead
    /// of relying on the `std`-only `make_rng`.
    ///
    /// # Arguments
    ///
    /// * `sleeper` - Implementation of the [`Sleeper`] trait
    /// * `rng` - Random number generator used for jitter
    pub fn call_with_sleeper_and_rng<S: Sleeper, R: rand::Rng>(
        mut self,
        sleeper: S,
        mut rng: R,
    ) -> Result<RetryOutcome<T>, RetryError<E>> {
        let mut attempt = 1u8;
        let mut cumulative_delay_ms: u64 = 0;

        loop {
            let result = (self.operation)();
            match self.step(result, attempt, cumulative_delay_ms, &mut rng) {
                Step::Done(outcome) => return Ok(outcome),
                Step::Fail(error) => return Err(error),
                Step::Sleep(delay_ms) => {
                    sleeper.sleep_ms(delay_ms);
                    cumulative_delay_ms = cumulative_delay_ms.saturating_add(delay_ms);
                    attempt = attempt.saturating_add(1);
                }
            }
        }
    }
}

/// Begin a retry for an operation that returns a future.
///
/// The async counterpart to [`Retryable`], and the entry point for
/// [`RetryBuilder::call_async`]. It is a separate trait because the operation's
/// bound differs (`FnMut() -> impl Future` rather than `FnMut() -> Result`);
/// everything after construction — `when`, `notify`, `on_success`,
/// `on_failure` — is the same builder.
///
/// The closure is called once per attempt and must return a *fresh* future each
/// time, which rules out passing a single future: futures are not restartable.
///
/// ```rust,ignore
/// let outcome = (|| async { fetch().await })
///     .retry_async(ExponentialBackoff::default())
///     .when(|e| e.is_transient())
///     .call_async(|ms| tokio::time::sleep(Duration::from_millis(ms)))
///     .await?;
/// ```
#[cfg(feature = "async")]
pub trait AsyncRetryable<T, E> {
    /// Begin building a retry operation with the given backoff strategy
    fn retry_async<B: BackoffStrategy>(self, backoff: B) -> DefaultRetryBuilder<Self, B, T, E>
    where
        Self: Sized;
}

#[cfg(feature = "async")]
impl<F, Fut, T, E> AsyncRetryable<T, E> for F
where
    F: FnMut() -> Fut,
    Fut: core::future::Future<Output = Result<T, E>>,
{
    fn retry_async<B: BackoffStrategy>(
        self,
        backoff: B,
    ) -> RetryBuilder<Self, B, T, E, fn(&E) -> bool> {
        RetryBuilder {
            operation: self,
            backoff,
            when: None,
            delay_from: None,
            notify: None,
            on_success: None,
            on_failure: None,
            _phantom_t: core::marker::PhantomData,
            _phantom_e: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "async")]
impl<F, Fut, B, T, E, W> RetryBuilder<F, B, T, E, W>
where
    F: FnMut() -> Fut,
    Fut: core::future::Future<Output = Result<T, E>>,
    B: BackoffStrategy,
    W: Fn(&E) -> bool,
{
    /// Execute the retry operation on an async runtime (requires `std`).
    ///
    /// Identical policy to [`call`](Self::call) — same predicate, callbacks and
    /// backoff — but the operation is awaited and the delay yields to the
    /// executor instead of blocking the thread.
    ///
    /// The `sleeper` supplies the runtime's timer. Any
    /// `Fn(u64) -> Future<Output = ()>` works, so this crate stays free of a
    /// runtime dependency:
    ///
    /// ```rust,ignore
    /// use chrono_machines::{AsyncRetryable, ExponentialBackoff};
    /// use std::time::Duration;
    ///
    /// let outcome = (|| async { reqwest::get(url).await })
    ///     .retry_async(ExponentialBackoff::default())
    ///     .call_async(|ms| tokio::time::sleep(Duration::from_millis(ms)))
    ///     .await?;
    /// ```
    ///
    /// # Cancellation
    ///
    /// Dropping the returned future cancels the retry wherever it stands, in
    /// the operation or mid-sleep. The in-flight attempt is dropped with it, so
    /// the operation must be cancel-safe if that matters to the caller;
    /// `on_failure` does not fire, because nothing failed.
    #[cfg(feature = "std")]
    pub async fn call_async<S: crate::sleep::AsyncSleeper>(
        self,
        sleeper: S,
    ) -> Result<RetryOutcome<T>, RetryError<E>> {
        let rng: StdRng = rand::make_rng();
        self.call_async_with_rng(sleeper, rng).await
    }

    /// Execute the retry operation on an async runtime with a caller-supplied RNG.
    ///
    /// The `no_std` entry point, mirroring
    /// [`call_with_sleeper_and_rng`](Self::call_with_sleeper_and_rng): callers
    /// provide their own [`rand::Rng`] for jitter instead of relying on the
    /// `std`-only `make_rng`.
    pub async fn call_async_with_rng<S: crate::sleep::AsyncSleeper, R: rand::Rng>(
        mut self,
        sleeper: S,
        mut rng: R,
    ) -> Result<RetryOutcome<T>, RetryError<E>> {
        let mut attempt = 1u8;
        let mut cumulative_delay_ms: u64 = 0;

        loop {
            let result = (self.operation)().await;
            match self.step(result, attempt, cumulative_delay_ms, &mut rng) {
                Step::Done(outcome) => return Ok(outcome),
                Step::Fail(error) => return Err(error),
                Step::Sleep(delay_ms) => {
                    sleeper.sleep_ms(delay_ms).await;
                    cumulative_delay_ms = cumulative_delay_ms.saturating_add(delay_ms);
                    attempt = attempt.saturating_add(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
