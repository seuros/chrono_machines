use super::*;
use core::assert_matches;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn test_policy_default() {
    let policy = Policy::default();
    assert_eq!(policy.max_attempts, 3);
    assert_eq!(policy.base_delay_ms, 100);
    assert_eq!(policy.multiplier, 2.0);
    assert_eq!(policy.max_delay_ms, 10_000);
}

#[test]
fn test_calculate_delay_bounds() {
    let policy = Policy {
        max_attempts: 5,
        base_delay_ms: 100,
        multiplier: 2.0,
        max_delay_ms: 1000,
    };

    let mut rng = StdRng::seed_from_u64(42);

    // First attempt with full jitter: delay should be between 0 and 100ms
    let delay1 = policy.calculate_delay_with_rng(1, 1.0, &mut rng);
    assert!(delay1 <= 100);

    // Second attempt with full jitter: delay should be between 0 and 200ms
    let delay2 = policy.calculate_delay_with_rng(2, 1.0, &mut rng);
    assert!(delay2 <= 200);

    // Fifth attempt with full jitter: delay should be capped at max_delay_ms (1000ms)
    let delay5 = policy.calculate_delay_with_rng(5, 1.0, &mut rng);
    assert!(delay5 <= 1000);
}

#[test]
fn test_should_retry() {
    let policy = Policy {
        max_attempts: 3,
        ..Policy::default()
    };

    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));
    assert!(!policy.should_retry(3));
    assert!(!policy.should_retry(4));
}

#[test]
fn test_max_delay_cap() {
    let policy = Policy {
        max_attempts: 10,
        base_delay_ms: 100,
        multiplier: 2.0,
        max_delay_ms: 500,
    };

    let mut rng = StdRng::seed_from_u64(42);

    // High attempt number should still be capped
    let delay = policy.calculate_delay_with_rng(10, 1.0, &mut rng);
    assert!(delay <= 500);
}

#[test]
fn test_zero_multiplier() {
    let policy = Policy {
        max_attempts: 5,
        base_delay_ms: 100,
        multiplier: 1.0, // No exponential growth
        max_delay_ms: 10_000,
    };

    let mut rng = StdRng::seed_from_u64(42);

    // All delays with full jitter should be between 0 and base_delay_ms
    for attempt in 1..=5 {
        let delay = policy.calculate_delay_with_rng(attempt, 1.0, &mut rng);
        assert!(delay <= 100);
    }
}

#[test]
fn test_jitter_factor() {
    let policy = Policy {
        max_attempts: 5,
        base_delay_ms: 1000,
        multiplier: 1.0,
        max_delay_ms: 10_000,
    };

    let mut rng = StdRng::seed_from_u64(42);

    // 10% jitter: delay should be between 900ms (90%) and 1000ms (100%)
    let delay = policy.calculate_delay_with_rng(1, 0.1, &mut rng);
    assert_matches!(delay, 900..=1000);

    // No jitter: delay should be exactly base_delay_ms
    let delay = policy.calculate_delay_with_rng(1, 0.0, &mut rng);
    assert_eq!(delay, 1000);

    // Full jitter: delay should be between 0 and 1000ms
    let delay = policy.calculate_delay_with_rng(1, 1.0, &mut rng);
    assert!(delay <= 1000);
}

#[test]
fn test_jitter_factor_clamping() {
    let policy = Policy {
        max_attempts: 5,
        base_delay_ms: 1000,
        multiplier: 1.0,
        max_delay_ms: 10_000,
    };

    let mut rng = StdRng::seed_from_u64(42);

    // Negative jitter_factor should be clamped to 0.0
    let delay = policy.calculate_delay_with_rng(1, -0.5, &mut rng);
    assert_eq!(delay, 1000, "negative jitter_factor should clamp to 0.0");

    // jitter_factor > 1.0 should be clamped to 1.0
    let delay = policy.calculate_delay_with_rng(1, 2.0, &mut rng);
    assert!(
        delay <= 1000,
        "jitter_factor > 1.0 should clamp to 1.0, got delay {delay}"
    );

    // Extreme values should still be clamped
    let delay = policy.calculate_delay_with_rng(1, 999.0, &mut rng);
    assert!(delay <= 1000, "extreme jitter_factor should be clamped");

    let delay = policy.calculate_delay_with_rng(1, -999.0, &mut rng);
    assert_eq!(delay, 1000, "extreme negative should clamp to 0.0");
}

/// `Policy` computes its delay through the same code as `ExponentialBackoff`,
/// so one seed must give one delay. NaN is the exception by design: `Policy`
/// reads it as full jitter.
#[test]
fn test_policy_matches_exponential_backoff() {
    const JITTER: &[(f64, f64)] = &[
        (0.0, 0.0),
        (0.1, 0.1),
        (1.0, 1.0),
        (-2.0, 0.0),
        (3.0, 1.0),
        (f64::NAN, 1.0),
    ];

    let policy = Policy {
        max_attempts: 10,
        base_delay_ms: 150,
        multiplier: 1.7,
        max_delay_ms: 4_000,
    };

    for &(policy_jitter, backoff_jitter) in JITTER {
        let backoff = ExponentialBackoff::new()
            .base_delay_ms(150)
            .multiplier(1.7)
            .max_delay_ms(4_000)
            .max_attempts(10)
            .jitter_factor(backoff_jitter);
        let mut policy_rng = StdRng::seed_from_u64(7);
        let mut backoff_rng = StdRng::seed_from_u64(7);

        for attempt in 1..10 {
            assert_eq!(
                Some(policy.calculate_delay_with_rng(attempt, policy_jitter, &mut policy_rng)),
                backoff.delay(attempt, &mut backoff_rng),
                "attempt {attempt}, jitter {policy_jitter}"
            );
        }
    }
}

/// The `no_std` retry driver: a caller-supplied sleeper and RNG, no `std`.
#[cfg(feature = "alloc")]
#[test]
fn test_retry_with_caller_sleeper_and_rng() {
    use crate::retry::Retryable;
    use crate::sleep::FnSleeper;

    let mut attempts = 0_u8;
    let outcome = (|| {
        attempts += 1;
        if attempts < 3 {
            Err("transient")
        } else {
            Ok(attempts)
        }
    })
    .retry(ExponentialBackoff::new().base_delay_ms(1).max_attempts(5))
    .call_with_sleeper_and_rng(FnSleeper(|_ms| {}), StdRng::seed_from_u64(7))
    .expect("succeeds on the third attempt");

    assert_eq!(outcome.attempts(), 3);
    assert_eq!(outcome.into_inner(), 3);
}
