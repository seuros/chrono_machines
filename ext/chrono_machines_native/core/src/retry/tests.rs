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
fn test_delay_from_hint_used_verbatim() {
    use core::cell::Cell;
    use std::sync::{Arc, Mutex};

    let attempts = Cell::new(0);
    let delays = Arc::new(Mutex::new(Vec::new()));
    let delays_clone = Arc::clone(&delays);

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
        .retry(ExponentialBackoff::default().max_attempts(5))
        .delay_from(|_e: &TestError, attempt| DelayHint::Ms(1000 * attempt as u64))
        .notify(move |ctx| {
            delays_clone.lock().unwrap().push(ctx.next_delay_ms);
        })
        .call_with_sleeper(FnSleeper(|_| {}));

    let outcome = result.expect("retry should succeed");
    assert_eq!(outcome.attempts(), 3);
    // Hints pass through untouched - no jitter.
    assert_eq!(*delays.lock().unwrap(), vec![Some(1000), Some(2000)]);
    assert_eq!(outcome.cumulative_delay_ms(), 3000);
}

#[test]
fn test_delay_from_backoff_falls_through_to_strategy() {
    use core::cell::Cell;
    use std::sync::{Arc, Mutex};

    let attempts = Cell::new(0);
    let delays = Arc::new(Mutex::new(Vec::new()));
    let delays_clone = Arc::clone(&delays);

    let operation = || {
        let current = attempts.get();
        attempts.set(current + 1);
        if current < 1 {
            Err(TestError::Retryable)
        } else {
            Ok(7)
        }
    };

    let result = operation
        .retry(ConstantBackoff::new().delay_ms(250).max_attempts(3))
        .delay_from(|_e: &TestError, _attempt| DelayHint::Backoff)
        .notify(move |ctx| {
            delays_clone.lock().unwrap().push(ctx.next_delay_ms);
        })
        .call_with_sleeper(FnSleeper(|_| {}));

    let outcome = result.expect("retry should succeed");
    assert_eq!(outcome.attempts(), 2);
    assert_eq!(*delays.lock().unwrap(), vec![Some(250)]);
}

#[test]
fn test_delay_from_halt_stops_retrying() {
    use core::cell::Cell;

    let attempts = Cell::new(0);

    let operation = || {
        attempts.set(attempts.get() + 1);
        Err::<i32, TestError>(TestError::Retryable)
    };

    let result = operation
        .retry(ExponentialBackoff::default().max_attempts(5))
        .delay_from(|_e: &TestError, _attempt| DelayHint::Halt)
        .call_with_sleeper(FnSleeper(|_| {}));

    let err = result.expect_err("retry should halt");
    assert_eq!(err.kind(), RetryErrorKind::HintHalted);
    assert_eq!(err.attempts(), 1);
    assert_eq!(err.cause(), Some(&TestError::Retryable));
    assert_eq!(attempts.get(), 1);
}

#[test]
fn test_delay_from_hint_beyond_max_delay_halts() {
    let operation = || Err::<i32, TestError>(TestError::Retryable);

    let result = operation
        .retry(
            ExponentialBackoff::default()
                .max_delay_ms(10_000)
                .max_attempts(5),
        )
        .delay_from(|_e: &TestError, _attempt| DelayHint::Ms(60_000))
        .call_with_sleeper(FnSleeper(|_| {}));

    let err = result.expect_err("oversized hint should halt");
    assert_eq!(err.kind(), RetryErrorKind::HintHalted);
    assert_eq!(err.attempts(), 1);
    assert_eq!(err.cause(), Some(&TestError::Retryable));
}

#[test]
fn test_delay_from_exhaustion_wins_over_hint() {
    use core::cell::Cell;

    let attempts = Cell::new(0);

    let operation = || {
        attempts.set(attempts.get() + 1);
        Err::<i32, TestError>(TestError::Retryable)
    };

    let result = operation
        .retry(ExponentialBackoff::default().max_attempts(2))
        .delay_from(|_e: &TestError, _attempt| DelayHint::Ms(1))
        .call_with_sleeper(FnSleeper(|_| {}));

    let err = result.expect_err("retry should exhaust");
    assert_eq!(err.kind(), RetryErrorKind::Exhausted);
    assert_eq!(err.attempts(), 2);
    assert_eq!(attempts.get(), 2);
}

