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
  spec.required_ruby_version = '>= 3.3.0'

  spec.metadata['allowed_push_host'] = 'https://rubygems.org'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/seuros/chrono_machines'
  spec.metadata['changelog_uri'] = 'https://github.com/seuros/chrono_machines/blob/main/CHANGELOG.md'
  spec.metadata['rubygems_mfa_required'] = 'true'

  spec.files = Dir.glob(%w[
    lib/**/*.rb
    sig/**/*.rbs
    README.md
    CHANGELOG.md
    LICENSE.txt
  ]).select { |f| File.exist?(f) }
  spec.require_paths = ['lib']

  spec.add_dependency 'zeitwerk', '~> 2.7'
end
