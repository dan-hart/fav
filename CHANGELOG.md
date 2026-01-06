# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.3] - 2026-01-06
### Added
- `--display-path-format` for `pick` and `tui` to control UI path rendering
### Changed
- Default path-only output now uses absolute paths
- Default display output now uses relative paths

## [0.1.2] - 2026-01-06
### Added
- Package metadata for public distribution
### Changed
- Tightened git-secrets allowlist
- Formatting-only refactors to satisfy clippy/fmt

## [0.1.1] - 2026-01-06
### Added
- Expanded unit and CLI integration tests
- Coverage workflow using `cargo llvm-cov` with a coverage feature

## [0.1.0] - 2026-01-05
### Added
- Core CLI: add/list/search/dial/tag/alias/rm
- Speed‑dial ids and aliases
- Tags and groups
- Import/export, backup/restore, prune/check
- Interactive pick (fzf) and built‑in TUI
- Security tooling and docs
