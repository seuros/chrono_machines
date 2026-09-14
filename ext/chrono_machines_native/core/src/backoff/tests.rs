use super::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn test_exponential_backoff_builder() {
    let backoff = ExponentialBackoff::new()
        .base_delay_ms(200)
        .multiplier(3.0)
        .max_delay_ms(5000)
        .max_attempts(5)
        .jitter_factor(0.5);

    assert_eq!(backoff.base_delay_ms, 200);
    assert_eq!(backoff.multiplier, 3.0);
    assert_eq!(backoff.max_delay_ms, 5000);
    assert_eq!(backoff.max_attempts, 5);
    assert_eq!(backoff.jitter_factor, 0.5);
}

#[test]
fn test_exponential_delays() {
    let backoff = ExponentialBackoff::new()
        .base_delay_ms(100)
        .multiplier(2.0)
        .jitter_factor(0.0); // No jitter for predictable testing

    let mut rng = StdRng::seed_from_u64(42);

    assert_eq!(backoff.delay(1, &mut rng), Some(100));
    assert_eq!(backoff.delay(2, &mut rng), Some(200));
    assert_eq!(backoff.delay(3, &mut rng), None); // Exceeds max_attempts (default 3)
}

#[test]
fn test_constant_backoff() {
    let backoff = ConstantBackoff::new()
        .delay_ms(500)
        .max_attempts(4)
        .jitter_factor(0.0);

    let mut rng = StdRng::seed_from_u64(42);

    assert_eq!(backoff.delay(1, &mut rng), Some(500));
    assert_eq!(backoff.delay(2, &mut rng), Some(500));
    assert_eq!(backoff.delay(3, &mut rng), Some(500));
    assert_eq!(backoff.delay(4, &mut rng), None);
}

#[test]
fn test_fibonacci_sequence() {
    assert_eq!(fibonacci(1), 1);
    assert_eq!(fibonacci(2), 1);
    assert_eq!(fibonacci(3), 2);
    assert_eq!(fibonacci(4), 3);
    assert_eq!(fibonacci(5), 5);
    assert_eq!(fibonacci(6), 8);
    assert_eq!(fibonacci(7), 13);
}

#[test]
fn test_fibonacci_backoff() {
    let backoff = FibonacciBackoff::new()
        .base_delay_ms(100)
        .max_attempts(5)
        .jitter_factor(0.0);

    let mut rng = StdRng::seed_from_u64(42);

    assert_eq!(backoff.delay(1, &mut rng), Some(100)); // 100 * 1
    assert_eq!(backoff.delay(2, &mut rng), Some(100)); // 100 * 1
    assert_eq!(backoff.delay(3, &mut rng), Some(200)); // 100 * 2
    assert_eq!(backoff.delay(4, &mut rng), Some(300)); // 100 * 3
    assert_eq!(backoff.delay(5, &mut rng), None); // Exceeds max_attempts
}

#[test]
fn test_jitter_application() {
    let backoff = ConstantBackoff::new().delay_ms(1000).jitter_factor(1.0); // Full jitter

    let mut rng = StdRng::seed_from_u64(42);
    let delays: Vec<u64> = (1..10).filter_map(|i| backoff.delay(i, &mut rng)).collect();

    // With full jitter, delays should vary
    let all_different = delays.windows(2).any(|w| w[0] != w[1]);
    assert!(all_different, "Full jitter should produce varying delays");

    // All delays should be <= base delay
    assert!(delays.iter().all(|&d| d <= 1000));
}
