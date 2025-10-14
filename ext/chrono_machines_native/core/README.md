# ChronoMachines (Rust Core)

Pure Rust exponential backoff and retry library with full jitter support.

## Features

- **Full Jitter**: Prevents thundering herd problem by randomizing delays
- **`no_std` Compatible**: Works in embedded environments
- **Zero Allocation**: Stack-only data structures
- **Fast**: Minimal overhead for delay calculations
- **Standalone**: No external dependencies besides `rand`

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
chrono_machines = "0.1"
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
chrono_machines = { version = "0.1", default-features = false }
```

You'll need to provide your own RNG and use `calculate_delay_with_rng()`.

## License

MIT
