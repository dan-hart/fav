# Contributing to fav

Thanks for considering a contribution! This project aims to be fast, minimal, and predictable.

## Quick start

```bash
# install toolchain (if needed)
rustup default stable

# build
cargo build

# run tests
cargo test

# format + lint
cargo fmt
cargo clippy -- -D warnings
```

## Security & safety

- Never commit secrets (API keys, tokens, private keys, `.env`).
- Run the safety checks before committing:

```bash
make security-check
```

## Code style

- Keep CLI output stable and script-friendly.
- Prefer explicit flags and clear error messages.
- Avoid adding dependencies unless they are justified.

## Pull requests

- Include a clear description of the change and why it’s needed.
- Add tests if behavior changes.
- Update docs if a command or flag changes.

## Reporting issues

Use GitHub Issues. Please include:
- What you expected vs what happened
- Steps to reproduce
- Your OS and `fav --version`
