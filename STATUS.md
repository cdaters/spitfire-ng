# SPITFIRE NG Status

## Development Preview 0.1.0

SPITFIRE NG 0.1.0 is ready as a Development Preview. The release candidate has
passed packaging, runtime, terminal-client, backup/restore, license, and
privacy checks. Public publication is the next project step; no public binary
is available yet.

## Available today

- Stock SPITFIRE 3.7 Core Parity for the defined core scope
- ANSI/text caller and operator experience parity
- Modern, Classic SPITFIRE-inspired, and Minimal Terminal profiles
- Generated stock menus and exact-security `.BBS`/`.CLR` overrides
- Telnet, RAW TCP, and RLogin compatibility listeners
- Caller registration, authentication, privacy, profiles, and security levels
- Message conferences, mail, replies, threads, queues, and receipts
- File areas, catalogs, search, uploads, downloads, and new-file checks
- ASCII, XMODEM, YMODEM, and ZMODEM-family transfer support as documented
- Multinode runtime and session isolation
- Operator configuration, status, and renderer diagnostics
- Cold backup, restore, upgrade-preservation, and rollback procedures
- Versioned presentation and language packages with an en-US baseline
- Verified Moebius 1.0.29 `.CLR` authoring on macOS

## Current binary

| Item | Status |
|---|---|
| Version | 0.1.0 |
| Channel | Development Preview |
| Platform | Apple Silicon macOS |
| Target | `aarch64-apple-darwin` |
| Archive | `spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz` |
| SHA-256 | `6c4d7ad492b1acee92481a3a577b49934c08e79822e98de50e918489a8fc9c97` |
| Signing | No Apple Developer ID signature |
| Notarization | Not notarized |
| Publication | Pending |

Only this target has completed package and live-client acceptance. Source code
is intended to remain portable, but other prebuilt platforms are not claimed
until they are built and tested.

## Not implemented yet

- RIP graphics and RIP terminal behavior
- Caller-selectable presentation profiles
- Production non-English translations and caller locale selection
- Remaining advanced Category-B commands and resources
- QWK/LAKOTA and other offline-mail ecosystems
- SMB/DOVE-Net, FidoNet, and CircuitNet interoperability
- SSH transport
- Web administration
- SFDraw, the planned display-authoring companion tool
- SFDATE and SFREG preservation tools

These are future directions, not partially shipped features.

## Maturity and support

Development Preview means the documented workflows are usable and tested, not
that production hardening or stable 1.0 compatibility is complete. Preview
upgrades should be paired with a cold backup and the previous executable.
Telnet, RAW, and RLogin do not encrypt caller credentials or session data.

See [Support and Bug Reports](docs/operator/support.md),
[Security](SECURITY.md), and the [Roadmap](ROADMAP.md).

## Next step

Publish the accepted 0.1.0 Development Preview through the controlled release
process, then verify the public download, checksum, installation, terminal
clients, and documentation links.
