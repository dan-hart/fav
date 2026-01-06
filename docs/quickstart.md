# Quickstart

## Install

```
cargo install --path .
```

## First run

`fav` requires no manual initialization. The first time you run it, it creates the config file (default `~/.fav.config`) and adds that file as favorite id `1`.

## Add favorites

```
# add current directory
fav add

# add a file
fav add ./notes/todo.md

# add with alias and tags
fav add ~/dotfiles --alias dotfiles --tag config --tag work
```

## List or search

```
fav
fav list --tag work
fav list --search notes
```

## Print a path (for use in other commands)

```
# speed-dial id
fav 3
fav get 3

# alias
fav dotfiles
fav get dotfiles
```

## Run another command with a favorite

```
# appends the path to the command
fav with dotfiles -- ls -la

# or use {} to insert the path
fav with dotfiles -- rg "TODO" {}
```
