# frozen_string_literal: true

require 'test_helper'

class JitterTest < Minitest::Test
  Fixed = Struct.new(:value) do
    def rand = value
  end

  def test_the_result_spans_the_documented_range
    assert_in_delta 7.0, ChronoMachines.jitter(10.0, factor: 0.3, random: Fixed.new(0.0))
    assert_in_delta 10.0, ChronoMachines.jitter(10.0, factor: 0.3, random: Fixed.new(1.0))
  end

  def test_a_zero_factor_leaves_the_duration_alone
    assert_in_delta 10.0, ChronoMachines.jitter(10.0, factor: 0.0, random: Fixed.new(0.0))
  end

  def test_a_factor_outside_the_unit_range_is_clamped
    assert_in_delta 0.0, ChronoMachines.jitter(10.0, factor: 9.0, random: Fixed.new(0.0))
    assert_in_delta 10.0, ChronoMachines.jitter(10.0, factor: -5.0, random: Fixed.new(0.0))
  end

  def test_a_nonsense_factor_raises
    assert_raises(ArgumentError) { ChronoMachines.jitter(1.0, factor: Float::NAN) }
    assert_raises(ArgumentError) { ChronoMachines.jitter(1.0, factor: 'half') }
  end
end
