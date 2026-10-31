# frozen_string_literal: true

require 'deadline_machines'
require 'zeitwerk'

loader = Zeitwerk::Loader.for_gem
loader.ignore("#{__dir__}/chrono_machines/errors.rb")
loader.ignore("#{__dir__}/chrono_machines/async_support.rb")
loader.ignore("#{__dir__}/chrono_machines/test_helper.rb")
loader.ignore("#{__dir__}/chrono_machines/native_speedup.rb")
loader.inflector.inflect('dsl' => 'DSL')
loader.setup

require_relative 'chrono_machines/errors'

module ChronoMachines
  # Global configuration instance
  @config = Configuration.new

  def self.configure
    yield @config
  end

  def self.config
    @config
  end

  def self.retry(policy_name_or_options = {}, &)
    Executor.new(policy_name_or_options).call(&)
  end

  # Full jitter over a single duration: the result lands in
  # `seconds * (1 - factor)` to `seconds`, so a fleet of callers waiting the
  # same nominal time does not wake up together.
  def self.jitter(seconds, factor:, random: Random)
    factor = Float(factor)
    raise ArgumentError, 'factor cannot be NaN' if factor.nan?

    factor = factor.clamp(0.0, 1.0)
    seconds * (1 - factor + (random.rand * factor))
  end
end

# Load optional async support after core is loaded
begin
  require_relative 'chrono_machines/async_support'
rescue LoadError
  # Async gem not available, skip async support
end
