#!/usr/bin/env ruby
# frozen_string_literal: true

require 'bundler/setup'
require 'benchmark'
require_relative 'lib/chrono_machines'

ITERATIONS = 1_000_000

puts "ChronoMachines: Backoff Strategy Performance Comparison"
puts "=" * 70

# Check if native extension is loaded
native_loaded = defined?(ChronoMachines::NativeExecutor) &&
                ChronoMachines::Executor.ancestors.include?(ChronoMachines::NativeExecutor)

puts "Native extension: #{native_loaded ? '✓ Loaded' : '✗ Not available'}"
puts "\nBenchmarking #{ITERATIONS} iterations per strategy"
puts "=" * 70

# Test all three backoff strategies
strategies = [:exponential, :constant, :fibonacci]
results = {}

strategies.each do |strategy|
  puts "\n### #{strategy.to_s.capitalize} Backoff ###"
  puts "-" * 70

  executor = ChronoMachines::Executor.new(
    backoff_strategy: strategy,
    base_delay: 1.0,
    multiplier: 2.0,
    max_delay: 60.0,
    jitter_factor: 0.1,
    max_attempts: 5,
    retryable_exceptions: [StandardError]
  )

  results[strategy] = {}

  Benchmark.bm(30) do |x|
    if native_loaded
      results[strategy][:native] = x.report("Native (#{strategy}):") do
        ITERATIONS.times { |i| executor.send(:calculate_delay, (i % 10) + 1) }
      end
    end

    results[strategy][:ruby] = x.report("Ruby (#{strategy}):") do
      ITERATIONS.times { |i| executor.send(:ruby_calculate_delay, (i % 10) + 1) }
    end
  end
end

# Summary
puts "\n" + "=" * 70
puts "SUMMARY"
puts "=" * 70

strategies.each do |strategy|
  next unless results[strategy][:native] && results[strategy][:ruby]

  native_time = results[strategy][:native].real
  ruby_time = results[strategy][:ruby].real
  speedup = ruby_time / native_time

  puts "\n#{strategy.to_s.capitalize} Backoff:"
  puts "  Native: %.6fs" % native_time
  puts "  Ruby:   %.6fs" % ruby_time
  puts "  Speedup: %.2fx faster" % speedup
end

puts "\n" + "=" * 70