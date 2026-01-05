# Security

This repo is designed to be safe by default. Do **not** commit secrets, keys, or personal data.

## Quick setup (recommended)

```bash
# Install git-secrets for this repo
./scripts/automation/setup-git-secrets.sh

# Install ASP pre-commit hook (blocks risky changes)
./scripts/automation/install-asp-hooks.sh .
```

## Policy

- Never commit credentials (API keys, tokens, passwords, private keys, `.env`).
- Keep examples sanitized (use placeholders like `YOUR_API_KEY_HERE`).
- Treat this repo as public.

## Secrets management

- Use a secrets manager (1Password, keychain, vault) instead of storing secrets in files.
- If you need env vars locally, use `.env` (ignored) and document placeholders in `.env.example`.
- Rotate any secret that was ever committed.

## Key protection

- Store private keys only in a password manager or encrypted storage.
- Keep file permissions restrictive (`chmod 600` for private keys).
- Never copy keys into this repository.

## Pre-commit protection

This repo ships two protections:

1) **git-secrets**
- Blocks commits that match common secret patterns.

2) **ASP preflight**
- Blocks sensitive files, warns on risky filenames, and checks for data path/system changes.

Run manually if needed:

```bash
./scripts/utilities/asp-preflight.sh --staged --strict
```

## Incident response

If a secret is committed:

1. **Rotate it immediately**.
2. Remove it from git history.
3. Re-run `git secrets --scan-history`.

## Audit checklist (monthly)

```bash
# Scan history
 git secrets --scan-history

# Search common patterns
git grep -E "(credential|secret|token|password)" -i
```
