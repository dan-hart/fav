# Automation Recipes

## Register and Resolve

```bash
fav ensure ./project --alias project --tag repo --json
fav resolve project
fav get project --no-touch --json
```

Paths must exist when registering. Missing stored paths remain resolvable for diagnosis.

## Preview Batch Changes

```bash
fav meta --query 'tag:=repo -tag:=archive' --note 'Active repository' --dry-run --json
fav meta --query 'tag:=repo -tag:=archive' --note 'Active repository' --yes --json
fav rm --query 'missing:true' --dry-run --json
fav rm --query 'missing:true' --yes --json
```

Inspect changes/changed_ids before applying. Previews use the same validation and
in-memory mutations as application, suppressing saves and external launches. Supported
operations include add/ensure, metadata, removal, imports, pruning, repair, presets,
export-to-file, open, and command execution. Reads/stdout exports remain readable.

## Imports and Backups

```bash
fav import paths --file paths.json --tag imported --dry-run --json
fav import paths --file paths.json --tag imported --json
fav import history --shell zsh --limit 50 --dry-run --json
fav io --export --file favorites-backup.json
fav io --import --merge --file favorites-backup.json --dry-run --json
fav io --import --merge --file favorites-backup.json --json
```

History can contain private paths. Imports stay local. Exports include absolute paths,
notes, aliases, and presets; inspect them before sharing.

## Execute With Substitution

```bash
fav with project --dry-run --json -- rg TODO {}
fav preset add scan -- rg --hidden TODO {}
fav preset run scan project --dry-run --json
```

Execution passes arguments directly without a shell. `{}` is replaced inside argument
tokens; when absent, the path is appended. Pipes require explicitly invoking a shell.

## Concurrent Agents

Each process locks `<config>.lock` before loading/modifying the store. Contention waits
up to five seconds, then returns busy/exit 5. Writes use unique temporary files in the
config directory, sync contents, and atomically replace the store. Locks are released
before interactive selection/external execution; selection reloads under lock before
updating usage.

Use --config/FAV_CONFIG to isolate jobs. Do not delete locks while processes run.
Prefer a local filesystem: network filesystem locking/rename semantics vary. Atomic
replacement prevents partial JSON writes but is not a backup or a full power-loss
durability guarantee.
