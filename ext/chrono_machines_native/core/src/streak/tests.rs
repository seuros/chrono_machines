use rand::SeedableRng;
use rand::rngs::StdRng;

use super::Backoff;
use crate::{ConstantBackoff, ExponentialBackoff};

fn exact(max_attempts: u8) -> ExponentialBackoff {
    ExponentialBackoff::new()
        .max_attempts(max_attempts)
        .base_delay_ms(100)
        .multiplier(2.0)
        .max_delay_ms(1_000)
        .jitter_factor(0.0)
}

#[test]
fn delay_grows_with_the_streak_and_resets_on_success() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut backoff = Backoff::new(exact(u8::MAX));
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(100));
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(200));
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(400));
    assert_eq!(backoff.failures(), 3);
    backoff.succeeded();
    assert_eq!(backoff.failures(), 0);
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(100));
}

#[test]
fn a_bounded_strategy_is_spent_after_its_attempts() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut backoff = Backoff::new(exact(3));
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(100));
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(200));
    assert_eq!(backoff.failed_with_rng(&mut rng), None);
    assert_eq!(backoff.failed_with_rng(&mut rng), None);
    backoff.succeeded();
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(100));
}

#[test]
fn an_unbounded_strategy_is_never_spent() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut backoff = Backoff::new(exact(u8::MAX));
    for _ in 0..1_000 {
        assert!(backoff.failed_with_rng(&mut rng).is_some());
    }
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(1_000));
    assert_eq!(backoff.failures(), u8::MAX - 1);
}

#[test]
fn any_strategy_works() {
    let mut rng = StdRng::seed_from_u64(1);
    let strategy = ConstantBackoff::new()
        .delay_ms(50)
        .max_attempts(2)
        .jitter_factor(0.0);
    let mut backoff = Backoff::new(strategy);
    assert_eq!(backoff.failed_with_rng(&mut rng), Some(50));
    assert_eq!(backoff.failed_with_rng(&mut rng), None);
}

#[cfg(feature = "std")]
#[test]
fn the_std_entry_point_jitters_within_the_strategy() {
    let strategy = ExponentialBackoff::new()
        .max_attempts(u8::MAX)
        .base_delay_ms(1_000)
        .jitter_factor(0.25);
    let mut backoff = Backoff::new(strategy);
    let delay = backoff.failed().expect("unbounded");
    assert!((750..=1_000).contains(&delay), "{delay}");
}
