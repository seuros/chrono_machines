# frozen_string_literal: true

require 'minitest/autorun'
require 'chrono_machines'

# The shapes are valid or do not exist, and their arithmetic is the core's.
class ShapesTest < Minitest::Test
  C = ChronoMachines
  MS = 1_000_000
  MAX = 2**64 - 1
  HALF = 1 << 63

  def test_fields_are_nanoseconds_and_floats
    assert_equal [MS, 2.0, 10 * MS, 1.0], C::Exponential.new(MS, 2, 10 * MS, 1).to_h.values
    assert_equal [0, 0.5], C::Constant.new(span: 0, jitter: 0.5).to_h.values
    assert_equal [MAX, MAX, 0.0], C::Fibonacci.new(MAX, MAX, 0).to_h.values
    assert_instance_of Float, C::Constant.new(1, 1).jitter
    assert_equal C::Constant.new(1, 1), C::Constant.new(1, 1.0)
  end

  def test_a_shape_no_delay_comes_from_raises_range_error
    {
      -> { C::Exponential.new(2, 2.0, 1, 0.0) } => 'an Exponential needs base <= max, a finite multiplier ' \
                                                   '>= 1.0 and a jitter in 0.0..=1.0',
      -> { C::Exponential.new(1, 0.5, 2, 0.0) } => nil,
      -> { C::Exponential.new(1, Float::INFINITY, 2, 0.0) } => 'multiplier must be finite',
      -> { C::Constant.new(1, 1.5) } => 'a Constant needs a jitter in 0.0..=1.0',
      -> { C::Constant.new(1, Float::NAN) } => 'jitter must be finite',
      -> { C::Fibonacci.new(2, 1, 0.0) } => 'a Fibonacci needs base <= max and a jitter in 0.0..=1.0',
      -> { C::Fibonacci.new(-1, 1, 0.0) } => 'base must be in 0..=18446744073709551615',
      -> { C::Fibonacci.new(1, 2**64, 0.0) } => 'max must be in 0..=18446744073709551615'
    }.each do |build, message|
      error = assert_raises(RangeError, &build)
      assert_equal message, error.message if message
    end
  end

  def test_a_wrong_type_raises_type_error
    assert_equal 'span must be an Integer', assert_raises(TypeError) { C::Constant.new(1.0, 0.0) }.message
    assert_equal 'jitter must be a Float or Integer', assert_raises(TypeError) { C::Constant.new(1, 1r) }.message
    assert_raises(TypeError) { C::Exponential.new(1, '2', 2, 0.0) }
    assert_raises(TypeError) { C::Fibonacci.new(1, nil, 0.0) }
    assert_raises(ArgumentError) { C::Constant.new(1) }
  end

  def test_unjittered_sequences_reach_the_cap
    grow = C::Exponential.new(100 * MS, 2.0, 1_000 * MS, 0.0)
    assert_equal [100, 200, 400, 800, 1_000, 1_000].map { _1 * MS }, (1..6).map { grow.delay(_1, MAX) }
    fib = C::Fibonacci.new(100 * MS, 1_000 * MS, 0.0)
    assert_equal [100, 100, 200, 300, 500, 800, 1_000].map { _1 * MS }, (1..7).map { fib.delay(_1, 0) }
    assert_equal 250, C::Constant.new(250, 0.0).delay(255, MAX)
    assert_equal [1_000 * MS, nil, 1_000 * MS], [grow.cap, C::Constant.new(1, 0.0).cap, fib.cap]
  end

  def test_a_draw_takes_its_share_of_the_jitter_off
    shape = C::Constant.new(1_000, 1.0)
    assert_equal [1_000, 500, 0], [0, HALF, MAX].map { shape.delay(1, _1) }
    assert_equal 750, C::Fibonacci.new(1_000, 1_000, 0.5).delay(1, HALF)
    random = Random.new(7)
    1_000.times do
      delay = C::Exponential.new(MS, 2.0, 8 * MS, 0.25).delay(4, random.rand(2**64))
      assert_operator delay, :<=, 8 * MS
      assert_operator delay, :>=, 6 * MS
    end
  end

  def test_delay_arguments_are_checked
    shape = C::Constant.new(1, 0.0)
    assert_equal 'attempt must be in 1..=255', assert_raises(RangeError) { shape.delay(0, 0) }.message
    assert_raises(RangeError) { shape.delay(256, 0) }
    assert_equal 'draw must be in 0..=18446744073709551615', assert_raises(RangeError) { shape.delay(1, -1) }.message
    assert_raises(RangeError) { shape.delay(1, 2**64) }
    assert_raises(TypeError) { shape.delay(1.0, 0) }
    assert_raises(TypeError) { shape.delay(1, 0.5) }
  end

  def test_the_shapes_are_sealed_and_the_hooks_private
    [C::Exponential, C::Constant, C::Fibonacci].each do |shape|
      assert_raises(TypeError) { Class.new(shape) }
      assert_raises(NoMethodError) { shape.allocate }
    end
    %i[__exponential __constant __fibonacci].each do |hook|
      assert_raises(NoMethodError) { C.public_send(hook, 1, 1) }
    end
  end

  # Reflection still builds a value that skipped the check (as Marshal.load
  # does); the native side reads the struct by exact class and validates it
  # again, so a forged value never reaches the core.
  def forge(shape, **fields)
    Class.instance_method(:allocate).bind_call(shape).tap do |value|
      Data.instance_method(:initialize).bind_call(value, **fields)
    end
  end

  def test_a_forged_value_is_refused_natively
    inverted = forge(C::Fibonacci, base: 9, max: 1, jitter: 0.0)
    assert_raises(RangeError) { inverted.delay(1, 0) }
    assert_raises(RangeError) { inverted.cap }
    assert_raises(RangeError) { C::Retry.new(inverted, 3) }
    floated = forge(C::Constant, span: 1.5, jitter: 0.0)
    assert_raises(TypeError) { floated.delay(1, 0) }
    assert_raises(TypeError) { C::Retry.new(floated, nil) }
    assert_raises(RangeError) { C::Fibonacci.new(5, 9, 0.0).with(max: 1) }
  end

  def test_a_shape_with_a_redefined_reader_is_read_from_the_struct
    shape = C::Exponential.new(MS, 2.0, 4 * MS, 0.0)
    C::Exponential.alias_method(:original_max, :max)
    C::Exponential.define_method(:max) { 0 }
    assert_equal 4 * MS, shape.delay(5, 0)
    assert_equal 4 * MS, shape.cap
  ensure
    verbose, $VERBOSE = $VERBOSE, nil
    C::Exponential.alias_method(:max, :original_max)
    C::Exponential.remove_method(:original_max)
    $VERBOSE = verbose
  end
end
