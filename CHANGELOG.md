# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.3.0] - 2026-10-01
### Added
- Global versioned JSON responses, structured errors, and documented exit codes
- Machine-readable command discovery through schema
- Non-interactive mode and mutation previews across commands
- Idempotent ensure, read-only resolve, and get --no-touch
- Quoted query values and exact field matching using field:=value
- Store locking, unique atomic replacement files, and concurrent writer tests
- Detailed help/examples, agent/automation/compatibility guides, and README badges
- Windows opener and macOS/Linux/Windows CI coverage
### Changed
- JSON execution captures child stdout/stderr and reports failure with exit 7
- Existing path output and list --format json retain their previous formats

## [0.2.0] - 2026-03-31
### Added
- Notes on favorites via `add --note` and `meta --note`
- Structured query filtering, smart ranking, and optional note display in listings
- Expert commands for shell helpers, system open, reusable presets, duplicate checks, path repair, and importing from history/path files
- `with --dry-run` and query-based batch `meta`/`rm` flows guarded by `--yes`

## [0.1.4] - 2026-01-06
### Added
- `get`, `meta`, `io`, and `health` commands for a simpler CLI
### Changed
- Consolidated metadata commands into `meta`
- Collapsed import/export/backup/restore into `io`
- Replaced check/prune with `health`
- Removed groups from outputs, sorting, and the data format

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
