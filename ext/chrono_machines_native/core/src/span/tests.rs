use super::*;
use matryona_machines::testing::XorShift64;

const MS: Duration = Duration::from_millis(1);

#[test]
fn power_and_fibonacci() {
    assert_eq!(power(2.0, 0), 1.0);
    assert_eq!(power(2.0, 10), 1024.0);
    assert_eq!(power(1.5, 3), 3.375);
    assert_eq!(power(100.0, u8::MAX), f64::INFINITY);
    let sequence: [f64; 9] = core::array::from_fn(|n| fibonacci(u8::try_from(n).unwrap_or(0)));
    assert_eq!(sequence, [0.0, 1.0, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 21.0]);
    // Past u64::MAX the factor keeps growing instead of stopping at a ceiling.
    assert!(fibonacci(100) > fibonacci(94));
    assert!(fibonacci(94) > lossy(u128::from(u64::MAX)));
}

#[test]
fn scaled_caps_and_keeps_exact_spans_exact() {
    let huge = Duration::new(u64::MAX / 2, 123_456_789);
    // A factor of one leaves even a span past 2^53 ns untouched.
    assert_eq!(scaled(huge, 1.0, Duration::MAX), huge);
    assert_eq!(scaled(huge, 1.0, MS), MS);
    assert_eq!(scaled(MS * 100, 2.0, Duration::MAX), MS * 200);
    assert_eq!(scaled(MS * 100, 2.0, MS * 150), MS * 150);
    assert_eq!(scaled(MS, f64::INFINITY, MS * 7), MS * 7);
    // Zero stays zero, even against an infinite factor (0 * inf is NaN).
    assert_eq!(scaled(Duration::ZERO, f64::INFINITY, MS), Duration::ZERO);
    assert_eq!(
        scaled(Duration::from_nanos(3), 1.5, Duration::MAX),
        Duration::from_nanos(4)
    );
    assert_eq!(scaled(Duration::MAX, 2.0, Duration::MAX), Duration::MAX);
}

#[test]
fn jitter_takes_a_share_off_and_never_adds() {
    let span = Duration::new(u64::MAX, 999_999_999);
    assert_eq!(jittered(span, 1.0, 0), span);
    assert_eq!(jittered(span, 0.0, u64::MAX), span);
    assert_eq!(jittered(MS * 1000, 1.0, 1 << 63), MS * 500);
    assert_eq!(jittered(MS * 1000, 0.5, 1 << 63), MS * 750);
    // The top draw rounds to a share of one: full jitter reaches zero.
    assert_eq!(jittered(MS * 1000, 1.0, u64::MAX), Duration::ZERO);
    let mut rng = XorShift64::new(7);
    for jitter in [0.1, 0.5, 1.0] {
        for _ in 0..1_000 {
            let delay = jittered(MS * 1000, jitter, rng.next_u64());
            assert!(delay <= MS * 1000, "{delay:?}");
            assert!(
                delay.as_secs_f64() >= 1.0 - jitter - 1e-9,
                "{delay:?} at {jitter}"
            );
        }
    }
}

#[test]
fn from_nanos_saturates() {
    assert_eq!(from_nanos(1_500_000_000), Duration::new(1, 500_000_000));
    assert_eq!(from_nanos(Duration::MAX.as_nanos()), Duration::MAX);
    assert_eq!(from_nanos(u128::MAX), Duration::MAX);
}
