# SPITFIRE NG 0.1.0 Development Preview

This archive is the installable SPITFIRE NG Development Preview for the target
named in `RELEASE.toml`. It is not a stable 1.0 release. Stock SPITFIRE 3.7
Core Parity and the defined ANSI/text Operator/Caller Experience tier are
accepted; RIP, Category-B expansion, SSH, web administration, service units,
and production hardening are not included.

Read `RELEASE-NOTES.md` for the accepted scope and known limitations. This
Apple Silicon macOS build is not signed with an Apple Developer ID and is not
notarized. Verify both checksum layers before following the bounded first-run
procedure in `docs/operator/macos-first-run.md`.

## Install the executable

Verify the adjacent `.sha256` file before extraction. After extraction, either
run `bin/spitfire` in place or copy it to a directory on your `PATH`:

```sh
mkdir -p "$HOME/.local/bin"
cp bin/spitfire "$HOME/.local/bin/spitfire"
chmod 755 "$HOME/.local/bin/spitfire"
"$HOME/.local/bin/spitfire" --version
```

No Git checkout, Cargo workspace, Rust toolchain, database server, or global
resource installation is required. The executable embeds the accepted Modern,
Classic, Minimal, and en-US packages and materializes strict board-local copies
during `spitfire setup`.

## Create and run a board

```sh
mkdir -p "$HOME/Spitfire/boards" "$HOME/Spitfire/backups"
spitfire setup "$HOME/Spitfire/boards/my-board"
spitfire status "$HOME/Spitfire/boards/my-board/spitfire.toml"
spitfire run "$HOME/Spitfire/boards/my-board/spitfire.toml"
```

Keep the first listener on `127.0.0.1`; Telnet, RAW, and RLogin are plaintext.
The complete setup, operation, customization, backup/restore, and upgrade
procedures are under `docs/operator/`. Start with
`docs/operator/getting-started.md`.

## License and provenance boundary

Original SPITFIRE NG code and project-authored distributable resources are
available under MIT OR Apache-2.0, at your option. The texts are under
`licenses/SPITFIRE-NG/`. Locked third-party notices and upstream license files
are under `licenses/third-party/`.

This grant does not relicense original Buffalo Creek binaries, documentation,
DISPLAY resources, registered/private artifacts, third-party research
archives, Synchronet material, or any externally authored asset. None of that
research material is included in this archive.

This archive is unsigned. SHA-256 detects accidental or malicious byte changes
relative to the separately obtained checksum, but it does not authenticate who
published the checksum.

Ordinary bugs and reproducible operator problems should follow
`docs/operator/support.md`. Do not post passwords, private board data, tokens,
or unredacted caller information. Security vulnerabilities require the private
reporting channel identified on the official download/release page, not a
public issue.
