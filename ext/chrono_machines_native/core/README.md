# ChronoMachines (Rust Core)

Pure Rust exponential backoff and retry library with full jitter support.

## Features

- **Multiple Backoff Strategies**: Exponential, Constant, and Fibonacci
- **Full Jitter**: Prevents thundering herd problem by randomizing delays
- **`no_std` Compatible**: Works in embedded environments (with optional `alloc`)
- **Zero Allocation**: Core delay calculations use stack-only data structures
- **Retry Builder**: Fluent `.retry()` API returning rich `RetryOutcome`
- **Instrumentation Hooks**: Separate `notify`, `on_success`, and `on_failure` callbacks
- **Named Policies**: Optional registry (std/alloc) with global helpers
- **ESP32 Ready**: Tested on embedded systems
- **Fast**: Minimal overhead for retry operations

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
chrono_machines = "0.2"
```

### Basic Example

```rust
use chrono_machines::Policy;

let policy = Policy {
    max_attempts: 5,
    base_delay_ms: 100,
    multiplier: 2.0,
    max_delay_ms: 10_000,
};

// Calculate delay for first retry using full jitter (1.0)
let delay_ms = policy.calculate_delay(1, 1.0);
println!("Wait {}ms before retry", delay_ms);
```

### With Custom RNG (no_std)

```rust
use chrono_machines::Policy;
use rand::rngs::SmallRng;
use rand::SeedableRng;

let policy = Policy::default();
let mut rng = SmallRng::seed_from_u64(12345);

// Calculate delay with 50% jitter (0.5)
let delay = policy.calculate_delay_with_rng(1, 0.5, &mut rng);
```

### Fluent Retry Builder

```rust
use chrono_machines::{backoff::ExponentialBackoff, retry::Retryable};

let mut attempts = 0;
let operation = || {
    attempts += 1;
    if attempts < 3 {
        Err("not yet")
    } else {
        Ok("success")
    }
};

# #[cfg(feature = "std")]
let outcome = operation
    .retry(ExponentialBackoff::default().max_attempts(5))
    .notify(|err, attempt, delay| {
        println!("attempt {attempt} failed ({err}), next delay {delay}ms");
    })
    .on_success(|value, attempt| {
        println!("attempt {attempt} succeeded with {value}");
    })
    .on_failure(|err| {
        eprintln!("retry stopped: {err}");
    })
    .call()
    .expect("retry succeeds");

println!("took {} attempts", outcome.attempts());
println!("value = {}", outcome.into_inner());
```

### Named Policies & DSL (requires `std`)

```rust
use chrono_machines::{
    backoff::{BackoffPolicy, ExponentialBackoff},
    register_global_policy,
    retry_with_policy,
};

register_global_policy(
    "api",
    BackoffPolicy::from(ExponentialBackoff::new().base_delay_ms(250).max_attempts(4)),
);

let outcome = retry_with_policy("api", || {
    // fallible operation
    Ok::<_, &'static str>("done")
})
.expect("named policy is registered");

assert_eq!(outcome.attempts(), 1);
```

## Algorithm

ChronoMachines implements **full jitter** exponential backoff:

```
delay = random(0, min(base * multiplier^(attempt-1), max))  // with jitter_factor = 1.0
```

This approach:
1. Calculates exponential backoff: `base * multiplier^(attempt-1)`
2. Caps at `max_delay_ms`
3. Applies configurable jitter: blends between deterministic and random delay based on `jitter_factor`

### Why Full Jitter?

Full jitter prevents the "thundering herd" problem where multiple clients retry simultaneously,
overwhelming a recovering service. By randomizing the delay, retries are naturally distributed
over time.

## Features

### `std` (default)

Enables standard library support and `StdRng` for `calculate_delay()` method.

### `no_std`

Disable default features for `no_std` environments:

```toml
[dependencies]
chrono_machines = { version = "0.2", default-features = false }
```

You'll need to provide your own RNG and use `calculate_delay_with_rng()`.

### `alloc`

Enable the lightweight, vector-backed `PolicyRegistry` without the standard
library:

```toml
[dependencies]
chrono_machines = { version = "0.2", default-features = false, features = ["alloc"] }
```

## Backoff Strategies

### Exponential Backoff
Delays grow exponentially: `base * multiplier^(attempt-1)`

```rust
use chrono_machines::{ExponentialBackoff, Retryable};

operation.retry(
    ExponentialBackoff::new()
        .base_delay_ms(100)
        .multiplier(2.0)
        .max_delay_ms(10_000)
).call()
```

### Constant Backoff
Fixed delay with optional jitter

```rust
use chrono_machines::{ConstantBackoff, Retryable};

operation.retry(
    ConstantBackoff::new()
        .delay_ms(500)
        .jitter_factor(0.1)
).call()
```

### Fibonacci Backoff
Delays grow by Fibonacci sequence: 1, 1, 2, 3, 5, 8, 13...

```rust
use chrono_machines::{FibonacciBackoff, Retryable};

operation.retry(
    FibonacciBackoff::new()
        .base_delay_ms(100)
        .max_delay_ms(5_000)
).call()
```

## License

MIT
