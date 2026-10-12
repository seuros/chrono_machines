# Rails Integration

ChronoMachines calculates retry delays. Active Job and your queue backend schedule jobs; your application decides which operations can be repeated and when they are complete. These examples use the public `Executor#next_delay` API and require no adapter.

Hilbert's Hotel is fully occupied and still taking reservations. The concierge is moving guest `n` to room `2n`; the vacancy board is understandably out of date.

The examples assume an application service, `VacancyBoardRefresh.call(hotel_id)`, that reads occupancy and updates a local vacancy board. It can safely refresh the same board more than once. Its client raises `HilbertHotel::ConciergeUnavailable` for retryable failures. That error exposes `retry_after` as a number of seconds or `nil`, with any HTTP header parsing handled by the client.

## Using `retry_on`

Use Rails' built-in retry handling when the delay only needs the failed attempt number:

```ruby
class VacancyBoardRefreshJob < ApplicationJob
  ATTEMPTS = 5
  RETRY_POLICY = ChronoMachines::Executor.new(
    max_attempts: ATTEMPTS,
    base_delay: 1,
    max_delay: 60,
    jitter_factor: 0.5
  )

  retry_on HilbertHotel::ConciergeUnavailable,
           attempts: ATTEMPTS,
           wait: ->(attempt) { RETRY_POLICY.next_delay(attempt: attempt) }

  def perform(hotel_id)
    VacancyBoardRefresh.call(hotel_id)
  end
end
```

The shared `ATTEMPTS` constant keeps Rails' limit and the policy's limit equal. Both include the initial execution: five attempts allow four retries. Rails only calls `wait` before the limit is reached. If the limits drift, `next_delay` can return `nil`, which Rails treats as no wait rather than a request to stop.

Rails selects the exceptions to retry and serializes its retry counters. Chrono supplies the backoff and jitter; Rails uses the wait proc's result directly. This preserves normal `retry_on`, `discard_on`, and `rescue_from` behavior, including Rails' handling of wrapped exception causes.

The wait proc receives only the attempt number. It cannot pass the exception to `delay_from`. Use explicit scheduling when the delay depends on the error.

## Scheduling durable retries with server hints

Use a persistent queue backend, and pass the failed attempt number and exception to `next_delay`. Carry the next attempt in the new job's arguments:

```ruby
class VacancyBoardRefreshWithHintsJob < ApplicationJob
  RETRY_POLICY = ChronoMachines::Executor.new(
    max_attempts: 5,
    base_delay: 1,
    max_delay: 60,
    jitter_factor: 0.5,
    retryable_exceptions: [HilbertHotel::ConciergeUnavailable],
    delay_from: ->(exception:, **) { exception.retry_after }
  )

  def perform(hotel_id, attempt: 1)
    VacancyBoardRefresh.call(hotel_id)
  rescue HilbertHotel::ConciergeUnavailable => error
    delay = RETRY_POLICY.next_delay(attempt: attempt, exception: error)
    raise unless delay

    self.class.set(wait_until: Time.current + delay)
        .perform_later(hotel_id, attempt: attempt + 1)
  end
end
```

The queue stores the scheduled time and arguments, so the count survives process restarts. The first failure uses `attempt: 1`. At the fifth failure, `next_delay` returns `nil` and the original exception is raised without scheduling another job.

`delay_from` returns `nil` to use backoff, a numeric delay to honor the server's hint without jitter, or `:halt`/`false` to stop. A hint above `max_delay` also stops; it is never shortened to the cap. A zero delay is valid, so check for `nil`, not a positive number.

This example rescues the client's error directly. If your client wraps errors, classify them explicitly or use Rails' exception handlers; a Ruby `rescue` does not search exception causes.

For workflows with recovery sweeps, persist the failure count and next check time on the execution record. A sweep can enqueue due work after a worker interruption. Keep readiness polls separate from failed attempts, and preserve the same operation identifier across retries.

## Policy callbacks

`next_delay` does not run `on_retry`, `on_success`, or `on_failure`. `ChronoMachines.retry` and `Executor#call` run those callbacks around their own inline retry loop, which waits within the current execution.

For scheduled jobs, invoke any configured callbacks at explicit application events:

| Callback | When to invoke it |
| --- | --- |
| `on_retry(exception:, attempt:, next_delay:)` | After the queue has accepted the retry. |
| `on_success(result:, attempts:)` | When the operation has actually completed. |
| `on_failure(exception:, attempts:)` | When the application decides the operation has permanently failed. |

A job that schedules another readiness poll has not finished its operation. Returning from `perform` or running `after_perform` is not enough to report success. Likewise, an interruption that will resume is not a permanent failure. Callback failures should not hide the original error or cause a completed side effect to run again.

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
