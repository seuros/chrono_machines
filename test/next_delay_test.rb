# frozen_string_literal: true

require_relative 'test_helper'

class NextDelayTest < Minitest::Test
  include ChronoMachines::TestHelper

  def test_returns_backoff_after_each_failed_attempt
    executor = ChronoMachines::Executor.new(base_delay: 1, max_delay: 3, max_attempts: 5, jitter_factor: 0)

    assert_equal [1, 2, 3, 3, nil], (1..5).map { |attempt| executor.next_delay(attempt: attempt) }
  end

  def test_uses_named_policies
    ChronoMachines.config.define_policy(:queued, backoff_strategy: :constant, base_delay: 2, jitter_factor: 0)

    assert_equal 2, ChronoMachines::Executor.new(:queued).next_delay(attempt: 1)
  end

  def test_stops_for_non_retryable_errors
    executor = ChronoMachines::Executor.new(retryable_exceptions: [IOError])

    assert_nil executor.next_delay(attempt: 1, exception: ArgumentError.new)
  end

  def test_uses_server_hint_without_jitter
    error = IOError.new
    executor = ChronoMachines::Executor.new(delay_from: lambda { |exception:, attempt:|
      assert_same error, exception
      assert_equal 2, attempt
      7
    })

    assert_equal 7, executor.next_delay(attempt: 2, exception: error)
  end

  def test_nil_hint_uses_backoff
    executor = ChronoMachines::Executor.new(base_delay: 2, jitter_factor: 0, delay_from: ->(**) { nil })

    assert_equal 2, executor.next_delay(attempt: 1, exception: IOError.new)
  end

  def test_zero_hint_allows_an_immediate_retry
    executor = ChronoMachines::Executor.new(delay_from: ->(**) { 0 })

    assert_equal 0, executor.next_delay(attempt: 1, exception: IOError.new)
  end

  def test_stops_when_the_hint_exceeds_the_cap
    executor = ChronoMachines::Executor.new(max_delay: 10, delay_from: ->(**) { 60 })

    assert_nil executor.next_delay(attempt: 1, exception: IOError.new)
  end

  def test_halt_hint_stops_retrying
    executor = ChronoMachines::Executor.new(delay_from: ->(**) { :halt })

    assert_nil executor.next_delay(attempt: 1, exception: IOError.new)
  end

  def test_does_not_call_hint_without_an_exception
    executor = ChronoMachines::Executor.new(base_delay: 1, jitter_factor: 0,
                                           delay_from: ->(**) { flunk 'No exception was supplied' })

    assert_equal 1, executor.next_delay(attempt: 1)
  end

  def test_does_not_sleep_or_run_lifecycle_callbacks
    callback = ->(**) { flunk 'Calculating a delay must not run lifecycle callbacks' }
    executor = ChronoMachines::Executor.new(on_retry: callback, on_failure: callback, on_success: callback)
    def executor.robust_sleep(_delay) = raise 'Must not sleep'

    assert_kind_of Numeric, executor.next_delay(attempt: 1, exception: IOError.new)
    assert_nil executor.next_delay(attempt: 3, exception: IOError.new)
  end

  def test_validates_the_failed_attempt_number
    executor = ChronoMachines::Executor.new

    assert_raises(ArgumentError) { executor.next_delay(attempt: 0) }
  end

  def test_seeded_jitter_matches_ruby_for_every_strategy
    %i[exponential constant fibonacci].each do |strategy|
      options = { backoff_strategy: strategy, max_attempts: 5, base_delay: 1, max_delay: 3, jitter_factor: 0.5 }
      executor = ChronoMachines::Executor.new(**options, random: Random.new(123))
      ruby_executor = ChronoMachines::Executor.new(**options, random: Random.new(123))

      (1..4).each do |attempt|
        expected = ruby_executor.send(:ruby_calculate_delay, attempt)
        assert_in_delta expected, executor.next_delay(attempt: attempt), 0.000001
      end
    end
  end

  def test_inline_retries_use_the_same_delays
    options = { base_delay: 1, jitter_factor: 0.5, max_attempts: 3 }
    policy = ChronoMachines::Executor.new(**options, random: Random.new(42))
    delays = []
    executor = ChronoMachines::Executor.new(**options, random: Random.new(42))
    executor.define_singleton_method(:robust_sleep) { |delay| delays << delay }

    assert_raises(ChronoMachines::MaxRetriesExceededError) { executor.call { raise IOError } }
    assert_equal [policy.next_delay(attempt: 1), policy.next_delay(attempt: 2)], delays
  end
end
