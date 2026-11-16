# frozen_string_literal: true

require 'minitest/autorun'
require 'chrono_machines'

# Retry mirrors the core's failure streak: a refusal is a symbol and changes
# nothing, malformed input raises before the core is asked.
class RetryTest < Minitest::Test
  C = ChronoMachines
  MS = 1_000_000
  GROW = C::Exponential.new(100 * MS, 2.0, 1_000 * MS, 0.0)

  def state(retry_state)
    [retry_state.failures, retry_state.waited]
  end

  def test_the_budget_counts_the_first_try
    streak = C::Retry.new(GROW, 3)
    assert_equal [100 * MS, 200 * MS], [streak.failed(0), streak.failed(0)]
    assert_equal [2, 300 * MS], state(streak)
    assert_equal :exhausted, streak.failed(0)
    assert_equal :exhausted, streak.hinted(1)
    assert_equal :exhausted, streak.hinted(10_000 * MS)
    assert_equal [2, 300 * MS], state(streak)
    assert_equal :exhausted, C::Retry.new(GROW, 1).failed(0)
  end

  def test_a_hint_is_taken_verbatim_up_to_the_cap
    streak = C::Retry.new(GROW, nil)
    assert_equal 7, streak.hinted(7)
    assert_equal :beyond_cap, streak.hinted(1_000 * MS + 1)
    assert_equal [1, 7], state(streak)
    assert_equal 200 * MS, streak.failed(0)
    uncapped = C::Retry.new(C::Constant.new(1, 0.0), nil)
    assert_equal 2**64 - 1, uncapped.hinted(2**64 - 1)
  end

  def test_the_hint_ceiling_is_its_own_knob
    streak = C::Retry.new(GROW, nil)
    assert_equal 1_000 * MS, streak.hint_ceiling
    assert_same streak, streak.with_hint_ceiling(30_000 * MS)
    assert_equal 30_000 * MS, streak.hint_ceiling
    assert_equal 30_000 * MS, streak.hinted(30_000 * MS)
    assert_equal :beyond_cap, streak.hinted(30_000 * MS + 1)
    assert_equal [1, 30_000 * MS], state(streak)
    assert_equal 200 * MS, streak.failed(0)
    constant = C::Retry.new(C::Constant.new(1, 0.0), nil)
    assert_nil constant.hint_ceiling
    assert_equal :beyond_cap, constant.with_hint_ceiling(1).hinted(2)
    assert_raises(RangeError) { streak.with_hint_ceiling(-1) }
    assert_raises(TypeError) { streak.with_hint_ceiling(nil) }
    assert_equal 30_000 * MS, streak.hint_ceiling
  end

  def test_success_ends_the_streak_and_unbounded_never_refuses
    streak = C::Retry.new(C::Fibonacci.new(10, 1_000, 0.0), 3)
    streak.failed(0)
    assert_nil streak.succeeded
    assert_equal [0, 0], state(streak)
    forever = C::Retry.new(C::Constant.new(2**64 - 1, 0.0), nil)
    300.times { assert_equal 2**64 - 1, forever.failed(0) }
    assert_equal 255, forever.failures
    # The sum passes 2**64 and keeps counting to the core's Duration::MAX.
    assert_operator forever.waited, :>, 2**64
  end

  def test_readers
    streak = C::Retry.new(GROW, 5)
    assert_equal [GROW, 5], [streak.strategy, streak.max_attempts]
    assert_instance_of C::Exponential, streak.strategy
    assert_predicate streak.strategy, :frozen?
    assert_nil C::Retry.new(GROW, nil).max_attempts
  end

  def test_construction_checks_both_arguments
    assert_equal 'max_attempts must be in 1..=255', assert_raises(RangeError) { C::Retry.new(GROW, 0) }.message
    assert_raises(RangeError) { C::Retry.new(GROW, 256) }
    assert_raises(TypeError) { C::Retry.new(GROW, 3.0) }
    message = 'strategy must be a ChronoMachines::Exponential, ChronoMachines::Constant or ' \
              'ChronoMachines::Fibonacci'
    assert_equal message, assert_raises(TypeError) { C::Retry.new(GROW.to_h, 3) }.message
    assert_raises(TypeError) { C::Retry.new(Struct.new(:span, :jitter).new(1, 0.0), 3) }
    assert_raises(TypeError) { C::Retry.allocate }
  end

  def test_a_bad_argument_changes_nothing
    streak = C::Retry.new(GROW, nil)
    streak.failed(0)
    before = state(streak)
    assert_raises(RangeError) { streak.failed(-1) }
    assert_raises(RangeError) { streak.failed(2**64) }
    assert_raises(TypeError) { streak.failed(0.5) }
    assert_raises(RangeError) { streak.hinted(-1) }
    assert_raises(TypeError) { streak.hinted(1.0) }
    assert_equal before, state(streak)
  end

  # Frozen is checked first, before an argument is even converted.
  def test_a_frozen_retry_raises_frozen_error_first
    streak = C::Retry.new(GROW, nil)
    streak.failed(0)
    streak.freeze
    assert_raises(FrozenError) { streak.failed(0) }
    assert_raises(FrozenError) { streak.failed('not a draw') }
    assert_raises(FrozenError) { streak.hinted(nil) }
    assert_raises(FrozenError) { streak.succeeded }
    assert_raises(FrozenError) { streak.with_hint_ceiling(1) }
    assert_equal [1, 100 * MS], state(streak)
  end

  # The strategy answer is built from classes pinned at load; GC.compact must
  # not move them out from under the extension.
  def test_answers_survive_gc_compact
    streak = C::Retry.new(C::Fibonacci.new(1, 2, 0.5), 2)
    GC.compact
    assert_equal C::Fibonacci.new(1, 2, 0.5), streak.strategy
    assert_equal 1, C::Fibonacci.new(1, 2, 0.0).delay(1, 0)
  end
end
