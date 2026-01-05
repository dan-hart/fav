# Data format

The config file is JSON with this shape:

```
{
  "version": 1,
  "next_id": 4,
  "items": [
    {
      "id": 1,
      "path": "/Users/you/.fav.config",
      "alias": null,
      "tags": [],
      "group": null,
      "uses": 0,
      "last_used": null
    }
  ]
}
```

Fields:

- `version` - schema version
- `next_id` - next id to assign when adding favorites
- `items` - array of favorites

Favorite fields:

- `id` - stable speed-dial id
- `path` - absolute path stored at add time
- `alias` - optional alias string
- `tags` - list of tag strings
- `group` - optional group string
- `uses` - usage count (incremented by dial/alias/pick/with)
- `last_used` - UNIX timestamp in seconds
