# Compatibility

## Platforms

CI runs tests on macOS, Linux, and Windows. Homebrew builds from source on macOS/Linux
with Rust as a build dependency. Windows users install from source with Cargo.

Open uses open on macOS, xdg-open on Linux, and explorer.exe on Windows. Linux needs
an installed desktop opener; headless users can use resolve/with. Unsupported operating
systems return a clear opener error.

## Shells and Paths

Generated helpers support bash, zsh, and fish. PowerShell users can invoke fav directly;
PowerShell helpers are not included. Picking requires fzf; the TUI requires a terminal.

Pass paths with spaces/Unicode as one argument. Use -- before positional paths beginning
with a hyphen, or pass an absolute path:

```bash
fav ensure -- ./-leading-hyphen
fav ensure './notes with spaces.txt'
fav resolve project --json
```

Adding canonicalizes paths and resolves symlinks. Ensure treats links/targets as the
same favorite. Relative paths resolve against the working directory. Tilde/environment
variables expand in config/favorite paths. Non-UTF-8 paths currently use lossy storage;
avoid them.

## Output and Store Compatibility

Default path/human output and list --format json remain compatible with 0.2.0. New
--json uses a versioned envelope. Normal command execution retains child status; other
errors use documented categories. The store remains version 2; old version-1 stores
receive default notes/presets when loaded. No new migration is needed for 0.3.0.