#[test]
fn test_retry_notify_callback() {
    use core::cell::Cell;
    #[cfg(feature = "std")]
    use std::sync::{Arc, Mutex};

    #[cfg(not(feature = "std"))]
    use alloc::rc::Rc;

    let attempts = Cell::new(0);
    let notify_calls = Arc::new(Mutex::new(Vec::new()));
    let notify_calls_clone = Arc::clone(&notify_calls);

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
            notify_calls_clone.lock().unwrap().push((
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
    let calls = notify_calls.lock().unwrap();
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
        // Jitter off: see `test_on_failure_callback_invoked`.
        .retry(
            ExponentialBackoff::default()
                .max_attempts(3)
                .jitter_factor(0.0),
        )
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
        // Jitter off: the assertion below is that a delay was *accumulated*,
        // and full jitter can legitimately draw 100ms * 0.004 -> 0.
        .retry(
            ExponentialBackoff::default()
                .max_attempts(2)
                .jitter_factor(0.0),
        )
        .on_failure(|err| {
            let marker = match err.kind() {
                RetryErrorKind::Exhausted => 1,
                RetryErrorKind::PredicateRejected => 2,
                RetryErrorKind::HintHalted => 3,
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
    use core::cell::Cell;
    #[cfg(feature = "std")]
    use std::sync::{Arc, Mutex};

    #[cfg(not(feature = "std"))]
    use alloc::rc::Rc;

    let attempts = Cell::new(0);
    let notify_contexts = Arc::new(Mutex::new(Vec::new()));
    let notify_contexts_clone = Arc::clone(&notify_contexts);

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
        .retry(
            ConstantBackoff::new()
                .delay_ms(100)
                .max_attempts(5)
                .jitter_factor(0.0),
        )
        .notify(move |ctx| {
            // Capture context for verification
            notify_contexts_clone.lock().unwrap().push((
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
    let contexts = notify_contexts.lock().unwrap();
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
    use core::cell::Cell;
    #[cfg(feature = "std")]
    use std::sync::{Arc, Mutex};

    #[cfg(not(feature = "std"))]
    use alloc::rc::Rc;

    let attempts = Cell::new(0);
    let success_context = Arc::new(Mutex::new(None));
    let success_context_clone = Arc::clone(&success_context);

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
        .retry(
            ConstantBackoff::new()
                .delay_ms(50)
                .max_attempts(5)
                .jitter_factor(0.0),
        )
        .on_success(move |ctx| {
            success_context_clone.lock().unwrap().replace((
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
    let ctx = success_context.lock().unwrap();
    assert!(ctx.is_some());
    let (attempt, next_delay, cumulative, no_error) = ctx.unwrap();
    assert_eq!(attempt, 3);
    assert_eq!(next_delay, None); // No next delay on success
    assert_eq!(cumulative, 100);
    assert!(no_error); // Error should be None
}

#[test]
fn test_retry_context_cumulative_accuracy() {
    use core::cell::Cell;
    #[cfg(feature = "std")]
    use std::sync::{Arc, Mutex};

    #[cfg(not(feature = "std"))]
    use alloc::rc::Rc;

    let attempts = Cell::new(0);
    let cumulative_progression = Arc::new(Mutex::new(Vec::new()));
    let cumulative_progression_clone = Arc::clone(&cumulative_progression);

    let operation = || {
        let current = attempts.get();
        attempts.set(current + 1);
        Err::<(), TestError>(TestError::Retryable)
    };

    let _result = operation
        .retry(
            ConstantBackoff::new()
                .delay_ms(25)
                .max_attempts(4)
                .jitter_factor(0.0),
        )
        .notify(move |ctx| {
            cumulative_progression_clone
                .lock()
                .unwrap()
                .push(ctx.cumulative_delay_ms);
        })
        .call_with_sleeper(FnSleeper(|_| {}));

    // Verify cumulative delay progression
    let progression = cumulative_progression.lock().unwrap();
    assert_eq!(progression.len(), 3); // 3 retries before exhaustion
    assert_eq!(progression[0], 0); // Before first sleep
    assert_eq!(progression[1], 25); // After first sleep
    assert_eq!(progression[2], 50); // After second sleep
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
    use core::cell::Cell;
    #[cfg(feature = "std")]
    use std::sync::{Arc, Mutex};

    #[cfg(not(feature = "std"))]
    use alloc::rc::Rc;

    let attempts = Cell::new(0);
    let delays = Arc::new(Mutex::new(Vec::new()));
    let delays_clone = Arc::clone(&delays);

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
                delays_clone.lock().unwrap().push(delay);
            }
        })
        .call_with_sleeper(FnSleeper(|_| {}));

    let outcome = result.expect("retry should succeed");
    assert_eq!(outcome.attempts(), 3);

    // Verify constant delay was used (ConstantBackoff default has no jitter)
    let recorded_delays = delays.lock().unwrap();
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
