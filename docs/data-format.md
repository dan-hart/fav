# Data format

The config file is JSON with this shape:

```
{
  "version": 2,
  "next_id": 4,
  "items": [
    {
      "id": 1,
      "path": "/Users/you/.fav.config",
      "alias": null,
      "tags": [],
      "note": null,
      "uses": 0,
      "last_used": null
    }
  ],
  "presets": [
    {
      "name": "grep-todo",
      "command": ["rg", "TODO", "{}"],
      "note": "Search the selected favorite for TODO markers"
    }
  ]
}
```

Fields:

- `version` - schema version
- `next_id` - next id to assign when adding favorites
- `items` - array of favorites
- `presets` - array of saved command templates used by `fav preset`

Favorite fields:

- `id` - stable speed-dial id
- `path` - absolute path stored at add time
- `alias` - optional alias string
- `tags` - list of tag strings
- `note` - optional note/description string
- `uses` - usage count (incremented by get, `fav <id>`, `fav <alias>`, pick, with)
- `last_used` - UNIX timestamp in seconds

Preset fields:

- `name` - preset name
- `command` - command tokens with optional `{}` placeholder
- `note` - optional preset description
