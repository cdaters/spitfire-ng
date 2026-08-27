# SPITFIRE NG 0.1.0 Development Preview

This is the first installable SPITFIRE NG Development Preview. It is a
preservation-driven, independently implemented modern BBS with accepted Stock
SPITFIRE 3.7 Core Parity and the defined ANSI/text Operator/Caller Experience
tier. It is not SPITFIRE 3.7, an original Buffalo Creek executable, or a stable
1.0 release.

## Download target

- Apple Silicon macOS
- Rust target `aarch64-apple-darwin`
- `.tar.gz` archive with adjacent SHA-256 and an internal SHA-256 manifest
- unsigned with Apple Developer ID and unnotarized

No other prebuilt platform is claimed in 0.1.0.

## Included

- clean interactive board setup and strict configuration/status diagnostics;
- Telnet, RAW TCP, and RLogin compatibility listeners, with their plaintext
  security boundary documented;
- caller registration, Argon2id authentication, privacy-aware caller profiles,
  caller/session limits, and multinode isolation;
- message conferences, private/public messages, queues, receipts, replies,
  threads, scans, and caller mail summaries;
- file areas, catalogs, new-file checks, uploads/downloads, and implemented
  ASCII/XMODEM/YMODEM/ZMODEM-family transfers;
- engine-generated, security-filtered stock menus and exact-security BBS/CLR
  display overrides;
- Modern 1.0.1, independently authored Classic SPITFIRE 1.1.1, and Minimal
  Terminal 1.0.1 presentation profiles;
- board-owned `display/` customization without editing managed profile files;
- a versioned localization foundation with complete en-US 1.0.1 catalogs;
- cold backup, restore, rollback procedures, and operator-facing renderer/
  terminal/session diagnostics; and
- a verified Moebius 1.0.29 macOS `.CLR` authoring workflow using IBM
  VGA/CP437, 16-color ANSI, iCE off, static art, Save Without Sauce Info, and
  no UTF-8 export.

## Important limitations

- Development Preview quality; production hardening and stable compatibility
  guarantees are not claimed.
- Only Apple Silicon macOS has a validated/accepted prebuilt binary.
- The binary is not Developer ID signed or notarized; verify both checksum
  layers and follow the bounded macOS first-run instructions.
- Telnet, RAW, and RLogin do not encrypt caller credentials or session data.
- RIP is not implemented and RIP bytes are never sent.
- Caller-selectable presentation profiles and production non-English language
  packs are not implemented.
- Stock Category-B expansion, QWK/LAKOTA, SMB/DOVE-Net, FidoNet/CircuitNet,
  expanded doors, SSH, web administration, and automatic service/update
  management remain future work where documented.
- SFDATE and SFREG are separate preservation/research streams and are not part
  of this package.
- SFDraw is a future companion-tool plan and is not implemented.

## Install and verify

Download the archive and its `.sha256` file from the same official release.
Verify the archive, extract it, then verify `MANIFEST.SHA256` before running:

```sh
shasum -a 256 -c \
  spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz.sha256
tar -xzf spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz
cd spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin
shasum -a 256 -c MANIFEST.SHA256
./bin/spitfire --version
```

Start with `docs/operator/getting-started.md`. macOS first-run help is in
`docs/operator/macos-first-run.md`, and support/reporting guidance is in
`docs/operator/support.md`.

## License and provenance

Original SPITFIRE NG code and project-authored distributable resources are
available under `MIT OR Apache-2.0`, at your option. Third-party dependencies
retain the terms recorded in the archive. Original Buffalo Creek binaries,
documentation, DISPLAY resources, private/registered artifacts, research
archives, and Synchronet materials are not included or relicensed.
