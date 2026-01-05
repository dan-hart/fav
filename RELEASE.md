# Release checklist

1) Update `CHANGELOG.md` with the new version and date.
2) Bump `Cargo.toml` version.
3) Run tests:
   ```bash
   cargo test
   ```
4) Build and smoke‑test:
   ```bash
   cargo build --release
   ./target/release/fav --help
   ```
5) Tag the release:
   ```bash
   git tag vX.Y.Z
   ```
6) Push tags:
   ```bash
   git push --tags
   ```
7) (Optional) Publish to crates.io:
   ```bash
   cargo publish
   ```
