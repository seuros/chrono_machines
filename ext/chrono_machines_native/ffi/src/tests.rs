use super::*;
use core::assert_matches;
use rand::SeedableRng;

#[test]
fn test_exponential_delay() {
    RNG.with(|rng| {
        *rng.borrow_mut() = StdRng::seed_from_u64(1337);
    });

    let delay = calculate_delay_exponential(1, 0.0004, 1.0, 0.001, 0.5);

    assert_matches!(delay, 0.0002..=0.0004);
}

#[test]
fn test_constant_delay() {
    RNG.with(|rng| {
        *rng.borrow_mut() = StdRng::seed_from_u64(42);
    });

    // Constant delay with 10% jitter should be 90-100% of base
    let delay = calculate_delay_constant(5, 1.0, 0.1);
    assert_matches!(delay, 0.9..=1.0);

    // No jitter should return exact value
    let delay = calculate_delay_constant(3, 0.5, 0.0);
    assert_eq!(delay, 0.5);
}

#[test]
fn test_fibonacci_delay() {
    RNG.with(|rng| {
        *rng.borrow_mut() = StdRng::seed_from_u64(123);
    });

    // Fibonacci sequence: 1, 1, 2, 3, 5, 8, 13...
    // Attempt 1: base_delay * 1 = 0.1
    let delay1 = calculate_delay_fibonacci(1, 0.1, 10.0, 0.0);
    assert_eq!(delay1, 0.1);

    // Attempt 5: base_delay * 5 = 0.5
    let delay5 = calculate_delay_fibonacci(5, 0.1, 10.0, 0.0);
    assert_eq!(delay5, 0.5);

    // Attempt 8: base_delay * 21 = 2.1
    let delay8 = calculate_delay_fibonacci(8, 0.1, 10.0, 0.0);
    assert_eq!(delay8, 2.1);
}

#[test]
fn test_attempt_clamped_into_u8() {
    const CASES: &[(i64, u8)] = &[
        (i64::MIN, 1),
        (-1, 1),
        (0, 1),
        (1, 1),
        (200, 200),
        (255, 255),
        (256, 255),
        (i64::MAX, 255),
    ];
    for &(attempt, expected) in CASES {
        assert_eq!(attempt_u8(attempt), expected, "attempt: {attempt}");
    }
}

#[test]
fn test_fibonacci_sequence() {
    assert_eq!(fibonacci(0), 0);
    assert_eq!(fibonacci(1), 1);
    assert_eq!(fibonacci(2), 1);
    assert_eq!(fibonacci(3), 2);
    assert_eq!(fibonacci(4), 3);
    assert_eq!(fibonacci(5), 5);
    assert_eq!(fibonacci(6), 8);
    assert_eq!(fibonacci(7), 13);
    assert_eq!(fibonacci(8), 21);
}
