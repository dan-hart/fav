# fav

[![CI](https://github.com/dan-hart/fav/actions/workflows/ci.yml/badge.svg)](https://github.com/dan-hart/fav/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/dan-hart/fav)](https://github.com/dan-hart/fav/releases)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![Homebrew](https://img.shields.io/badge/Homebrew-dan--hart%2Ftap-orange)](https://github.com/dan-hart/homebrew-tap)

**Your filesystem. On speed dial. Ready for scripts and AI agents.**

Remember paths once, then use them everywhere.

- Instant path recall by id or alias
- Tags for organization
- Notes, structured queries, and smarter ranking
- `fav with` runs commands with a favorite path (no subshells)
- Shell helpers, presets, duplicate checks, repair tools, and importers
- Tested on macOS, Linux, and Windows
- Versioned JSON, command discovery, previews, and safe concurrent updates

## Discover. Resolve. Preview. Apply.

```bash
# Discover commands, arguments, examples, and response contracts
fav schema

# Safe to retry: same path, same ID, preserved usage
fav ensure ~/projects/site --alias site --tag active --note "Release checklist" --json

# Look up a path without changing its ranking
fav resolve site --json

# Preview precise batch changes before applying
fav meta --query 'note:="release checklist"' --tag release --dry-run --json
fav meta --query 'note:="release checklist"' --tag release --yes --json

# Inspect the exact argument array before execution
fav with site --dry-run --json -- rg TODO {}

# Find active favorites without parsing a table
fav list --query 'tag:=active -tag:=archive' --json
```

`--json` emits one JSON document. `--non-interactive` refuses interactive selection.
`--dry-run` previews changes without saving config contents or launching commands.
Existing path output and `list --format json` remain available for scripts.

## Install

```bash
# Homebrew
brew install dan-hart/tap/fav

# upgrade
brew update && brew upgrade dan-hart/tap/fav

# local source checkout
cargo install --path .
```

## Getting started

```bash
# save the current directory
fav add

# see your ids and aliases
fav list

# recall a path by id or alias
fav 1
```

## Quick start

```bash
# add current directory
fav add

# add a file with alias, tags, and a note
fav add ./notes/todo.md --alias todo --tag notes --note "Daily scratchpad"

# list favorites
fav

# use a favorite path directly
fav todo

# run a command with a favorite path
fav with todo -- cat

# preview the resolved command without running it
fav with todo --dry-run -- rg "TODO"
```

## Usage highlights

```bash
# speed-dial by id
fav 3
fav get 3

# search + filter
fav list --search notes
fav list --tag work
fav list --query "tag:work note:project" --smart

# interactive pick (requires fzf)
fav pick --tag work

# built-in TUI
fav tui --tag work

# shell helpers
eval "$(fav shell init zsh)"
fcd todo

# open paths and run presets
fav open todo
fav preset add grep-todo -- rg "TODO" {}
fav preset run grep-todo todo

# inspect duplicates or preview a path repair
fav doctor duplicates
fav doctor repair --from ~/old-root --to ~/new-root --dry-run

# import from shell history or path files
fav import history --shell zsh --limit 50
fav import paths --file ./paths.json --tag imported

# export/import
fav io --export > backup.json
fav io --import --file backup.json
```

## Config

Favorites are stored per-user in `~/.fav.config` by default.
Override with `--config <path>` or `FAV_CONFIG=<path>`.

On first run, the config file path itself is added as favorite id `1`.

## Docs

Full documentation lives in `docs/`.

- [Command reference](docs/commands.md): flags and examples for every command
- [Agent integration](docs/agents.md): response schema, exit codes, and retries
- [Automation recipes](docs/automation.md): batching, backups, and concurrency
- [Compatibility](docs/compatibility.md): platforms, shells, and unusual paths
- [Quickstart](docs/quickstart.md): your first favorites
- [Release checklist](RELEASE.md): verification and Homebrew publishing

## Contributing

See `CONTRIBUTING.md` for development and contribution guidelines.

## Code of Conduct

See `CODE_OF_CONDUCT.md`.

## Security

This repo uses `git-secrets` and ASP preflight checks to prevent credential leaks.
See `docs/security.md` for setup and policy.

## License

GPL-3.0-only. See `LICENSE`.
