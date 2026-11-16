use core::num::NonZeroU8;
use core::time::Duration;

use matryona_machines::testing::{XorShift64, refused};

use super::*;

const MS: Duration = Duration::from_millis(1);

fn attempt(n: u8) -> NonZeroU8 {
    NonZeroU8::new(n).expect("a test attempt is non-zero")
}

/// The attempt at array index `n`, counting from one.
fn nth(n: usize) -> NonZeroU8 {
    attempt(u8::try_from(n + 1).expect("a short test sequence"))
}

fn exponential(base: u32, multiplier: f64, max: u32, jitter: f64) -> Exponential {
    Exponential::new(MS * base, multiplier, MS * max, jitter).expect("a valid shape")
}

fn constant(span: u32, jitter: f64) -> Constant {
    Constant::new(MS * span, jitter).expect("a valid shape")
}

fn fibonacci(base: u32, max: u32, jitter: f64) -> Fibonacci {
    Fibonacci::new(MS * base, MS * max, jitter).expect("a valid shape")
}

#[test]
fn a_shape_no_delay_comes_from_does_not_exist() {
    for jitter in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert_eq!(Exponential::new(MS, 2.0, MS, jitter), None, "{jitter}");
        assert_eq!(Constant::new(MS, jitter), None, "{jitter}");
        assert_eq!(Fibonacci::new(MS, MS, jitter), None, "{jitter}");
    }
    for multiplier in [0.0, 0.5, -2.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            Exponential::new(MS, multiplier, MS, 0.0),
            None,
            "{multiplier}"
        );
    }
    assert_eq!(Exponential::new(MS * 2, 2.0, MS, 0.0), None);
    assert_eq!(Fibonacci::new(MS * 2, MS, 0.0), None);
    // The edges are valid: no growth, no jitter, full jitter, zero spans.
    assert!(Exponential::new(Duration::ZERO, 1.0, Duration::ZERO, 1.0).is_some());
    assert!(Constant::new(Duration::ZERO, 0.0).is_some());
    assert!(Fibonacci::new(Duration::MAX, Duration::MAX, 1.0).is_some());
    let shape = exponential(100, 2.5, 900, 0.25);
    let fields = (
        shape.base(),
        shape.multiplier(),
        shape.max(),
        shape.jitter(),
    );
    assert_eq!(fields, (MS * 100, 2.5, MS * 900, 0.25));
    assert_eq!(
        (constant(7, 0.5).span(), constant(7, 0.5).jitter()),
        (MS * 7, 0.5)
    );
    let fib = fibonacci(3, 9, 1.0);
    assert_eq!((fib.base(), fib.max(), fib.jitter()), (MS * 3, MS * 9, 1.0));
}

#[test]
fn unjittered_shapes_follow_their_sequences_to_the_cap() {
    let grow = exponential(100, 2.0, 1_000, 0.0);
    let delays: [Duration; 6] = core::array::from_fn(|n| grow.delay(nth(n), 9));
    assert_eq!(delays, [100, 200, 400, 800, 1_000, 1_000].map(|ms| MS * ms));
    assert_eq!(grow.delay(attempt(u8::MAX), 0), MS * 1_000);
    let fib = fibonacci(100, 1_000, 0.0);
    let delays: [Duration; 8] = core::array::from_fn(|n| fib.delay(nth(n), 9));
    assert_eq!(
        delays,
        [100, 100, 200, 300, 500, 800, 1_000, 1_000].map(|ms| MS * ms)
    );
    assert_eq!(fib.delay(attempt(u8::MAX), 0), MS * 1_000);
    assert_eq!(constant(250, 0.0).delay(u64::MAX), MS * 250);
    // A shape that never grows answers its base forever.
    assert_eq!(exponential(5, 1.0, 5, 0.0).delay(attempt(200), 0), MS * 5);
}

#[test]
fn strategy_dispatches_and_reports_the_cap() {
    let shapes: [Strategy; 3] = [
        exponential(10, 3.0, 50, 0.0).into(),
        constant(10, 0.0).into(),
        fibonacci(10, 50, 0.0).into(),
    ];
    let third = shapes.map(|shape| shape.delay(attempt(3), 0));
    assert_eq!(third, [MS * 50, MS * 10, MS * 20]);
    assert_eq!(
        shapes.map(|shape| shape.cap()),
        [Some(MS * 50), None, Some(MS * 50)]
    );
}

