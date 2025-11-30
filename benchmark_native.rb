#!/usr/bin/env ruby
# frozen_string_literal: true

require 'bundler/setup'
require 'benchmark'
require_relative 'lib/chrono_machines'

ITERATIONS = 100_000

puts "ChronoMachines Native vs Pure Ruby Benchmark"
puts "=" * 60

# Check if native extension is loaded
native_loaded = ChronoMachines::Executor.ancestors.include?(ChronoMachines::NativeExecutor)

if native_loaded
  puts "✓ Native extension loaded (using Module.prepend)"
else
  puts "✗ Native extension not available - running pure Ruby only"
end

puts "\nBenchmarking calculate_delay with #{ITERATIONS} iterations"
puts "-" * 60

# Create executor instances
executor = ChronoMachines::Executor.new(
  base_delay: 1.0,
  multiplier: 2.0,
  max_delay: 60.0,
  jitter_factor: 0.1,
  max_attempts: 5,
  retryable_exceptions: [StandardError]
)

# Benchmark the calculation
Benchmark.bm(20) do |x|
  x.report("calculate_delay:") do
    ITERATIONS.times do |i|
      # Test with different attempt numbers to cover the exponential backoff curve
      executor.send(:calculate_delay, (i % 10) + 1)
    end
  end
end

puts "\nImplementation: #{native_loaded ? 'Native (Rust)' : 'Pure Ruby'}"

# Additional detailed test with different attempt values
puts "\n" + "=" * 60
puts "Sample delay calculations (10 samples per attempt level):"
puts "-" * 60

[1, 2, 3, 5, 10].each do |attempts|
  delays = 10.times.map { executor.send(:calculate_delay, attempts) }
  avg = delays.sum / delays.size
  min = delays.min
  max = delays.max

  puts "Attempt %2d: avg=%.4fs, min=%.4fs, max=%.4fs" % [attempts, avg, min, max]
end
