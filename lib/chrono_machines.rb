# frozen_string_literal: true

require 'zeitwerk'

loader = Zeitwerk::Loader.for_gem
loader.ignore("#{__dir__}/chrono_machines/errors.rb")
loader.ignore("#{__dir__}/chrono_machines/async_support.rb")
loader.ignore("#{__dir__}/chrono_machines/test_helper.rb")
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
end

# Load optional async support after core is loaded
begin
  require_relative 'chrono_machines/async_support'
rescue LoadError
  # Async gem not available, skip async support
end
