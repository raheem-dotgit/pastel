# Contributing to Pastel

Thanks for helping out! Bug reports, ideas and pull requests are all welcome.

## Reporting a bug

Open an [issue](https://github.com/raheem-dotgit/pastel/issues) with:

- your distro and desktop (for example Ubuntu 24.04, GNOME 46, Wayland)
- what you did, what you expected, and what happened
- any output from running `pastel` in a terminal

## Development setup

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev wl-clipboard
git clone https://github.com/raheem-dotgit/pastel
cd pastel
cargo run
```

## Code layout

| File | Contents |
|---|---|
| `src/main.rs` | CLI commands, single-instance check, Super+V setup |
| `src/ui.rs` | GTK window, list, preferences, clipboard capture |
| `src/backend.rs` | `wl-paste --watch` watcher and `wl-copy` |
| `src/models.rs` | History and settings, saved as JSON |
| `src/paths.rs` | File locations and the desktop entry |
| `src/format.rs` | Clip previews and relative times |

## Pull requests

1. Fork the repo and create a branch.
2. Keep changes focused; one feature or fix per PR.
3. Before pushing, run the same checks as CI:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```
4. Describe what changed and how you tested it.

## Releases

Maintainers tag a version (`git tag v1.2.0 && git push --tags`). GitHub
Actions then builds x86_64 and aarch64 binaries and attaches them to the
release, which is what `install.sh` downloads.
