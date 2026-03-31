# Fav Expert Workflows Implementation Plan

**Goal:** Add shell integration, smarter ranking, queryable notes, expert commands, presets, repair tools, and importers without breaking the current `fav` workflow.

**Architecture:** Keep the current Rust binary structure and add focused helper functions/types inside the existing codebase rather than doing a broad refactor first. Advanced features are isolated behind new top-level expert commands and additive flags, while the current `add/list/get/with` behavior remains compatible.

**Tech Stack:** Rust, clap, serde/serde_json, assert_cmd, predicates, tempfile

---

### Task 1: Extend the store model safely

**Files:**
- Modify: `src/main.rs`
- Test: `src/main.rs`

- [ ] Add `note` to `Favorite` and `presets` to `Store` using `#[serde(default)]`.
- [ ] Add a `Preset` struct and store sync helpers.
- [ ] Update store seeding and migration logic for version `2`.
- [ ] Write/adjust unit tests proving old stores still load and new fields round-trip.
- [ ] Run: `cargo test tests::save_and_load_store_roundtrip -- --exact`

### Task 2: Add query parsing, smart ranking, and note-aware filtering

**Files:**
- Modify: `src/main.rs`
- Test: `src/main.rs`
- Test: `tests/cli.rs`

- [ ] Write failing unit tests for query parsing and query matching.
- [ ] Write failing unit tests for smart ranking.
- [ ] Add `--query`, `--smart`, and `--show-notes` support to list/pick/tui/meta/rm plumbing.
- [ ] Implement minimal parser and matcher for free text, field terms, and negation.
- [ ] Implement smart ranking that boosts exact matches, prefix matches, usage, and recency.
- [ ] Run focused tests, then `cargo test`.

### Task 3: Add note editing and batch-safe metadata/removal flows

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/cli.rs`
- Modify: `docs/commands.md`

- [ ] Write failing CLI tests for `add --note`, `meta --note`, `meta --query`, and `rm --query --yes`.
- [ ] Extend clap args without breaking current positional targeting.
- [ ] Implement query-based mutation helpers with explicit safety checks.
- [ ] Update command reference examples.
- [ ] Run: `cargo test tests::cli -- --nocapture`

### Task 4: Add shell, open, preset, and doctor expert commands

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/commands.md`

- [ ] Write failing CLI tests for `shell init`, `open --dry-run`, preset CRUD/run, `doctor duplicates`, and `doctor repair --dry-run`.
- [ ] Implement shell script generation for zsh, bash, and fish.
- [ ] Implement platform opener command resolution with dry-run output.
- [ ] Implement preset storage and execution using the same `{}` replacement rules as `with`.
- [ ] Implement duplicate reporting and prefix-based repair flows.
- [ ] Update docs with example expert workflows.
- [ ] Run: `cargo test`

### Task 5: Add importers for history and path files

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/commands.md`

- [ ] Write failing tests for history import and generic path import.
- [ ] Implement history parsing for zsh, bash, and fish with auto-detection.
- [ ] Implement generic path import for newline-delimited, CSV, and JSON array inputs.
- [ ] Ensure duplicate paths are skipped with a clear stderr summary.
- [ ] Update docs with import examples.
- [ ] Run: `cargo test`

### Task 6: Verify, polish, and preserve compatibility

**Files:**
- Modify: `README.md`
- Modify: `docs/commands.md`
- Modify: `CHANGELOG.md`

- [ ] Review `--help` output for command clarity and examples.
- [ ] Update changelog and user-facing docs for the new expert surfaces.
- [ ] Run: `cargo fmt --check`
- [ ] Run: `cargo clippy -- -D warnings`
- [ ] Run: `cargo test`
- [ ] Run: `cargo build --release`
