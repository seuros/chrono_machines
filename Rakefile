# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'minitest/test_task'
require 'shellwords'

# Plain Minitest runner. JRuby and TruffleRuby use it for `rake test`.
Minitest::TestTask.create(:minitest) do |t|
  t.libs << 'test'
  t.libs << 'lib'
  t.test_globs = ['test/**/*_test.rb']
end

# padded_cell runs each test in its own forked cell; it needs CRuby.
desc 'Run the test suite (padded_cell on CRuby, plain Minitest elsewhere)'
if RUBY_ENGINE == 'ruby'
  task :test do
    sh 'padded_cell', *Shellwords.split(ENV.fetch('TESTOPTS', ''))
  end
else
  task test: :minitest
end

GEMSPEC = Gem::Specification.load('chrono_machines.gemspec')

# The extension is built on CRuby only. rb_sys is a runtime dependency, so it
# is present on every engine, but its extension task aborts rather than raising
# when rake-compiler is absent, and rake-compiler is in the mri group.
if RUBY_ENGINE == 'ruby'
  begin
    require 'rb_sys/extensiontask'
  rescue LoadError
    warn 'rb_sys not available; native build tasks disabled'
  end
end

SUPPORTED_NATIVE_PLATFORMS = %w[
  arm64-darwin
  x86_64-darwin
  aarch64-linux
  x86_64-linux
  x86_64-linux-musl
].freeze

if defined?(RbSys::ExtensionTask)
  native_ext = RbSys::ExtensionTask.new('chrono_machines_native', GEMSPEC) do |ext|
    ext.lib_dir = 'lib/chrono_machines_native'
    ext.tmp_dir = 'tmp/rb_sys'
    ext.cross_platform = SUPPORTED_NATIVE_PLATFORMS if ENV.key?('RUBY_TARGET')
  end

  # A single-version cross build stages its binary at the host binary's path,
  # which the gem package depends on, so packaging would also compile for the
  # host and fail where the host linker can't target the platform (aarch64).
  if ENV.key?('RUBY_TARGET')
    host_binary = "#{native_ext.lib_dir}/#{native_ext.name}.#{RbConfig::CONFIG['DLEXT']}"
    Rake::Task[host_binary].clear_prerequisites if Rake::Task.task_defined?(host_binary)
  end

  namespace :native do
    desc 'Compile the native extension in release mode for the current platform'
    task build: ['rb_sys:env:release', 'compile']

    desc 'Build native gems for all supported platforms'
    task :all do
      SUPPORTED_NATIVE_PLATFORMS.each do |platform|
        sh({ 'RUBY_TARGET' => platform }, 'bundle', 'exec', 'rake', 'native:build')
      end
    end
  end
end

task default: :test

Dir[File.join(__dir__, 'tasks/*.rake')].sort.each { |f| load f }
