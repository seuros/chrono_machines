#!/usr/bin/env ruby
# frozen_string_literal: true

require 'bundler/setup'
require 'benchmark'
require_relative 'lib/chrono_machines'

ITERATIONS = 1_000_000

puts "ChronoMachines: Native vs Pure Ruby Benchmark"
puts "=" * 60

executor = ChronoMachines::Executor.new(
  base_delay: 1.0,
  multiplier: 2.0,
  max_delay: 60.0,
  jitter_factor: 0.1,
  max_attempts: 5,
  retryable_exceptions: [StandardError]
)

# Check what's loaded
native_loaded = defined?(ChronoMachines::NativeExecutor) &&
                ChronoMachines::Executor.ancestors.include?(ChronoMachines::NativeExecutor)

puts "Native extension: #{native_loaded ? '✓ Loaded' : '✗ Not available'}"
puts "Ruby fallback:    ✓ Always available as ruby_calculate_delay"
puts "\nBenchmarking with #{ITERATIONS} iterations"
puts "-" * 60

Benchmark.bm(25) do |x|
  if native_loaded
    x.report("Native (calculate_delay):") do
      ITERATIONS.times { |i| executor.send(:calculate_delay, (i % 10) + 1) }
    end
  end

  x.report("Ruby (ruby_calculate_delay):") do
    ITERATIONS.times { |i| executor.send(:ruby_calculate_delay, (i % 10) + 1) }
  end
end

if native_loaded
  puts "\n" + "=" * 60
  puts "Native extension is active and can be compared with Ruby fallback"
  puts "=" * 60
end
