# Command reference

## Top-level

- Global options: `--config <path>` (defaults to `~/.fav.config` or `FAV_CONFIG` when set)
- `fav` - list favorites (same as `fav list`)
- `fav <id>` - print the path for a speed-dial id
- `fav <alias>` - print the path for an alias

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
fav list [--tag <tag>...] [--group <name>] [--search <query>]
         [--format table|plain|json]
         [--path-format tilde|absolute|relative]
         [--sort id|alias|path|tag|group|recent|uses]
         [--reverse]
```

## search

Search favorites by alias/path/tag/group. Same output options as `list`.

```
fav search <query> [--tag <tag>...] [--group <name>]
          [--format table|plain|json]
          [--path-format tilde|absolute|relative]
          [--sort id|alias|path|tag|group|recent|uses]
          [--reverse]
```

## dial

Print a path by speed-dial id.

```
fav dial <id> [--path-format tilde|absolute|relative]
```

## tag

Manage tags on a favorite.

```
fav tag <target> <tag>... [--set]
fav tag <target> --rm <tag>...
fav tag <target> --rename old=new
```

- `--set` replaces existing tags (only with add)
- `--rm` removes matching tags (case-insensitive)
- `--rename` renames a tag (case-insensitive match on the old tag)

## tags

List all tags with counts.

```
fav tags [--format plain|json]
```

## alias

Set or replace an alias.

```
fav alias <target> <alias>
```

## group

Assign or clear a group.

```
fav group <group> <target>
fav group --clear <target>
```

## groups

List all groups with counts.

```
fav groups [--format plain|json]
```

## pick

Interactive selection using `fzf` (path-only output).

```
fav pick [--tag <tag>...] [--group <name>] [--search <query>]
         [--path-format tilde|absolute|relative]
         [--display-path-format tilde|absolute|relative]
         [--sort id|alias|path|tag|group|recent|uses]
         [--reverse]
```

## tui

Interactive terminal UI (path-only output).

```
fav tui [--tag <tag>...] [--group <name>] [--search <query>]
        [--path-format tilde|absolute|relative]
        [--display-path-format tilde|absolute|relative]
        [--sort id|alias|path|tag|group|recent|uses]
        [--reverse]
```

Controls:
- Type to filter
- ↑/↓ to move
- Enter to select
- Esc/Ctrl+C to quit

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

## export

Export favorites to JSON.

```
fav export [--file <path>]
```

Without `--file`, output is written to stdout.

## import

Import favorites from JSON.

```
fav import [--file <path>] [--merge]
```

- Without `--file`, input is read from stdin.
- With `--merge`, imported items are appended with new ids (alias conflicts are rejected).

## backup

Create a timestamped backup file next to the config.

```
fav backup
```

## restore

Restore from a backup JSON file.

```
fav restore <file>
```

## check

List favorites whose paths no longer exist.

```
fav check [--path-format tilde|absolute|relative]
```

## prune

Remove favorites whose paths no longer exist.

```
fav prune
```

## rm

Remove a favorite by id, alias, or path.

```
fav rm <target>
```
