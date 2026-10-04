# frozen_string_literal: true

require_relative 'test_helper'

class NativeRactorTest < Minitest::Test
  def setup
    ChronoMachines::Executor # autoloading it pulls in the optional native speedup
    skip 'Native extension not available' unless defined?(ChronoMachinesNative)
  end

  def test_delays_compute_inside_a_ractor
    experimental = Warning[:experimental]
    Warning[:experimental] = false

    delays = Array.new(4) do
      Ractor.new { ChronoMachinesNative.exponential_delay(3, 0.1, 2.0, 10.0, 0.0) }
    end.map(&:value)

    delays.each { |delay| assert_in_delta 0.4, delay }
  ensure
    Warning[:experimental] = experimental
  end
end
