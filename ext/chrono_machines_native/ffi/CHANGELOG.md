# Changelog

## [0.3.0](https://github.com/seuros/chrono_machines/compare/chrono_machines_native-v0.2.3...chrono_machines_native-v0.3.0) (2026-06-24)


### Features

* **native:** make core crate no_std capable ([#15](https://github.com/seuros/chrono_machines/issues/15)) ([76129a3](https://github.com/seuros/chrono_machines/commit/76129a31ad75142641e4a29a82424ad26fb9ecd6))


### Bug Fixes

* **native:** use workspace dependency for chrono-machines in ffi ([fdeca2f](https://github.com/seuros/chrono_machines/commit/fdeca2f938e2647107793cfcf4c5c7c77e62e09b))

## [0.2.3](https://github.com/seuros/chrono_machines/compare/chrono_machines_native-v0.2.2...chrono_machines_native-v0.2.3) (2026-06-15)


### Bug Fixes

* **native:** deduplicate backoff jitter, fibonacci, and retry-failure logic ([46f861a](https://github.com/seuros/chrono_machines/commit/46f861acc7e29024fc35dafb39aaec22db33440d))

## [0.2.2](https://github.com/seuros/chrono_machines/compare/chrono_machines_native-v0.2.1...chrono_machines_native-v0.2.2) (2026-05-25)


### Bug Fixes

* remove unused Magnus embed feature ([3b0877a](https://github.com/seuros/chrono_machines/commit/3b0877a11f768a135c877980320cae53fa58cd3d))

## [0.2.1](https://github.com/seuros/chrono_machines/compare/chrono_machines_native-v0.2.0...chrono_machines_native-v0.2.1) (2026-03-19)


### Bug Fixes

* **native:** upgrade rand to 0.10 ([#10](https://github.com/seuros/chrono_machines/issues/10)) ([d7e2738](https://github.com/seuros/chrono_machines/commit/d7e2738552a98e4f528055f671cfdfbdb02b0c7a))

## [0.2.0](https://github.com/seuros/chrono_machines/compare/chrono_machines_native-v0.1.0...chrono_machines_native-v0.2.0) (2025-12-19)


### Features

* add async support, error classification ([#6](https://github.com/seuros/chrono_machines/issues/6)) ([3d6d2c6](https://github.com/seuros/chrono_machines/commit/3d6d2c6e06de58247c74cdcef94e78a099b11e31))
* add constant and fibonacci backoff strategies with native acceleration ([582ba9c](https://github.com/seuros/chrono_machines/commit/582ba9ccd84072ec4ccf5d3340eced27cc4d1925))
* Add Rust native speedup with fluent retry DSL ([#4](https://github.com/seuros/chrono_machines/issues/4)) ([ac980bf](https://github.com/seuros/chrono_machines/commit/ac980bfda450b4e63739e4dc3c19513dd989e819))


### Bug Fixes

* add Ruby 4.0.0-preview3 to test matrix ([#9](https://github.com/seuros/chrono_machines/issues/9)) ([df8a3a2](https://github.com/seuros/chrono_machines/commit/df8a3a23acf5ea37b115ab7d05936331b4ae2018))
