# Release Checklist

Use this checklist when shipping a new `fav` release and updating Homebrew.

## 1. Prepare the repo

- Update `Cargo.toml` and `Cargo.lock` to the new version.
- Move the relevant `CHANGELOG.md` entries from `Unreleased` to the new version heading.
- Update install docs if the release changes how users should install or upgrade.

## 2. Verify locally

- `cargo fmt --check`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test --locked`
- `cargo test --locked --features coverage`
- `cargo build --release`
- `./scripts/utilities/asp-preflight.sh --staged --strict`
- `git secrets --scan --cached`

## 3. Publish the app release

- Commit and push the release changes on `main`.
- Wait for macOS, Linux, and Windows CI to pass on the release commit.
- Create and push the release tag, for example `v0.3.0`.
- Create the GitHub release in `dan-hart/fav`.

## 4. Prepare the Homebrew source

The app repository is public. Homebrew can download the tagged source directly.

- Download `https://github.com/dan-hart/fav/archive/refs/tags/v0.3.0.tar.gz`.
- Compute its SHA-256 with `shasum -a 256` for the formula.
- Verify the archive contains the released source and lockfile.

## 5. Update the tap

- Add or update `Formula/fav.rb` in `dan-hart/homebrew-tap`.
- Point `url` at the public app tag archive and `homepage` at the app repository.
- Update `sha256` and `version`.
- Commit and push the tap change.

## 6. Verify Homebrew

- `brew update`
- `brew reinstall dan-hart/tap/fav` or `brew install dan-hart/tap/fav`
- `brew test dan-hart/tap/fav`
- `fav --version`
