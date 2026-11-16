# frozen_string_literal: true

require 'minitest/autorun'
require 'chrono_machines'

# Records what the executor would sleep, in nanoseconds, and advances a test
# clock by it, so the suite never waits. Only inside `recording`.
module SleepRecorder
  private

  def sleep(span)
    slept = Thread.current[:chrono_slept]
    return super unless slept

    nanos = (span * 1_000_000_000).to_i
    slept << nanos
    clock = DeadlineMachines.clock
    clock.advance(nanos) if clock.respond_to?(:advance)
  end
end
ChronoMachines::Executor.prepend(SleepRecorder)

class ExecutorTest < Minitest::Test
  C = ChronoMachines
  S = 1_000_000_000
  GROW = C::Exponential.new(S, 2.0, 60 * S, 0.0)
  FIXED = C::Constant.new(5, 0.0)

  # A Random stand-in answering one fixed draw.
  FixedDraw = Struct.new(:draw) do
    def rand(_limit) = draw
  end

  def setup
    @slept = Thread.current[:chrono_slept] = []
  end

  def teardown
    Thread.current[:chrono_slept] = nil
    DeadlineMachines.clock = nil
  end

  def failing(times, error = IOError)
    calls = 0
    lambda do
      calls += 1
      raise error, "attempt #{calls}" if calls <= times

      calls
    end
  end

  def test_retries_until_the_block_succeeds
    events = []
    result = C.retry(GROW, 5, on_retry: ->(**event) { events << event },
                              on_success: ->(**event) { events << event }, &failing(2))
    assert_equal 3, result
    assert_equal [S, 2 * S], @slept
    assert_equal [[1, S], [2, 2 * S]], events.first(2).map { [_1[:attempt], _1[:next_delay]] }
    assert_equal({ result: 3, attempts: 3 }, events.last)
  end

  def test_exhaustion_raises_max_retries_exceeded_after_on_failure
    failures = []
    error = assert_raises(C::MaxRetriesExceededError) do
      C::Executor.new(FIXED, 3, on_failure: ->(**event) { failures << event }).call(&failing(9))
    end
    assert_equal [3, 'attempt 3'], [error.attempts, error.original_exception.message]
    assert_equal [[3, IOError]], failures.map { [_1[:attempts], _1[:exception].class] }
    assert_equal [5, 5], @slept
  end

  def test_an_exception_outside_retryable_is_raised_at_once
    failures = []
    executor = C::Executor.new(FIXED, 3, retryable: [IOError], on_failure: ->(**event) { failures << event })
    assert_raises(ArgumentError) { executor.call(&failing(1, ArgumentError)) }
    assert_equal [1], failures.map { _1[:attempts] }
    assert_empty @slept
    assert_equal 2, executor.call(&failing(1))
  end

  def test_delay_from_overrides_halts_or_defers
    hints = { 1 => 7, 2 => nil, 3 => :halt }
    executor = C::Executor.new(GROW, nil, delay_from: ->(exception:, attempt:) { hints.fetch(attempt) })
    assert_raises(IOError) { executor.call(&failing(9)) }
    # The hint is verbatim; deferring to the strategy gives the second delay.
    assert_equal [7, 2 * S], @slept
    beyond = C::Executor.new(GROW, nil, delay_from: ->(**) { 60 * S + 1 })
    assert_raises(IOError) { beyond.call(&failing(1)) }
    wrong = C::Executor.new(GROW, nil, delay_from: ->(**) { 1.5 })
    assert_raises(TypeError) { wrong.call(&failing(1)) }
  end

  def test_exhaustion_wins_over_a_hint
    asked = []
    executor = C::Executor.new(FIXED, 2, delay_from: ->(attempt:, **) { asked << attempt && :halt })
    assert_raises(C::MaxRetriesExceededError) { C::Executor.new(FIXED, 1).call(&failing(1)) }
    assert_raises(IOError) { executor.call(&failing(9)) }
    assert_equal [1], asked
  end

  def test_draws_come_from_random
    shape = C::Constant.new(1_000, 1.0)
    C::Executor.new(shape, 3, random: FixedDraw.new(1 << 63)).call(&failing(2))
    assert_equal [500, 500], @slept
  end

  def test_each_call_starts_a_fresh_streak
    executor = C::Executor.new(GROW, 3)
    2.times { executor.call(&failing(1)) }
    assert_equal [S, S], @slept
  end

  def test_runtime_keywords_are_type_checked
    assert_raises(TypeError) { C::Executor.new(GROW, 3, retryable: IOError) }
    assert_raises(TypeError) { C::Executor.new(GROW, 3, retryable: [String]) }
    assert_raises(TypeError) { C::Executor.new(GROW, 3, on_retry: :log) }
    assert_raises(TypeError) { C::Executor.new(GROW, 3, random: Object.new) }
    assert_raises(RangeError) { C::Executor.new(GROW, 0) }
    assert_raises(TypeError) { C::Executor.new({}, 3) }
    executor = C::Executor.new(GROW, 3)
    assert_predicate executor, :frozen?
    assert_equal [GROW, 3], [executor.strategy, executor.max_attempts]
  end

  def test_backoff_is_clamped_to_the_remaining_budget
    DeadlineMachines.clock = DeadlineMachines::TestClock.new
    assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(5 * S / 2) { C::Executor.new(GROW, 10).call(&failing(9)) }
    end
    assert_equal [S, S / 2 * 3], @slept
  end

  def test_an_inner_deadline_expiring_is_retried_like_any_other_failure
    DeadlineMachines.clock = DeadlineMachines::TestClock.new
    calls = 0
    result = DeadlineMachines.with_deadline(60 * S) do
      C::Executor.new(GROW, 10).call do
        calls += 1
        raise DeadlineMachines::Exceeded, DeadlineMachines.current if calls < 3

        :recovered
      end
    end
    assert_equal [:recovered, [S, 2 * S]], [result, @slept]
  end

  def test_a_spent_budget_is_raised_not_retried
    DeadlineMachines.clock = DeadlineMachines::TestClock.new
    calls = 0
    assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(S) do
        C::Executor.new(GROW, 10).call do
          calls += 1
          DeadlineMachines.clock.advance(2 * S)
          DeadlineMachines.check!
        end
      end
    end
    assert_equal [1, []], [calls, @slept]
  end
end
