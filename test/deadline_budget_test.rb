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

  # Advancing the clock by whatever was slept keeps the budget honest without
  # the suite actually waiting.
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

  def test_backoff_is_clamped_to_the_remaining_budget
    assert_raises(DeadlineMachines::Exceeded) do
      DeadlineMachines.with_deadline(2.5) do
        executor.call { raise IOError, 'down' }
      end
    end

    assert_equal [1.0, 1.5], @slept.map { |d| d.round(6) }
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
    assert_equal [1.0, 2.0], @slept.map { |d| d.round(6) }
  end

  def test_without_a_deadline_the_backoff_is_untouched
    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      executor(max_attempts: 4).call { raise IOError, 'down' }
    end

    assert_equal [1.0, 2.0, 4.0], @slept.map { |d| d.round(6) }
  end
end
