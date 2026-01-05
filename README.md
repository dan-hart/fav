# fav

Speed-dial for files and folders.

Remember paths once, then use them everywhere.

- Instant path recall by id or alias
- Tags + groups for organization
- `fav with` runs commands with a favorite path (no subshells)
- Search, pick (fzf), export/import, backup/restore
- Works on macOS and Linux

## Install

```bash
cargo install --path .
```

## Quick start

```bash
# add current directory
fav add

# add a file with alias and tags
fav add ./notes/todo.md --alias todo --tag notes

# list favorites
fav

# use a favorite path directly
fav todo

# run a command with a favorite path
fav with todo -- cat
```

## Usage highlights

```bash
# speed-dial by id
fav 3
fav dial 3

# search + filter
fav search notes
fav list --tag work --group project

# interactive pick (requires fzf)
fav pick --tag work

# built-in TUI
fav tui --tag work

# export/backup
fav export > backup.json
fav backup
```

## Config

Favorites are stored per-user in `~/.fav.config` by default.
Override with `--config <path>` or `FAV_CONFIG=<path>`.

On first run, the config file path itself is added as favorite id `1`.

## Docs

Full documentation lives in `docs/`.

## Security

This repo uses `git-secrets` and ASP preflight checks to prevent credential leaks.
See `docs/security.md` for setup and policy.

## License

GPL-3.0-only. See `LICENSE`.
