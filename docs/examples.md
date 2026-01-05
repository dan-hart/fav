# Examples

## Use with other commands

```
# open a config folder
ls -la "$(fav dotfiles)"

# use fav with to avoid subshells
fav with dotfiles -- ls -la

# use {} to insert path
fav with 3 -- rg "TODO" {}
```

## Tags and groups

```
# add tags on creation
fav add ~/work/project --alias proj --tag work --tag repo

# add a group
fav group project proj

# list by tag or group
fav list --tag work
fav list --group project
```

## Search and pick

```
# search by alias, path, tag, or group
fav search notes

# interactive selection (requires fzf)
fav pick --tag work

# built-in TUI
fav tui --tag work
```

## Backup and restore

```
# write a backup
fav backup

# restore from a backup file
fav restore ~/.fav.config.backup-1700000000
```

## Import and export

```
# export to stdout
fav export > backup.json

# import from file
fav import --file backup.json

# merge from stdin
cat backup.json | fav import --merge
```
