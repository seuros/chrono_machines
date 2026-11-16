# ChronoMachines

> The temporal manipulation engine that rewrites the rules of retry!

Backoff timing for Ruby over a native Rust core: exponential, constant and
Fibonacci shapes with full jitter, a failure-streak counter that refuses once
the attempt budget is spent, and a blocking retry runtime that honours
`deadline_machines` budgets. Every span is Integer nanoseconds.

## Quick Start

```ruby
gem 'chrono_machines'
```

```ruby
MS = 1_000_000
BACKOFF = ChronoMachines::Exponential.new(100 * MS, 2.0, 10_000 * MS, 1.0)

result = ChronoMachines.retry(BACKOFF, 5, retryable: [Timeout::Error]) do
  perform_risky_operation
end
```

## A Message from the Time Lords

So your microservices are failing faster than your deployment pipeline can recover, and you're stuck in an infinite loop of "let's just add more retries"?

Welcome to the temporal wasteland—where every millisecond matters, exponential backoff is law, and jitter isn't just a feeling you get when watching your error rates spike.

Still here? Excellent. Because in the fabric of spacetime, nobody can hear your servers screaming about cascading failures. It's all just timing and patience.

### The Pattern Time Forgot

Built for Ruby 4.0+ on a `no_std` Rust core that reads no clock and owns no
random generator, because when you're manipulating time itself, precision
matters. The core decides; you sleep.

## The Temporal Manifesto

### You Think: "I'll just add `retry` and call it resilience!"
### Reality: You're creating a time paradox that crashes your entire fleet

When your payment service fails, you don't want to hammer it into submission. You want to approach it like a time traveler—carefully, with exponential patience, and a healthy respect for the butterfly effect.

## Shapes

Each shape is a frozen `Data` value, validated by the core when built (by the
gem or by hand): `TypeError` for the wrong type, `RangeError` for a shape no
delay can come from. Spans are Integer nanoseconds in `0..2**64-1`; the reals
are stored as Floats.

| Shape | Delay after failure `n`, before jitter |
|-------|----------------------------------------|
| `Exponential.new(base, multiplier, max, jitter)` | `base * multiplier**(n - 1)`, capped at `max` (`base <= max`, finite `multiplier >= 1.0`) |
| `Constant.new(span, jitter)` | `span`, with no cap |
| `Fibonacci.new(base, max, jitter)` | `base * fib(n)` (1, 1, 2, 3, 5, ...), capped at `max` (`base <= max`) |

`shape.delay(attempt, draw)` answers the hold-off after failure number
`attempt` (1..255); `shape.cap` answers the cap, or nil for a `Constant`.

```ruby
fib = ChronoMachines::Fibonacci.new(100 * MS, 5_000 * MS, 0.5)
fib.delay(5, 0)        # => 500_000_000, a zero draw is the undithered delay
fib.delay(5, 2**63)    # => 375_000_000
fib.delay(5, Random.rand(2**64))
```

## The Science of Temporal Jitter

`jitter` is the share of a delay a draw may take off: with a draw `d` uniform
over `0...2**64`, the delay is `capped - capped * jitter * d / 2**64`, so it
lies in `capped * (1 - jitter)..capped`. `1.0` is full jitter, `0.0` none.

```ruby
# Instead of predictable delays that create thundering herds:
#   100ms, 200ms, 400ms
# full jitter spreads every caller over:
#   0..100ms, 0..200ms, 0..400ms
```

The core never draws for you. Pass `Random.rand(2**64)`, a seeded `Random`
in tests, or `0` when you want the cap of the range.

## Retry

`ChronoMachines::Retry.new(strategy, max_attempts)` counts consecutive
failures against a budget that includes the first try (`1..255`, or nil for
unbounded). Refusals are symbols and change nothing.

```ruby
streak = ChronoMachines::Retry.new(BACKOFF, 3)
streak.failed(Random.rand(2**64))  # => Integer nanoseconds to wait
streak.hinted(2_000 * MS)          # => 2_000_000_000, a Retry-After taken verbatim
streak.failed(0)                   # => :exhausted, the third attempt was the last
streak.failures                    # => 2
streak.waited                      # => nanoseconds answered so far
streak.succeeded                   # the streak ends; the next failure is the first again
```

`hinted` answers `:beyond_cap` for a hint longer than the shape's cap: the
failure asks for more patience than the policy allows. Exhaustion wins over
any hint. Mutating calls check frozen first (`FrozenError`), then their
arguments (`TypeError`/`RangeError`), before the core is asked.

