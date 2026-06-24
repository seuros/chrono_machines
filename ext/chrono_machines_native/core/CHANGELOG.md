# Changelog

## [0.4.0](https://github.com/seuros/chrono_machines/compare/chrono-machines-v0.3.2...chrono-machines-v0.4.0) (2026-06-24)


### Features

* **native:** make core crate no_std capable ([#15](https://github.com/seuros/chrono_machines/issues/15)) ([76129a3](https://github.com/seuros/chrono_machines/commit/76129a31ad75142641e4a29a82424ad26fb9ecd6))

## [0.3.2](https://github.com/seuros/chrono_machines/compare/chrono-machines-v0.3.1...chrono-machines-v0.3.2) (2026-06-15)


### Bug Fixes

* **native:** deduplicate backoff jitter, fibonacci, and retry-failure logic ([46f861a](https://github.com/seuros/chrono_machines/commit/46f861acc7e29024fc35dafb39aaec22db33440d))

## [0.3.1](https://github.com/seuros/chrono_machines/compare/chrono-machines-v0.3.0...chrono-machines-v0.3.1) (2026-03-19)


### Bug Fixes

* **native:** upgrade rand to 0.10 ([#10](https://github.com/seuros/chrono_machines/issues/10)) ([d7e2738](https://github.com/seuros/chrono_machines/commit/d7e2738552a98e4f528055f671cfdfbdb02b0c7a))

## [0.3.0](https://github.com/seuros/chrono_machines/compare/chrono-machines-v0.2.1...chrono-machines-v0.3.0) (2025-12-19)


### Features

* add async support, error classification ([#6](https://github.com/seuros/chrono_machines/issues/6)) ([3d6d2c6](https://github.com/seuros/chrono_machines/commit/3d6d2c6e06de58247c74cdcef94e78a099b11e31))
* add constant and fibonacci backoff strategies with native acceleration ([582ba9c](https://github.com/seuros/chrono_machines/commit/582ba9ccd84072ec4ccf5d3340eced27cc4d1925))
* Add Rust native speedup with fluent retry DSL ([#4](https://github.com/seuros/chrono_machines/issues/4)) ([ac980bf](https://github.com/seuros/chrono_machines/commit/ac980bfda450b4e63739e4dc3c19513dd989e819))
