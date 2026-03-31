# Fav Expert Workflows Design

**Date:** 2026-03-30

**Goal:** Add ten power-user improvements to `fav` while preserving the simplicity of the existing core workflow: `add`, `list`, `get`, and `with`.

## Constraints

- Keep existing commands and config backward-compatible.
- Bias toward new users first: advanced behavior should live behind explicit expert commands or flags.
- Avoid broad architectural churn in this pass. Reuse the current single-binary structure unless a small helper extraction clearly lowers risk.
- Preserve stable stdout behavior for script-friendly commands; put guidance on stderr.

## Product Shape

The CLI keeps its current front door:

- `fav add`
- `fav list`
- `fav get`
- `fav with`

Advanced behavior is added as explicit expert surfaces:

- `fav shell init <zsh|bash|fish>`
- `fav open <target>`
- `fav import <subcommand>`
- `fav doctor <subcommand>`
- `fav preset <subcommand>`

Existing commands gain a few additive flags where the behavior is obvious:

- `fav add --note <text>`
- `fav meta --note <text> | --clear-note`
- `fav list --query <expr> --smart --show-notes`
- `fav pick --query <expr> --smart`
- `fav tui --query <expr> --smart`
- `fav with --dry-run`
- `fav rm --query <expr> --yes`
- `fav meta --query <expr> ...`

## Feature Mapping

1. Shell integration

- `fav shell init zsh|bash|fish` prints shell glue for:
  - `fcd <target>`: `cd` into a favorite in the parent shell
  - `fopen <target>`: open a favorite
  - `frun <preset> <target>`: run a saved preset against a favorite
- Output is designed to be sourced with `eval "$(fav shell init zsh)"`.

2. Smarter ranking

- Add `--smart` to `list`, `pick`, and `tui`.
- Smart ranking blends:
  - exact alias matches first
  - prefix alias/path/note matches
  - substring matches
  - usage count
  - recency
- `pick` and `tui` also use smart ranking automatically when an interactive filter is present, keeping the feature helpful without changing list defaults.

3. Open subcommand

- `fav open <target>` resolves a favorite and opens it with:
  - `open` on macOS
  - `xdg-open` on Linux
- `--dry-run` prints the launch command instead of running it.

4. Query language

- Add a small query language for `list`, `pick`, `tui`, `meta`, and `rm`.
- First-cut grammar is whitespace-separated AND terms:
  - free text matches alias/path/tags/note
  - `alias:<text>`
  - `path:<text>`
  - `tag:<text>`
  - `note:<text>`
  - `id:<number>`
  - `missing:true|false`
  - `dupe:true|false`
- A `-` prefix negates a term, for example `-tag:archive`.

5. Batch operations

- `fav meta` can target one item or a query result set.
- `fav rm` can target one item or a query result set.
- Query-based destructive operations require `--yes` so casual use stays safe.

6. Duplicate detection

- `fav doctor duplicates` reports:
  - exact duplicate canonical paths
  - alias collisions after normalization (`foo-bar` vs `foo_bar`)
- Duplicate status is also exposed to the query engine via `dupe:true`.

7. Path repair

- `fav doctor repair --from <old> --to <new>` rewrites missing paths by replacing a path prefix and validating the new location exists.
- `--dry-run` previews changes.
- This stays intentionally simple and deterministic; no interactive rename wizard in the first cut.

8. Notes/descriptions

- Favorites gain an optional `note` field.
- Notes are searchable and queryable, but hidden from the default list/table output to keep the core uncluttered.
- `--show-notes` reveals them in human-readable list output.

9. Safer automation ergonomics

- `fav with --dry-run` shows the final command line.
- Saved presets allow repeatable command templates:
  - `fav preset add <name> -- <command...>`
  - `fav preset list`
  - `fav preset run <name> <target>`
  - `fav preset rm <name>`
- Presets support the existing `{}` placeholder behavior.

10. External importers

- `fav import history [--shell auto|zsh|bash|fish] [--limit N]`
  - Parses shell history and imports paths seen in `cd` commands or standalone path arguments.
- `fav import paths --file <path>`
  - Imports from newline-delimited paths, CSV, or a JSON array of path strings.
- Importers skip duplicates, can attach tags/notes, and report how many favorites were added vs skipped.

## Data Model

`Store` remains JSON-backed and backward-compatible.

- `Store.version` increments to `2`.
- `Favorite` gains:
  - `note: Option<String>` with `#[serde(default)]`
- `Store` gains:
  - `presets: Vec<Preset>` with `#[serde(default)]`
- `Preset` includes:
  - `name: String`
  - `command: Vec<String>`
  - `note: Option<String>`

Old stores load cleanly because all new fields are optional/defaulted.

## UX and Output Rules

- Path-only commands (`get`, alias lookup, `pick`, `tui`) keep stdout path-only.
- Human guidance remains on stderr.
- Destructive query operations require explicit confirmation flags.
- Default `list` stays visually compact; advanced metadata is opt-in.

## Error Handling

- Query parse failures return actionable errors with examples.
- Importers report unreadable or unsupported files clearly.
- `open` returns a platform-specific hint if no opener is available.
- Repair mode reports unchanged, repaired, and failed rows separately.

## Testing Strategy

- Extend CLI tests for every new command and behavior gate.
- Add focused unit tests for:
  - query parsing and matching
  - smart ranking
  - duplicate detection
  - repair path rewriting
  - importer parsing
  - preset execution argument expansion
- Keep existing behavior tests green to prove compatibility.

## Non-Goals

- No daemon, background sync, or OS-specific service integration.
- No interactive repair wizard.
- No breaking redesign of current commands.
- No remote sync or cloud account concept.
