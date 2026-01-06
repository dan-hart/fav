# Paths and config

## Config location

By default, favorites are stored in:

```
~/.fav.config
```

You can override the config path with either:

- `--config <path>`
- `FAV_CONFIG=<path>`

The override is resolved with `~` expansion and relative paths are resolved against the current working directory.

## Per-user data

The config file lives in the current user's home directory, so each user gets an isolated favorites list by default.

## First favorite

On first run, `fav` creates the config file and automatically adds that file as favorite id `1`.

## Path rendering

Some commands support `--path-format`:

- `tilde` - use `~` for the home directory prefix
- `absolute` - full absolute path
- `relative` - relative to the current working directory when possible

Defaults:

- Display commands (`list`, `health`) render paths as `relative` by default.
- Path-only output (`get`, `fav <id>`, `fav <alias>`, `pick`, `tui`) uses `absolute` by default.
- `pick` and `tui` also accept `--display-path-format` to control the UI list paths separately.

## Output formats

`list` supports:

- `table` (default) - aligned columns
- `plain` - tab-separated fields (id, alias, path, tags)
- `json` - machine-readable output
