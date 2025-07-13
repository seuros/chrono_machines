# frozen_string_literal: true

require_relative 'lib/chrono_machines/version'

Gem::Specification.new do |spec|
  spec.name = 'chrono_machines'
  spec.version = ChronoMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['terminale@gmail.com']

  spec.summary = 'A robust Ruby gem for implementing retry mechanisms with exponential backoff and jitter.'
  spec.description = 'ChronoMachines offers a flexible and configurable solution for handling transient failures in distributed Ruby applications. It provides powerful retry strategies, including exponential backoff and full jitter, along with customizable callbacks for success, retry, and failure events. Define and manage retry policies with a clean DSL for seamless integration.'
  spec.homepage = 'https://github.com/seuros/chrono_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 3.2.0'

  spec.metadata['allowed_push_host'] = 'https://rubygems.org'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/seuros/chrono_machines'
  spec.metadata['changelog_uri'] = 'https://github.com/seuros/chrono_machines/blob/main/CHANGELOG.md'
  spec.metadata['rubygems_mfa_required'] = 'true'

  # Specify which files should be added to the gem when it is released.
  # This is a more robust way to include files than Dir.glob
  spec.files = Dir.chdir(__dir__) do
    `git ls-files -z`.split("\x0").reject do |f|
      (f == __FILE__) || f.match(%r{\A(?:(?:test|spec|features)/|\.(?:git|travis|circleci)|appveyor)})
    end
  end
  spec.bindir = 'exe'
  spec.executables = spec.files.grep(%r{\Aexe/}) { |f| File.basename(f) }
  spec.require_paths = ['lib']

  # Uncomment to register a new dependency of your gem
  # spec.add_dependency "example-gem", "~> 1.0"

  # For more information and examples about making a new gem, check out our
  # guide at: https://bundler.io/guides/creating_gem.html
end
