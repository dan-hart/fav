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

## Tags and metadata

```
# add tags on creation
fav add ~/work/project --alias proj --tag work --tag repo

# add tags later
fav meta proj --tag infra

# list by tag
fav list --tag work
```

## Search and pick

```
# search by alias, path, or tag
fav list --search notes

# interactive selection (requires fzf)
fav pick --tag work

# built-in TUI
fav tui --tag work
```

## Import and export

```
# export to stdout
fav io --export > backup.json

# import from file
fav io --import --file backup.json

# merge from stdin
cat backup.json | fav io --import --merge
```
