# frozen_string_literal: true

require 'test_helper'

class DeadlineBudgetTest < Minitest::Test
  def setup
    @clock = DeadlineMachines::TestClock.new
    DeadlineMachines.clock = @clock
    @slept = []
  end

  def teardown
    DeadlineMachines.clock = nil
  end

  # Advancing the test clock by whatever was slept keeps the budget honest
  # without the suite actually waiting.
  def executor(**options)
    defaults = { base_delay: 1.0, multiplier: 2.0, max_delay: 60, jitter_factor: 0.0, max_attempts: 10 }
    executor = ChronoMachines::Executor.new(defaults.merge(options))
    clock = @clock
    slept = @slept
    executor.define_singleton_method(:robust_sleep) do |delay|
      slept << delay
      clock.advance(delay)
    end
    executor
  end

  def test_backoff_never_sleeps_past_the_deadline
    assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(2.5) do
        executor.call { raise IOError, 'down' }
      end
    end

    assert_in_delta 2.5, @slept.sum
    assert_equal [1.0, 1.5], @slept.map { |d| d.round(6) }
  end

  def test_an_exhausted_budget_stops_the_retry_loop
    error = assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(0.5) do
        executor.call { raise IOError, 'down' }
      end
    end

    assert_equal [0.5], @slept.map { |d| d.round(6) }
    assert_instance_of IOError, error.cause
  end

  def test_cancelling_the_caller_stops_the_retry_loop
    assert_raises(DeadlineMachines::Cancelled) do
      DeadlineMachines.with_deadline(60) do |deadline|
        attempts = 0
        executor.call do
          attempts += 1
          deadline.cancel('client hung up') if attempts == 2
          raise IOError, 'down'
        end
      end
    end

    assert_equal [1.0], @slept.map { |d| d.round(6) }
  end

  def test_without_a_deadline_the_backoff_is_untouched
    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      executor(max_attempts: 4).call { raise IOError, 'down' }
    end

    assert_equal [1.0, 2.0, 4.0], @slept.map { |d| d.round(6) }
  end

  def test_a_deadline_wider_than_the_retries_does_not_interfere
    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      DeadlineMachines.with_deadline(3600) do
        executor(max_attempts: 4).call { raise IOError, 'down' }
      end
    end

    assert_equal [1.0, 2.0, 4.0], @slept.map { |d| d.round(6) }
  end

  def test_an_already_spent_deadline_runs_no_attempts_at_all
    attempts = 0

    assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(1) do
        @clock.advance(2)
        executor.call { attempts += 1 }
      end
    end

    assert_equal 0, attempts
    assert_empty @slept
  end

  def test_an_inner_deadline_expiring_is_retried_like_any_other_failure
    attempts = 0

    result = DeadlineMachines.with_deadline(60) do
      executor.call do
        attempts += 1
        raise DeadlineMachines::Exceeded, DeadlineMachines.current if attempts < 3

        :recovered
      end
    end

    assert_equal :recovered, result
    assert_equal 3, attempts
    assert_equal [1.0, 2.0], @slept.map { |d| d.round(6) }
  end

  def test_on_retry_reports_the_clamped_delay_not_the_planned_one
    reported = []

    assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(2.5) do
        executor(on_retry: ->(**kwargs) { reported << kwargs[:next_delay] }).call { raise IOError, 'down' }
      end
    end

    assert_equal [1.0, 1.5], reported.map { |d| d.round(6) }
  end

  def test_a_spent_budget_still_fires_the_terminal_failure_callback
    failures = []

    assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(0.5) do
        executor(on_failure: ->(**kwargs) { failures << kwargs[:attempts] }).call { raise IOError, 'down' }
      end
    end

    assert_equal [1], failures
  end
end
