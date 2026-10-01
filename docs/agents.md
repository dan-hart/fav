# Integrating fav With LLMs and Tools

## Invocation and Discovery

Invoke fav with an argument array rather than constructing a shell string. Keep paths
and queries as single arguments. Global options must precede the `--` separator used
by execution commands. Start with `fav schema` to discover commands, defaults, help,
examples, response fields, and exit codes without loading the store.

```json
["fav", "--json", "--non-interactive", "resolve", "project"]
```

## Response Contract

`--json` is separate from the legacy `list --format json` array format.
Successful responses use `schema_version: 1` and these fields:

| Field | Meaning |
| --- | --- |
| `ok` | True on success |
| `command` | Canonical top-level command |
| `dry_run` | Whether saving/execution was suppressed |
| `data` | Command-specific result, or null |
| `changed_ids` | Sorted unique IDs of changed favorite records |
| `items` | Changed records remaining after the operation |
| `changes` | Objects containing id, before, after; null denotes creation/deletion |
| `presets` | Resulting template collection |
| `output` | Human output represented as strings |
| `warnings` | Guidance represented as strings |

For previews, the changes/items/presets describe the proposed result, while config
contents remain unchanged. A `.lock` file and parent directory may be created to
coordinate access, including during previews and lookups.

### Command Data

| Command | Data |
| --- | --- |
| list, health, doctor duplicates | Favorite array; health returns missing records |
| get, resolve | Object containing id and rendered path |
| ensure | Ensured favorite record |
| io --export | Complete store object |
| preset list | Preset array |
| shell init | Object containing script |
| import | Object containing added and skipped counts |
| with, preset run, open preview | Program and argument array |
| Executed commands | Exit code, stdout, stderr |
| schema --json | Discovery document |

Help/version retain standard text output even with `--json`. JSON command execution
buffers child output; use normal mode for large or streaming output.

## Errors and Recovery

Errors contain schema_version, ok:false, data, and an error object with code, message,
and suggestion. Match codes, not message wording.

| Exit | Code | Recovery |
| --- | --- | --- |
| 0 | Success | Inspect result |
| 2 | invalid_input / invalid_arguments | Correct input using schema/help |
| 3 | not_found | List available favorites/presets |
| 4 | conflict | Use ensure or choose a unique alias |
| 5 | busy | Retry with a bounded delay |
| 6 | storage_error | Check config contents and permissions |
| 7 | execution_failed | Inspect child result/executable availability |

Normal with/preset execution preserves child exit codes. JSON execution uses exit 7
for failure and reports child output/status in data. Execution may fail after usage
was updated; do not automatically retry external commands with side effects.

## Retry-Safe Registration

```bash
fav ensure ./project --alias project --tag active --json
fav ensure ./project --alias project --tag active --json
```

Ensure identifies canonical paths, returns the existing ID, and updates only supplied
alias/note fields. Tags are merged, sorted, and deduplicated. Usage counts are preserved.
An alias belonging to another favorite is a conflict. Use meta to clear fields.

## Lookup and Query Safety

Use resolve or get --no-touch for inspection. Normal get/speed-dial lookup still counts
usage. Query terms are ANDed and case-insensitive. Use `alias:=project` for exact matching
and `note:="release checklist"` for exact multiword text. Single/double quotes group
values; backslash escapes the next character. Prefix `-` negates a term.

Empty queries, bare negation, incomplete quotes/escapes, unsupported fields, invalid IDs,
and invalid booleans fail before mutation. Preview query mutations before applying with
--yes. A preview does not freeze the matched set; another process may change records
before application. Apply explicit IDs separately when exact scope matters.

## Interactive Boundaries

--non-interactive and --json reject pick/tui and imports reading terminal stdin. Supply
--file or pipe input. Piped input can wait for EOF; external commands can still prompt
or hang. Agents should enforce process timeouts and choose noninteractive child commands.
