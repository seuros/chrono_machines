//! The async driver, exercised on a real tokio runtime.
//!
//! These are integration tests on purpose: they go through the public API, so
//! they also prove the `async` feature re-exports what callers need.

#![cfg(feature = "async")]

use chrono_machines::{AsyncRetryable, ConstantBackoff, ExponentialBackoff};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The sleeper every test uses: a plain closure, which is the whole point —
/// the crate never names tokio.
fn tokio_sleeper() -> impl Fn(u64) -> tokio::time::Sleep {
    |ms| tokio::time::sleep(Duration::from_millis(ms))
}

fn fixed(delay_ms: u64, max_attempts: u8) -> ConstantBackoff {
    ConstantBackoff::new()
        .delay_ms(delay_ms)
        .max_attempts(max_attempts)
}

#[tokio::test]
async fn retries_a_failing_future_until_it_succeeds() {
    let attempts = AtomicUsize::new(0);

    let outcome = (|| async {
        match attempts.fetch_add(1, Ordering::SeqCst) {
            0 | 1 => Err("not yet"),
            _ => Ok("landed"),
        }
    })
    .retry_async(fixed(10, 5))
    .call_async(tokio_sleeper())
    .await
    .expect("third attempt succeeds");

    assert_eq!(outcome.attempts(), 3);
    assert_eq!(*outcome.value(), "landed");
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn the_delay_actually_elapses_on_the_reactor() {
    let start = Instant::now();

    let result = (|| async { Err::<(), _>("always") })
        .retry_async(fixed(40, 3))
        .call_async(tokio_sleeper())
        .await;

    let err = result.expect_err("exhausts its attempts");
    // 3 attempts means 2 sleeps.
    assert_eq!(err.attempts(), 3);
    assert_eq!(err.cumulative_delay_ms(), 80);
    assert!(
        start.elapsed() >= Duration::from_millis(80),
        "wall clock {:?} is shorter than the delays it reported",
        start.elapsed(),
    );
}

/// The bug this guards against is the reason the async driver exists: a
/// blocking sleeper on a current-thread runtime stalls the executor, so
/// nothing else makes progress while a retry waits.
#[tokio::test(flavor = "current_thread")]
async fn a_waiting_retry_does_not_block_the_executor() {
    let ticks = Arc::new(AtomicUsize::new(0));

    let ticker = {
        let ticks = Arc::clone(&ticks);
        tokio::spawn(async move {
            for _ in 0..10 {
                tokio::time::sleep(Duration::from_millis(10)).await;
                ticks.fetch_add(1, Ordering::SeqCst);
            }
        })
    };

    let _ = (|| async { Err::<(), _>("always") })
        .retry_async(fixed(50, 3))
        .call_async(tokio_sleeper())
        .await;

    // 100ms of retry delay on a single thread: the other task must have run.
    assert!(
        ticks.load(Ordering::SeqCst) >= 5,
        "co-scheduled task only ticked {} times — the retry blocked the executor",
        ticks.load(Ordering::SeqCst),
    );
    ticker.abort();
}

/// Auto-trait leakage: nothing declares `Send`, but a retry future built from
/// `Send` parts is `Send` and can be spawned. If this stops compiling, the
/// crate has become unusable with `tokio::spawn`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_retry_future_is_send_and_spawnable() {
    fn assert_send<F: Send>(f: F) -> F {
        f
    }

    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&attempts);

    let future = assert_send(
        (move || {
            let counter = Arc::clone(&counter);
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err("cold start")
                } else {
                    Ok(7u32)
                }
            }
        })
        .retry_async(fixed(5, 3))
        .call_async(tokio_sleeper()),
    );

    let outcome = tokio::spawn(future)
        .await
        .expect("task not panicked")
        .expect("second attempt succeeds");

    assert_eq!(*outcome.value(), 7);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_rejected_error_fails_without_sleeping() {
    let start = Instant::now();
    let attempts = AtomicUsize::new(0);

    let err = (|| async {
        attempts.fetch_add(1, Ordering::SeqCst);
        Err::<(), _>("fatal")
    })
    .retry_async(fixed(500, 5))
    .when(|e: &&str| *e != "fatal")
    .call_async(tokio_sleeper())
    .await
    .expect_err("the predicate rejects this error");

    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(err.attempts(), 1);
    assert_eq!(err.cumulative_delay_ms(), 0);
    assert!(start.elapsed() < Duration::from_millis(500));
}

/// `when`, `notify`, `on_success` and `on_failure` are configured on the shared
/// builder, so they must behave identically for the async driver.
#[tokio::test]
async fn callbacks_fire_the_same_as_the_sync_driver() {
    let notified = Arc::new(AtomicUsize::new(0));
    let succeeded = Arc::new(AtomicUsize::new(0));
    let (n, s) = (Arc::clone(&notified), Arc::clone(&succeeded));

    let attempts = AtomicUsize::new(0);
    let outcome = (|| async {
        if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
            Err("retry me")
        } else {
            Ok(())
        }
    })
    .retry_async(fixed(5, 5))
    .notify(move |ctx| {
        assert!(ctx.error.is_some(), "notify only fires on a failed attempt");
        assert!(ctx.next_delay_ms.is_some());
        n.fetch_add(1, Ordering::SeqCst);
    })
    .on_success(move |ctx| {
        assert!(ctx.error.is_none());
        s.fetch_add(1, Ordering::SeqCst);
    })
    .call_async(tokio_sleeper())
    .await
    .expect("succeeds on the third attempt");

    assert_eq!(outcome.attempts(), 3);
    assert_eq!(notified.load(Ordering::SeqCst), 2);
    assert_eq!(succeeded.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn on_failure_reports_exhaustion() {
    let failures = Arc::new(AtomicUsize::new(0));
    let f = Arc::clone(&failures);

    let err = (|| async { Err::<(), _>("always") })
        .retry_async(ExponentialBackoff::default().max_attempts(3))
        .on_failure(move |e| {
            assert_eq!(e.attempts(), 3);
            assert_eq!(e.max_attempts(), 3);
            f.fetch_add(1, Ordering::SeqCst);
        })
        .call_async(tokio_sleeper())
        .await
        .expect_err("exhausts its attempts");

    assert_eq!(err.cause(), Some(&"always"));
    assert_eq!(failures.load(Ordering::SeqCst), 1);
}

/// Dropping the future mid-sleep must abandon the retry, not detach it.
#[tokio::test]
async fn dropping_the_future_cancels_the_retry() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&attempts);

    let retry = (move || {
        let counter = Arc::clone(&counter);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>("always")
        }
    })
    .retry_async(fixed(200, 10))
    .call_async(tokio_sleeper());

    // Times out during the first sleep, dropping the retry future.
    assert!(
        tokio::time::timeout(Duration::from_millis(50), retry)
            .await
            .is_err()
    );

    let seen = attempts.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        seen,
        "a cancelled retry kept attempting in the background",
    );
}
