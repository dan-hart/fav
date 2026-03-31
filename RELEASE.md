# Release Checklist

Use this checklist when shipping a new `fav` release and updating Homebrew.

## 1. Prepare the repo

- Update `Cargo.toml` and `Cargo.lock` to the new version.
- Move the relevant `CHANGELOG.md` entries from `Unreleased` to the new version heading.
- Update install docs if the release changes how users should install or upgrade.

## 2. Verify locally

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `cargo build --release`
- `./scripts/utilities/asp-preflight.sh --staged --strict`
- `git secrets --scan --cached`

## 3. Publish the app release

- Commit and push the release changes on `main`.
- Create and push the release tag, for example `v0.2.0`.
- Create the GitHub release in `dan-hart/fav`.

## 4. Publish the Homebrew source asset

`fav` can stay private while Homebrew remains public by publishing a source tarball on `dan-hart/homebrew-tap`.

- Create a source tarball from the release commit with a top-level directory such as `fav-0.2.0/`.
- Upload that tarball to a release on `dan-hart/homebrew-tap`, using a tag such as `fav-v0.2.0`.
- Compute the tarball SHA-256 for the formula.

## 5. Update the tap

- Add or update `Formula/fav.rb` in `dan-hart/homebrew-tap`.
- Point `url` at the public tap release asset.
- Update `sha256` and `version`.
- Commit and push the tap change.

## 6. Verify Homebrew

- `brew update`
- `brew reinstall dan-hart/tap/fav` or `brew install dan-hart/tap/fav`
- `brew test dan-hart/tap/fav`
- `fav --version`
