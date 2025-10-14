# frozen_string_literal: true

require_relative 'lib/chrono_machines/version'

Gem::Specification.new do |spec|
  spec.name = 'chrono_machines'
  spec.version = ChronoMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['terminale@gmail.com']

  spec.summary = 'A robust Ruby gem for implementing retry mechanisms with exponential backoff and jitter.'
  spec.description = 'ChronoMachines offers a flexible and configurable solution for handling transient failures ' \
                     'in distributed Ruby applications. It provides powerful retry strategies, including exponential ' \
                     'backoff and full jitter, with customizable callbacks. Features optional Rust-powered native ' \
                     'performance on CRuby with pure Ruby fallback for JRuby compatibility.'
  spec.homepage = 'https://github.com/seuros/chrono_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 3.3.0'

  spec.metadata['allowed_push_host'] = 'https://rubygems.org'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/seuros/chrono_machines'
  spec.metadata['changelog_uri'] = 'https://github.com/seuros/chrono_machines/blob/main/CHANGELOG.md'
  spec.metadata['github_repo'] = 'ssh://github.com/seuros/chrono_machines'
  spec.metadata['rubygems_mfa_required'] = 'true'

  spec.files = Dir.glob(%w[
                          lib/**/*.rb
                          sig/**/*.rbs
                          ext/**/*.{rb,rs,toml}
                          README.md
                          CHANGELOG.md
                          LICENSE.txt
                        ]).select { |f| File.exist?(f) }
  spec.require_paths = ['lib']

  # Add native extension (only on CRuby/TruffleRuby)
  spec.extensions = ['ext/chrono_machines_native/extconf.rb'] unless RUBY_ENGINE == 'jruby'

  spec.add_dependency 'zeitwerk', '~> 2.7'
end
