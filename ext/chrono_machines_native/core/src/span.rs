//! Span arithmetic shared by the strategies: growth factors, capping and
//! jitter, all in nanoseconds.

use core::time::Duration;

const NANOS_PER_SEC: u128 = 1_000_000_000;

/// 2^64 as an `f64`: a draw divided by it is a fraction in `0.0..=1.0`.
const DRAW_SPAN: f64 = 18_446_744_073_709_551_616.0;

/// `n` widened to `f64`. Exact below 2^53, which is every span under about
/// 104 days in nanoseconds; past that only the lowest nanoseconds round.
#[expect(
    clippy::cast_precision_loss,
    reason = "inherent to integer -> f64; exact below 2^53"
)]
const fn lossy(n: u128) -> f64 {
    n as f64
}

/// `ns` truncated to whole nanoseconds. Callers pass a finite, non-negative
/// value below a cap that already fits a `Duration`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "`as` is the saturating float -> int conversion; NaN becomes 0"
)]
const fn truncate(ns: f64) -> u128 {
    ns as u128
}

/// `ns` nanoseconds as a `Duration`, saturating at `Duration::MAX`.
fn from_nanos(ns: u128) -> Duration {
    let (Ok(secs), Ok(nanos)) = (
        u64::try_from(ns / NANOS_PER_SEC),
        u32::try_from(ns % NANOS_PER_SEC),
    ) else {
        return Duration::MAX;
    };
    Duration::new(secs, nanos)
}

/// `base^exp` by squaring: `f64::powi` lives in `std`.
pub const fn power(base: f64, exp: u8) -> f64 {
    let mut acc = 1.0_f64;
    let mut factor = base;
    let mut n = exp;
    while n > 0 {
        if n & 1 == 1 {
            acc *= factor;
        }
        factor *= factor;
        n >>= 1;
    }
    acc
}

/// The `n`th Fibonacci number, 1-indexed (1, 1, 2, 3, 5, ...), as an `f64`.
///
/// It passes 2^53 at n = 79 and `u64::MAX` at n = 94, so an integer
/// accumulator would stop short of the attempt range and scale to a delay
/// under the cap that is silently wrong; the `f64` keeps growing and leaves
/// the decision to the cap.
pub fn fibonacci(n: u8) -> f64 {
    let (mut previous, mut current) = (0.0_f64, 1.0_f64);
    for _ in 0..n {
        (previous, current) = (current, previous + current);
    }
    previous
}

/// `span * factor`, never more than `cap`. A factor of exactly one is the
/// span itself, untouched by rounding; an infinite product is the cap; a
/// zero span stays zero whatever the factor.
pub fn scaled(span: Duration, factor: f64, cap: Duration) -> Duration {
    if span.is_zero() {
        return Duration::ZERO;
    }
    if factor.total_cmp(&1.0).is_eq() {
        return span.min(cap);
    }
    let ns = lossy(span.as_nanos()) * factor;
    if ns >= lossy(cap.as_nanos()) {
        return cap;
    }
    from_nanos(truncate(ns)).min(cap)
}

/// `span` with a share of it, `jitter * draw / 2^64`, taken off: the result
/// lies in `span * (1 - jitter) ..= span`. A zero draw or a zero jitter is
/// the span itself, exactly.
pub fn jittered(span: Duration, jitter: f64, draw: u64) -> Duration {
    if draw == 0 || jitter <= 0.0 {
        return span;
    }
    let share = jitter * (lossy(u128::from(draw)) / DRAW_SPAN);
    let cut = from_nanos(truncate(lossy(span.as_nanos()) * share));
    span.saturating_sub(cut)
}

#[cfg(test)]
mod tests;
