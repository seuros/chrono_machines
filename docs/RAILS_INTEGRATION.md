# Rails Integration

ChronoMachines calculates retry delays. Active Job and your queue backend schedule jobs; your application decides which operations can be repeated and when they are complete. These examples ask a shape for its delay with `shape.delay(attempt, draw)`, which answers Integer nanoseconds, and require no adapter.

Hilbert's Hotel is fully occupied and still taking reservations. The concierge is moving guest `n` to room `2n`; the vacancy board is understandably out of date.

The examples assume an application service, `VacancyBoardRefresh.call(hotel_id)`, that reads occupancy and updates a local vacancy board. It can safely refresh the same board more than once. Its client raises `HilbertHotel::ConciergeUnavailable` for retryable failures. That error exposes `retry_after_ns`, the server's hint in Integer nanoseconds, or `nil`, with any HTTP header parsing handled by the client.

## Using `retry_on`

Use Rails' built-in retry handling when the delay only needs the failed attempt number:

```ruby
class VacancyBoardRefreshJob < ApplicationJob
  S = 1_000_000_000
  BACKOFF = ChronoMachines::Exponential.new(1 * S, 2.0, 60 * S, 0.5)

  retry_on HilbertHotel::ConciergeUnavailable,
           attempts: 5,
           wait: ->(attempt) { Rational(BACKOFF.delay(attempt, Random.rand(2**64)), S) }

  def perform(hotel_id)
    VacancyBoardRefresh.call(hotel_id)
  end
end
```

Rails counts the attempts, including the initial execution, and only calls `wait` before its limit is reached; the shape has no budget of its own, so there is nothing to keep in step. The wait proc converts nanoseconds to the seconds Rails takes.

Rails selects the exceptions to retry and serializes its retry counters. Chrono supplies the backoff and jitter; Rails uses the wait proc's result directly. This preserves normal `retry_on`, `discard_on`, and `rescue_from` behavior, including Rails' handling of wrapped exception causes.

The wait proc receives only the attempt number. Use explicit scheduling when the delay depends on the error.

## Scheduling durable retries with server hints

Use a persistent queue backend, and carry the next attempt in the new job's arguments:

```ruby
class VacancyBoardRefreshWithHintsJob < ApplicationJob
  S = 1_000_000_000
  ATTEMPTS = 5
  BACKOFF = ChronoMachines::Exponential.new(1 * S, 2.0, 60 * S, 0.5)

  def perform(hotel_id, attempt: 1)
    VacancyBoardRefresh.call(hotel_id)
  rescue HilbertHotel::ConciergeUnavailable => error
    raise if attempt >= ATTEMPTS

    hint = error.retry_after_ns
    raise if hint && hint > BACKOFF.cap

    delay = hint || BACKOFF.delay(attempt, Random.rand(2**64))
    self.class.set(wait: Rational(delay, S)).perform_later(hotel_id, attempt: attempt + 1)
  end
end
```

The queue stores the scheduled time and arguments, so the count survives process restarts. The first failure uses `attempt: 1`; at the fifth the original exception is raised without scheduling another job.

A hint is honoured without jitter; one above the cap stops, it is never shortened to the cap. This is the rule `ChronoMachines::Retry#hinted` applies in process, where it answers `:beyond_cap`. A zero delay is valid.

This example rescues the client's error directly. If your client wraps errors, classify them explicitly or use Rails' exception handlers; a Ruby `rescue` does not search exception causes.

For workflows with recovery sweeps, persist the failure count and next check time on the execution record. A sweep can enqueue due work after a worker interruption. Keep readiness polls separate from failed attempts, and preserve the same operation identifier across retries.

## Callbacks

`ChronoMachines.retry` and `Executor#call` run `on_retry`, `on_success` and `on_failure` around their own inline retry loop, which waits within the current execution. A shape's `delay` runs none of them.

For scheduled jobs, invoke your own instrumentation at explicit application events:

| Event | When it happens |
| --- | --- |
| retry | After the queue has accepted the retry. |
| success | When the operation has actually completed. |
| failure | When the application decides the operation has permanently failed. |

A job that schedules another readiness poll has not finished its operation. Returning from `perform` or running `after_perform` is not enough to report success. Likewise, an interruption that will resume is not a permanent failure. Instrumentation failures should not hide the original error or cause a completed side effect to run again.

## Retry ownership and safety

- Choose retryable errors per operation. Avoid a blanket retry policy for `StandardError` on every job.
- A timeout may happen after a remote side effect succeeds. For charges and refunds, persist a provider-supported idempotency key for the operation and reuse it on every attempt, including manual retries. An attempt number or a newly generated job ID is not a stable operation key.
- Use one retry path for each failure. `retry_on` and `discard_on` follow `rescue_from` handler precedence. An error raised after Chrono stops can still reach a Rails handler or the queue backend's retry policy; configure those limits together.
- The `set(wait_until:)` example creates a new job. For `ActiveJob::Continuable`, use Rails' checkpoints and resumption flow to preserve progress, and decide retry behavior for each step. A resumption should not automatically count as another failure or as completion.

## Application checks

Use Active Job's test adapter and your application's existing test data to check the chosen pattern:

- Retry a transient failure through serialized jobs and stop at the configured limit; let permanent errors propagate.
- Verify server hints, fallback backoff, zero delays, and stopping without enqueueing when a hint exceeds the cap.
- Ensure readiness polls and interrupted resumptions preserve progress without reporting success early.
- Simulate a provider accepting a request before the connection fails, then verify that retries reuse the operation's idempotency key and do not duplicate the side effect.
