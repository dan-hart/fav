# Command reference

## Top-level

- Global options: `--config <path>` (defaults to `~/.fav.config` or `FAV_CONFIG` when set)
- `fav` - list favorites (same as `fav list`)
- `fav <id>` / `fav <alias>` - print a favorite path (same as `fav get`)

## add

Add a favorite file or directory.

```
fav add [path] [--alias <name>] [--tag <tag>...] [--id-only]
```

If `path` is omitted, the current directory is used.
`--id-only` prints only the new id (useful in scripts).

## list

List favorites with optional filters and formatting.

```
fav list [--tag <tag>...] [--search <query>]
         [--format table|plain|json]
         [--path-format tilde|absolute|relative]
         [--sort id|alias|path|tag|recent|uses]
         [--reverse]
```

## get

Print a favorite path by id or alias.

```
fav get <target> [--path-format tilde|absolute|relative]
```

## meta

Update metadata for a favorite (alias and tags).

```
fav meta <target> [--alias <name> | --clear-alias]
         [--tag <tag>...] [--set-tags]
         [--rm-tag <tag>...]
         [--rename-tag old=new]
```

Notes:
- `--set-tags` replaces existing tags (only with `--tag`)
- `--rm-tag` removes matching tags (case-insensitive)
- `--rename-tag` renames a tag (case-insensitive match on the old tag)

## pick

Interactive selection using `fzf` (path-only output).

```
fav pick [--tag <tag>...] [--search <query>]
         [--path-format tilde|absolute|relative]
         [--display-path-format tilde|absolute|relative]
         [--sort id|alias|path|tag|recent|uses]
         [--reverse]
```

## tui

Interactive terminal UI (path-only output).

```
fav tui [--tag <tag>...] [--search <query>]
        [--path-format tilde|absolute|relative]
        [--display-path-format tilde|absolute|relative]
        [--sort id|alias|path|tag|recent|uses]
        [--reverse]
```

Controls:
- Type to filter
- ↑/↓ to move
- Enter to select
- Esc/Ctrl+C to quit

## io

Import or export favorites.

```
fav io --export [--file <path>]
fav io --import [--file <path>] [--merge]
```

- Without `--file`, export writes to stdout and import reads from stdin.
- With `--merge`, imported items are appended with new ids (alias conflicts are rejected).

## health

List or prune missing paths.

```
fav health [--path-format tilde|absolute|relative] [--prune]
```

## with

Run a command using a favorite path.

```
fav with <target> -- <command> [args...]
```

Behavior:
- If any argument contains `{}`, it will be replaced with the path.
- Otherwise the path is appended to the end of the command.

Examples:

```
# append the path
fav with dotfiles -- ls -la

# insert the path
fav with dotfiles -- rg "TODO" {}
```

## rm

Remove a favorite by id, alias, or path.

```
fav rm <target>
```
