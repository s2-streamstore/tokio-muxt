# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.1](https://github.com/s2-streamstore/tokio-muxt/compare/v0.8.0...v0.8.1) - 2026-11-06

### Other

- exclude CI and agent files from published crate ([#20](https://github.com/s2-streamstore/tokio-muxt/pull/20))
- publish to crates.io via trusted publishing ([#18](https://github.com/s2-streamstore/tokio-muxt/pull/18))

## [0.8.0](https://github.com/s2-streamstore/tokio-muxt/compare/v0.7.0...v0.8.0) - 2026-11-06

### Fixed

- pend when disarmed and saturate `fire_after` overflow ([#17](https://github.com/s2-streamstore/tokio-muxt/pull/17))

### Other

- [**breaking**] remove MuxTimer default constructor in favor of `MuxTimer::new()` ([#15](https://github.com/s2-streamstore/tokio-muxt/pull/15))
- upgrade and SHA-pin GitHub Actions dependencies ([#13](https://github.com/s2-streamstore/tokio-muxt/pull/13))

## [0.7.0](https://github.com/s2-streamstore/tokio-muxt/compare/v0.6.1...v0.7.0) - 2026-01-31

### Other

- readme ([#11](https://github.com/s2-streamstore/tokio-muxt/pull/11))
- [**breaking**] clippy, cleanups, and 2024 Rust edition ([#9](https://github.com/s2-streamstore/tokio-muxt/pull/9))

## [0.6.1](https://github.com/s2-streamstore/tokio-muxt/compare/v0.6.0...v0.6.1) - 2026-01-31

### Other

- expand mux timer coverage ([#8](https://github.com/s2-streamstore/tokio-muxt/pull/8))
- Fix latest-mode rearm for earlier deadlines ([#5](https://github.com/s2-streamstore/tokio-muxt/pull/5))
- add CI and release-plz workflows ([#6](https://github.com/s2-streamstore/tokio-muxt/pull/6))
- change license to MIT ([#3](https://github.com/s2-streamstore/tokio-muxt/pull/3))