## The Executor

`ChronoMachines::Executor.new(strategy, max_attempts, **runtime)` and
`ChronoMachines.retry(strategy, max_attempts, **runtime) { ... }` run a block
under a fresh `Retry`, sleeping each hold-off. The keywords belong to this
pure-Ruby runtime, not the core:

| Keyword | Default | Meaning |
|---------|---------|---------|
| `retryable:` | `[StandardError]` | exception classes worth retrying; anything else is re-raised at once |
| `delay_from:` | nil | `->(exception:, attempt:)`: nil keeps the shape, Integer nanoseconds replace its delay, `:halt` gives up |
| `on_retry:` | nil | `->(exception:, attempt:, next_delay:)`, `next_delay` in nanoseconds |
| `on_success:` | nil | `->(result:, attempts:)` |
| `on_failure:` | nil | `->(exception:, attempts:)`; one that raises does not mask the exception |
| `random:` | `Random` | where draws come from: anything answering `rand(2**64)` |

Exhaustion raises `ChronoMachines::MaxRetriesExceededError` (with
`original_exception` and `attempts`); a halt, a hint beyond the cap and an
exception outside `retryable:` re-raise the exception itself.

```ruby
fetch = ChronoMachines::Executor.new(
  BACKOFF, 5,
  retryable: [RateLimited, Timeout::Error],
  delay_from: ->(exception:, attempt:) { exception.retry_after_ns if exception.is_a?(RateLimited) },
  on_retry: ->(exception:, attempt:, next_delay:) { log(attempt, next_delay) }
)
fetch.call { client.get('/vacancies') }
```

### Deadlines

Every hold-off is clamped to the ambient `deadline_machines` budget. When the
caller's own budget is spent the executor stops retrying and raises
`DeadlineMachines::Exceeded`; a deadline error from a narrower inner scope is
retried like any other failure.

```ruby
DeadlineMachines.with_deadline(2 * 1_000_000_000) do
  ChronoMachines.retry(BACKOFF, 10) { flaky_call }
end
```

### Scheduling without sleeping

A job queue that reschedules instead of sleeping asks the shape directly:
`BACKOFF.delay(executions, Random.rand(2**64))` is the wait before the next
run. See [docs/RAILS_INTEGRATION.md](docs/RAILS_INTEGRATION.md).

## Rust

The core is the `chrono-machines` crate: `no_std`, allocation-free, no
dependencies. See its [README](ext/chrono_machines_native/core/README.md).

## A Word from the Time Corps Engineering Division

*The Temporal Commentary Engine activates:*

"Time isn't linear—especially when your payment processor is having 'a moment.'

The universe doesn't care about your startup's burn rate or your post on X about 'building in public.' It cares about one immutable law:

**Does your system handle failure gracefully across the fourth dimension?**

If not, welcome to the Time Corps. We have exponential backoff.

Remember: The goal isn't to prevent temporal anomalies—it's to fail fast, fail smart, and retry with mathematical precision.

As I always say when debugging production: 'Time heals all wounds, but jitter prevents thundering herds.'"

*— Temporal Commentary Engine, Log Entry ∞*

## Contributing to the Timeline

1. Fork it (like it's 2005, but with better temporal mechanics)
2. Create your feature branch (`git checkout -b feature/quantum-retries`)
3. Commit your changes (`git commit -am 'Add temporal stabilization'`)
4. Push to the branch (`git push origin feature/quantum-retries`)
5. Create a new Pull Request (and wait for the Time Lords to review)

## 📡 Credits

**AMS Metcalfe**, NCC-1973, *Metcalfe*-class tender.

Named for Robert Metcalfe, whose 1973 Ethernet memo at Xerox PARC started the
shared network that, in Metcalfe and Boggs's 1976 paper, settled collisions by
having each station wait a random, exponentially growing number of slots
before trying again: randomised exponential backoff.

## License

MIT License. See [LICENSE](LICENSE) file for details.

## Acknowledgments

- The Ruby community - For building a language worth retrying for
- Every timeout that ever taught us patience - You made us stronger
- The Time Corps - For maintaining temporal stability
- The universe - For being deterministically random

## Author

Built with time and Brawndo (it's got electrolytes) by temporal engineers fighting entropy one retry at a time.

**Remember: In the fabric of spacetime, nobody can hear your API timeout. But they can feel your exponential backoff working as intended.**
