# Command reference

## Automation Options (All Commands)

`--json` emits the version-1 response envelope. `--non-interactive` refuses selectors
and terminal stdin reads. `--dry-run` previews every mutation/execution without saving
config contents. Global flags must precede the execution `--` separator.

See [Agent integration](agents.md) for response fields/exit codes and
[Automation recipes](automation.md) for safe batching/concurrency.

## schema

`fav schema` prints recursive discovery JSON without loading config.
`fav schema --json` wraps it in the standard response envelope.

## ensure

`fav ensure [path] [--alias name] [--tag tag...] [--note text] [--id-only]`

Register an existing path or return its existing ID. Supplied alias/note replace those
fields; omitted metadata is preserved and tags are merged. Retries preserve ID/usage.

```bash
fav ensure ./project --alias project --tag repo --json
fav ensure ./project --dry-run --json
```

## resolve

`fav resolve <target> [--path-format tilde|absolute|relative]`

Resolve without changing usage. `get --no-touch` has the same behavior.

```bash
fav resolve project --json
fav get project --no-touch
```

## Precise Queries

Terms are ANDed. Quote multiword values and use = for exact matching:

```bash
fav list --query 'note:="release checklist" -tag:=archive' --json
fav meta --query 'alias:=project' --note 'Ready to ship' --dry-run --json
```

Incomplete quotes/escapes, bare negation, empty terms, and invalid fields fail with exit 2.

## Top-level

- Global options: `--config <path>` (defaults to `~/.fav.config` or `FAV_CONFIG` when set)
- `fav` - list favorites (same as `fav list`)
- `fav <id>` / `fav <alias>` - print a favorite path (same as `fav get`)

## add

Add a favorite file or directory.

```
fav add [path] [--alias <name>] [--tag <tag>...] [--note <text>] [--id-only]
```

If `path` is omitted, the current directory is used.
`--id-only` prints only the new id (useful in scripts).

## list

List favorites with optional filters and formatting.

```
fav list [--tag <tag>...] [--search <query>] [--query <expr>] [--smart]
         [--format table|plain|json]
         [--show-notes]
         [--path-format tilde|absolute|relative]
         [--sort id|alias|path|tag|recent|uses]
         [--reverse]
```

Query examples:
- `tag:work note:project`
- `alias:todo -tag:archive`
- `missing:true`
- `dupe:true`

## get

Print a favorite path by id or alias.

```
fav get <target> [--path-format tilde|absolute|relative]
```

## meta

Update metadata for one favorite or a query result set.

```
fav meta <target> [--alias <name> | --clear-alias]
fav meta --query <expr> --yes
         [--note <text> | --clear-note]
         [--tag <tag>...] [--set-tags]
         [--rm-tag <tag>...]
         [--rename-tag old=new]
```

Notes:
- `--set-tags` replaces existing tags (only with `--tag`)
- `--rm-tag` removes matching tags (case-insensitive)
- `--rename-tag` renames a tag (case-insensitive match on the old tag)
- Query-based updates require `--yes`
- Alias changes only work when exactly one favorite matches

## pick

Interactive selection using `fzf` (path-only output).

```
fav pick [--tag <tag>...] [--search <query>] [--query <expr>] [--smart]
         [--path-format tilde|absolute|relative]
         [--display-path-format tilde|absolute|relative]
         [--sort id|alias|path|tag|recent|uses]
         [--reverse]
```

## tui

Interactive terminal UI (path-only output).

```
fav tui [--tag <tag>...] [--search <query>] [--query <expr>] [--smart]
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
fav with <target> [--dry-run] -- <command> [args...]
```

Behavior:
- If any argument contains `{}`, it will be replaced with the path.
- Otherwise the path is appended to the end of the command.
- `--dry-run` prints the resolved command instead of running it.

Examples:

```
# append the path
fav with dotfiles -- ls -la

# insert the path
fav with dotfiles -- rg "TODO" {}
```

## rm

Remove a favorite by id, alias, path, or query.

```
fav rm <target>
fav rm --query <expr> --yes
```

Query-based removals require `--yes`.

## shell

Print shell helpers for `zsh`, `bash`, or `fish`.

```
fav shell init <zsh|bash|fish>
```

This outputs helper functions such as `fcd`, `fopen`, and `frun`.

## open

Open a favorite with the system opener.

```
fav open <target> [--dry-run]
```

## preset

Manage reusable command templates.

```
fav preset add <name> [--note <text>] -- <command> [args...]
fav preset list
fav preset run <name> <target> [--dry-run]
fav preset rm <name>
```

Presets use the same `{}` placeholder rules as `fav with`.

## doctor

Inspect duplicates or repair missing paths.

```
fav doctor duplicates
fav doctor repair --from <old-root> --to <new-root> [--dry-run]
```

`duplicates` reports exact duplicate paths and normalized alias collisions.
`repair` rewrites missing paths when the new path exists under the replacement root.

## import

Import favorites from shell history or path files.

```
fav import history [--shell auto|zsh|bash|fish] [--file <path>] [--limit <n>]
                  [--tag <tag>...] [--note <text>]

fav import paths [--file <path>] [--tag <tag>...] [--note <text>]
```

`import history` reads shell history and imports discovered paths.
`import paths` accepts newline-delimited paths, CSV, JSON arrays of strings, and bookmark HTML files with `file://` links.
