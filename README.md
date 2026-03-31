# fav

Speed-dial for files and folders.

Remember paths once, then use them everywhere.

- Instant path recall by id or alias
- Tags for organization
- Notes, structured queries, and smarter ranking
- `fav with` runs commands with a favorite path (no subshells)
- Shell helpers, presets, duplicate checks, repair tools, and importers
- Works on macOS and Linux

## Install

```bash
# Homebrew
brew install dan-hart/tap/fav

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

## Contributing

See `CONTRIBUTING.md` for development and contribution guidelines.

## Code of Conduct

See `CODE_OF_CONDUCT.md`.

## Security

This repo uses `git-secrets` and ASP preflight checks to prevent credential leaks.
See `docs/security.md` for setup and policy.

## License

GPL-3.0-only. See `LICENSE`.
