# frozen_string_literal: true

require_relative 'chrono_machines/version'
require_relative 'chrono_machines/errors'
require_relative 'chrono_machines/executor'
require_relative 'chrono_machines/configuration'
require_relative 'chrono_machines/dsl'

# Optional Async support - load after executor is defined
require_relative 'chrono_machines/async_support'

module ChronoMachines
  def self.retry(policy_name_or_options = {}, &)
    Executor.new(policy_name_or_options).call(&)
  end
end
