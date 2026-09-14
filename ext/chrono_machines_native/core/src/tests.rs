use super::*;
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
    assert!(
        delay >= 900 && delay <= 1000,
        "delay {} not in range 900-1000",
        delay
    );

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
        "jitter_factor > 1.0 should clamp to 1.0, got delay {}",
        delay
    );

    // Extreme values should still be clamped
    let delay = policy.calculate_delay_with_rng(1, 999.0, &mut rng);
    assert!(delay <= 1000, "extreme jitter_factor should be clamped");

    let delay = policy.calculate_delay_with_rng(1, -999.0, &mut rng);
    assert_eq!(delay, 1000, "extreme negative should clamp to 0.0");
}
