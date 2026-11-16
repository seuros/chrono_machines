# frozen_string_literal: true

require_relative 'lib/chrono_machines/version'

Gem::Specification.new do |spec|
  spec.name = 'chrono_machines'
  spec.version = ChronoMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['terminale@gmail.com']

  spec.summary = 'Exponential, constant and Fibonacci backoff with jitter, over a native Rust core.'
  spec.description = 'Backoff shapes and a failure-streak counter in Integer nanoseconds, validated by a ' \
                     'no_std Rust core, with a blocking retry runtime that honours deadline_machines budgets.'
  spec.homepage = 'https://github.com/seuros/chrono_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 4.0.0'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/seuros/chrono_machines'
  spec.metadata['changelog_uri'] = 'https://github.com/seuros/chrono_machines/blob/master/CHANGELOG.md'
  spec.metadata['github_repo'] = 'ssh://github.com/seuros/chrono_machines'
  spec.metadata['rubygems_mfa_required'] = 'true'
  spec.metadata['cargo_crate_name'] = 'chrono_machines_native'
  spec.metadata['cargo_manifest_path'] = 'ext/chrono_machines_native/ffi/Cargo.toml'

  spec.files = Dir.glob(%w[
    lib/**/*.rb sig/**/*.rbs ext/**/*.{rs,rb,toml,md} Cargo.toml
    Cargo.lock README.md CHANGELOG.md LICENSE.txt
  ]).select { |p| File.file?(p) }
    .reject { |p| p.end_with?('/tests.rs') || p.include?('/tests/') }
  spec.require_paths = ['lib']
  spec.extensions = ['ext/chrono_machines_native/extconf.rb']

  spec.add_dependency 'deadline_machines', '>= 0.1'
  spec.add_dependency 'rb_sys', '~> 0.9', '>= 0.9.130'
end
