# frozen_string_literal: true

source 'https://rubygems.org'

gemspec

# deadline_machines is not released yet; use the sibling checkout when there is
# one, and the published gem once there is one of those.
sibling = File.expand_path('../deadline_machines', __dir__)
gem 'deadline_machines', path: sibling if File.directory?(sibling)

gem 'minitest', '~> 6.0'
gem 'rake', '~> 13.0'
gem 'zeitwerk', '~> 2.7'

# Platform specific gems (MRI Ruby only)
platforms :mri do
  gem 'async', '~> 2.34'
  gem 'padded_cell', '~> 0.2' # Fork-per-test runner for `rake test` (CRuby only)
  gem 'rbs', '~> 3.0'
  gem 'rb_sys', '~> 0.9' # For building native extension
  gem 'rake-compiler', '~> 1.2'
end
