# frozen_string_literal: true

require 'minitest/autorun'

module ChronoMachines
  module TestHelper
    def setup
      super
      # Reset configuration before each test
      ChronoMachines.instance_variable_set(:@config, ChronoMachines::Configuration.new)
    end

    module Assertions
      def assert_cm_delay_range(delay, expected_min, expected_max, message = nil)
        assert_operator(delay, :>=, expected_min, "Expected delay #{delay} to be >= #{expected_min}. #{message}")
        assert_operator(delay, :<=, expected_max, "Expected delay #{delay} to be <= #{expected_max}. #{message}")
      end
    end

    include Assertions
  end
end
