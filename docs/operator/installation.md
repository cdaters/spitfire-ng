# Installation

## Status

- **Verified:** The prebuilt `aarch64-apple-darwin` archive was built, checksum
  verified, extracted, and used for clean setup and live client acceptance on
  Apple Silicon macOS. Locked source installation remains supported.
- **Development Preview:** The archive is not signed with an Apple Developer
  ID and is not notarized. Follow the checksum-first
  [macOS first-run procedure](macos-first-run.md).
- **Planned:** Linux, Windows, Intel macOS binaries, native installers, service
  units, signing/notarization, and an automatic release channel.

## Current distribution status

Normal operators should use the identified Development Preview archive when it
is downloaded from the
[official GitHub Release](https://github.com/cdaters/spitfire-ng/releases/tag/v0.1.0-development-preview).
It contains a prebuilt binary and does not require Git, Cargo, or repository
research fixtures. Verify and install it exactly as described in the
[Development Preview Package](development-preview-package.md).

The published archive was downloaded again from GitHub and matched canonical
SHA-256
`6c4d7ad492b1acee92481a3a577b49934c08e79822e98de50e918489a8fc9c97`.

Source builds are the developer/unsupported-host route. The code is designed
for portability, but only the Apple Silicon package has release acceptance.

## Prebuilt prerequisites

You need the `.tar.gz` archive, its adjacent `.sha256`, an Apple Silicon Mac,
and a terminal. SQLite is embedded; no separate database server is required.

## Source-build prerequisites

You need:

- Git;
- a stable Rust toolchain with Cargo (1.97.1 built the accepted preview);
- the host compiler/linker prerequisites required by Rust; and
- a terminal for the interactive setup password prompts.

SQLite is bundled by the Rust dependency; a separately managed SQLite server
is not required. Terminal clients are installed separately after the server.
The Cargo manifests do not currently declare a minimum supported Rust version,
so older compiler compatibility is not claimed.

## Developer source workflow

If you do not already have a checkout:

```bash
git clone https://github.com/cdaters/spitfire-ng.git
cd spitfire-ng
git switch main
```

Confirm the checkout before building:

```bash
git status --short --branch
git log -1 --oneline
```

### Install with Cargo

From the repository root:

```bash
cargo install --path crates/sf-bbs --locked --force
```

Cargo normally installs `spitfire` beneath its configured binary directory,
usually `$HOME/.cargo/bin`. Ensure that directory is on `PATH`, then verify:

```bash
spitfire --version
```

`--force` ensures that a previously installed preview binary with the same
0.1.0 package version is replaced by the current checkout. To keep a preview
installation isolated, select a prefix:

```bash
cargo install --path crates/sf-bbs --root ./local-install --locked --force
./local-install/bin/spitfire --version
```

If the exact dependency set is already cached and network access is
unavailable, add `--offline`. Offline mode cannot fetch missing dependencies:

```bash
cargo install --path crates/sf-bbs --locked --force --offline
```

### Build without installing

For a repository-local build:

```bash
cargo build --release -p sf-bbs
./target/release/spitfire --version
```

Use the same resolved binary consistently for setup, run, backup, and restore.
For preview upgrades, preserve the old binary/source revision with a cold
backup; see [Upgrades](upgrades.md).

### Verify the checkout

Developers and release builders should run the complete workspace gates:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

A Sysop installing an identified source revision does not need the ignored
historical research corpus to create or run a native board.

## Installation boundaries

- Do not run `init-fixture`; it is a development/test-board command, not the
  first-time operator path.
- Do not copy an existing board as a substitute for `setup`.
- Do not pipe setup answers from a file. The initial password reader requires
  a controlling terminal and intentionally disables echo.
- Do not expose Telnet, RAW, or RLogin to an untrusted network without
  understanding that they do not encrypt caller passwords or session data.
  Prefer the disabled-by-default SSH caller listener for encrypted remote
  terminal access; verify its board-local host fingerprint, firewall, and
  login-identifier policy before a non-loopback bind. SSH provides the BBS
  only, never host administration.
