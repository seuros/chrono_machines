# chrono-machines (Rust core)

Backoff timing for retries: how long to hold off after the `n`th consecutive
failure. `no_std`, allocation-free, no dependencies, no `unsafe`.

The core reads no clock, sleeps nowhere and owns no random generator. The
caller passes the uniform `u64` draw jitter consumes and does the waiting, so
the same type drives a blocking loop, an async task or a bare-metal timer.

```toml
[dependencies]
chrono-machines = "0.11.1" # x-release-please-version
```

## Shapes

Each shape is validated by its `new`, which answers `None` for one no delay
can come from (a base past the cap, a multiplier below one, a jitter outside
`0.0..=1.0`, NaN anywhere).

| Shape | Delay after failure `n` (before jitter) |
|-------|------------------------------------------|
| `Exponential::new(base, multiplier, max, jitter)` | `base * multiplier^(n-1)`, capped at `max` |
| `Constant::new(span, jitter)` | `span` |
| `Fibonacci::new(base, max, jitter)` | `base * fib(n)` (1, 1, 2, 3, 5, ...), capped at `max` |

Jitter takes `jitter * draw / 2^64` of the capped delay off, so a delay lies
in `capped * (1 - jitter) ..= capped`. A zero draw is the capped delay
exactly, which makes tests deterministic without a seeded generator.

```rust
use core::num::NonZeroU8;
use core::time::Duration;
use chrono_machines::{Fibonacci, Strategy};

let shape: Strategy = Fibonacci::new(Duration::from_millis(100), Duration::from_secs(5), 0.5)
    .ok_or("a valid shape")?
    .into();
let fifth = NonZeroU8::new(5).ok_or("non-zero")?;
assert_eq!(shape.delay(fifth, 0), Duration::from_millis(500));
assert_eq!(shape.delay(fifth, 1 << 63), Duration::from_millis(375));
# Ok::<(), &str>(())
```

## Retry

`Retry` counts consecutive failures against an attempt budget that includes
the first try (`None` is unbounded). Each failure answers its hold-off, or
`Refusal::Exhausted` once the budget is spent; a refusal changes nothing.
`hinted` takes a delay the failure named itself (an HTTP `Retry-After`) and
refuses one past the shape's cap with `Refusal::BeyondCap`. `succeeded` ends
the streak, so one value also serves a loop that never gives up.

A blocking retry loop, the operation and the generator being the caller's:

```rust
use core::num::NonZeroU8;
use core::time::Duration;
use chrono_machines::{Exponential, Refusal, Retry};

fn fetch(calls: &mut u32) -> Result<&'static str, &'static str> {
    *calls += 1;
    if *calls < 3 { Err("busy") } else { Ok("payload") }
}

let shape = Exponential::new(Duration::from_millis(1), 2.0, Duration::from_millis(8), 1.0)
    .ok_or("a valid shape")?;
let mut retry = Retry::new(shape.into(), NonZeroU8::new(5));
let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
let mut calls = 0;
let payload = loop {
    match fetch(&mut calls) {
        Ok(payload) => break payload,
        Err(error) => match retry.failed(seed) {
            Ok(delay) => std::thread::sleep(delay),
            Err(Refusal::Exhausted | Refusal::BeyondCap) => return Err(error),
        },
    }
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
};
assert_eq!((payload, retry.failures()), ("payload", 2));
# Ok::<(), &str>(())
```

An async loop is the same with the runtime's timer in place of
`thread::sleep`.