#[test]
fn jitter_keeps_every_delay_inside_its_share() {
    let mut rng = XorShift64::new(42);
    for jitter in [0.0, 0.1, 0.5, 1.0] {
        let shapes: [Strategy; 3] = [
            exponential(100, 2.0, 10_000, jitter).into(),
            constant(100, jitter).into(),
            fibonacci(100, 10_000, jitter).into(),
        ];
        for shape in shapes {
            for n in 1..=10 {
                let ceiling = Strategy::delay(&shape, attempt(n), 0);
                let delay = shape.delay(attempt(n), rng.next_u64());
                assert!(delay <= ceiling, "{shape:?} {n}: {delay:?}");
                let floor = ceiling.as_secs_f64() * (1.0 - jitter);
                assert!(
                    delay.as_secs_f64() >= floor - 1e-9,
                    "{shape:?} {n}: {delay:?}"
                );
            }
        }
    }
}

#[test]
fn the_budget_counts_the_first_try() {
    let mut retry = Retry::new(exponential(100, 2.0, 1_000, 0.0).into(), NonZeroU8::new(3));
    assert_eq!(retry.failed(0), Ok(MS * 100));
    assert_eq!(retry.failed(0), Ok(MS * 200));
    assert_eq!((retry.failures(), retry.waited()), (2, MS * 300));
    assert_eq!(
        refused(&mut retry, |retry| retry.failed(0)),
        Refusal::Exhausted
    );
    // Exhaustion wins over any hint, which is refused unchanged too.
    assert_eq!(
        refused(&mut retry, |retry| retry.hinted(MS)),
        Refusal::Exhausted
    );
    assert_eq!(
        refused(&mut retry, |retry| retry.hinted(MS * 5_000)),
        Refusal::Exhausted
    );
    let mut once = Retry::new(constant(1, 0.0).into(), NonZeroU8::new(1));
    assert_eq!(
        refused(&mut once, |retry| retry.failed(0)),
        Refusal::Exhausted
    );
    assert_eq!(once.max_attempts(), NonZeroU8::new(1));
    assert_eq!(once.strategy(), Strategy::from(constant(1, 0.0)));
}

#[test]
fn a_hint_is_taken_verbatim_up_to_the_cap() {
    let mut retry = Retry::new(exponential(100, 2.0, 1_000, 1.0).into(), None);
    assert_eq!(retry.hinted(MS * 1_000), Ok(MS * 1_000));
    assert_eq!(
        refused(&mut retry, |retry| retry.hinted(MS * 1_001)),
        Refusal::BeyondCap
    );
    assert_eq!((retry.failures(), retry.waited()), (1, MS * 1_000));
    // A hint advances the streak like a failure: the next delay is the second.
    let mut fixed = Retry::new(exponential(100, 2.0, 1_000, 0.0).into(), None);
    assert_eq!(fixed.hinted(MS * 7), Ok(MS * 7));
    assert_eq!(fixed.failed(0), Ok(MS * 200));
    // A constant shape caps nothing.
    let mut uncapped = Retry::new(constant(1, 0.0).into(), None);
    assert_eq!(uncapped.hinted(Duration::MAX), Ok(Duration::MAX));
    assert_eq!(uncapped.hinted(MS), Ok(MS));
    assert_eq!(uncapped.waited(), Duration::MAX);
}

#[test]
fn success_ends_the_streak_and_unbounded_never_refuses() {
    let mut retry = Retry::new(fibonacci(10, 1_000, 0.0).into(), NonZeroU8::new(3));
    assert_eq!(retry.failed(0), Ok(MS * 10));
    assert_eq!(retry.failed(0), Ok(MS * 10));
    retry.succeeded();
    assert_eq!((retry.failures(), retry.waited()), (0, Duration::ZERO));
    assert_eq!(retry.failed(0), Ok(MS * 10));
    let mut forever = Retry::new(exponential(1, 2.0, 60_000, 0.0).into(), None);
    for _ in 0..600 {
        assert!(forever.failed(u64::MAX).is_ok());
    }
    assert_eq!(forever.failures(), u8::MAX);
    assert_eq!(forever.failed(0), Ok(MS * 60_000));
}

#[test]
fn refusals_read_distinctly() {
    use core::fmt::Write;
    struct Line([u8; 64], usize);
    impl Write for Line {
        fn write_str(&mut self, text: &str) -> core::fmt::Result {
            let end = self.1 + text.len();
            self.0
                .get_mut(self.1..end)
                .ok_or(core::fmt::Error)?
                .copy_from_slice(text.as_bytes());
            self.1 = end;
            Ok(())
        }
    }
    let read = |refusal: Refusal| {
        let mut line = Line([0; 64], 0);
        write!(line, "{refusal}").expect("fits");
        (line.0, line.1)
    };
    assert_ne!(read(Refusal::Exhausted), read(Refusal::BeyondCap));
}
