# frozen_string_literal: true

module ChronoMachines
  module DSL
    def self.included(base)
      base.extend(ClassMethods)
    end

    module ClassMethods
      def chrono_policy(name, options = {})
        ChronoMachines.configure do |config|
          config.define_policy(name, options)
        end
      end
    end

    def with_chrono_policy(policy_name_or_options, &)
      ChronoMachines.retry(policy_name_or_options, &)
    end
  end
end
