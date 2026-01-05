# AGENTS.md

Guidance for coding agents working in this repo.

## ⚠️ CRITICAL: ASP (AI Safety Policy)

**Never commit secrets. Always run ASP preflight and git-secrets scans before commit.**

Required commands before committing:

```bash
./scripts/utilities/asp-preflight.sh --staged --strict

git secrets --scan --cached
```

If ASP flags a data path/system/display change, you must get explicit approval
and re-run with the relevant ack flags.

## Rust CLI best practices

- Keep stdout stable and script-friendly. Put human tips on stderr.
- Prefer clear, minimal subcommands and short flags.
- Avoid breaking changes to output formats.
- Provide `--help` examples for every command.
- Prefer deterministic ordering in list outputs.
- Treat paths carefully: expand `~`, resolve relative paths, avoid surprising mutations.
- Keep the config format stable; if you must change it, provide migration.
- Handle errors with actionable messages.

## Testing expectations

- Add tests when changing filtering, sorting, or output formatting.
- Run `cargo test` before commit.

## Security

- Never check in `.env`, keys, or credentials.
- Use `.gitignore` and `git-secrets` patterns.
- Keep examples sanitized (no real secrets).

## Documentation

- Update `docs/commands.md` when commands or flags change.
- Update `README.md` for user-facing changes.
