# SPITFIRE NG Documentation

Choose the section that matches what you want to do. You do not need to read
the historical or architectural material to set up a board.

## Getting started

- [Development Preview Package](operator/development-preview-package.md) —
  Verify, extract, install, upgrade, and roll back the prebuilt package.
- [macOS First Run](operator/macos-first-run.md) — Handle Gatekeeper safely for
  the unsigned and unnotarized preview.
- [Getting Started](operator/getting-started.md) — Create a board, connect,
  exercise messages and files, and make a cold backup.
- [Installation](operator/installation.md) — Prebuilt and source-build paths.
- [Troubleshooting](operator/troubleshooting.md) — Common startup, listener,
  terminal, transfer, and backup problems.

## Operator guide

- [Operator Documentation](operator/README.md)
- [Configuration](operator/configuration.md)
- [Sysop Guide](operator/sysop-guide.md)
- [Caller Management](operator/caller-management.md)
- [Messages](operator/messages.md)
- [Files](operator/files.md)
- [File Transfers](operator/transfers.md)
- [Terminal Clients](operator/terminal-clients.md)
- [Backup and Restore](operator/backup-restore.md)
- [Upgrades](operator/upgrades.md)
- [Support and Bug Reports](operator/support.md)

## Presentation and custom screens

- [Presentation Profiles](presentation-profiles.md) — Modern, Classic,
  Minimal, package validation, resource precedence, and fallback.
- [Classic SPITFIRE-Inspired Presentation](classic-presentation-profile.md) —
  Design, identity, provenance, and behavior boundaries.
- [Classic Operator Guide](operator/classic-presentation.md) — Select and test
  Classic on a board.
- [Customizing Display Screens](operator/custom-display-screens.md) — Author
  board-local `.CLR` and `.BBS` resources, including the verified Moebius
  1.0.29 workflow.
- [SFDraw](sfdraw.md) — Planned cross-platform SPITFIRE display editor.

## Language and localization

- [Localization Contract](localization.md) — Language-package format,
  semantic keys, fallback, encoding, validation, and security.
- [Language Packages](operator/localization.md) — Install, select, diagnose,
  back up, and restore a language package.

## Architecture

- [Project Charter](01-project-charter.md)
- [Compatibility Principles](02-compatibility-principles.md)
- [System Architecture](04-system-architecture.md)
- [Compatibility Matrix](05-compatibility-matrix.md)
- [Legacy Data and File Formats](06-legacy-file-formats.md)
- [Message System](07-message-system.md)
- [Directory Layout](13-directory-layout.md)
- [Nodes and Events](14-nodes-events.md)
- [Native Setup and Configuration](sfng-setup-configuration.md)
- [Native Caller and Authentication](sfng-caller-authentication.md)
- [Secure SSH Caller Transport](sfng-secure-ssh-transport.md) — Current-source
  caller identity, authentication, no-shell, host-key, configuration, PTY,
  encoding, diagnostics, client, and recovery contract.
- [Public Information](sfng-public-information.md) — Current-source schema-14
  caller-directory/privacy, locate, Other BBS, bulletin/newsletter/system-info,
  native-thought, authorization, audit, and recovery contract.
- [Caller Access Lifecycle and Security](sfng-caller-access.md) — Active,
  Locked Out, and recoverable Deleted states; base/effective security;
  subscription and JOKER policy; named-Sysop protection; audit, concurrency,
  schema-12 migration, and recovery.
- [Native Caller/Sysop Interaction](sfng-caller-sysop-interaction.md)
- [Native Message System](sfng-message-system.md) — Conferences, queues,
  caller/text discovery, immutable payloads, primary/CC delivery identities,
  visibility, receipts, tombstones, Copy/Forward lineage, mutation audit, and
  the future network-adapter boundary.
- [Native File System](sfng-file-system.md)
- [Tranche 5 Implementation](research/m039-tranche-5-safe-file-inspection-request-maintenance-implementation.md) —
  Schema 15, safe inspection, requests/review, staged maintenance, recovery,
  and exact compatibility boundaries.
- [Tranche 5 Verification](research/m039-tranche-5-verification.md) — B-013
  verification and the exact remaining B-015/B-012 acceptance items.
- [Native File Transfers](sfng-file-transfers.md)
- [Native Multinode Runtime](sfng-multinode-runtime.md)
- [Native Backup and Restore](sfng-backup-restore.md)

## Compatibility and historical SPITFIRE

- [Historical SPITFIRE Overview](HISTORICAL-SPITFIRE.md)
- [Stock SPITFIRE 3.7 Parity Checklist](stock-spitfire-3.7-parity.md)
- [Synchronet Engineering Reference](research/synchronet-reference.md) —
  Comparison only; Synchronet is not SPITFIRE's behavioral authority.
- [SyncTERM RLogin Interoperability](research/syncterm-rlogin-autologin.md)
- [Historical THOUGHTS.BBS Format](research/historical-thoughts-bbs-format.md) —
  Rights-safe fixed-record format facts, parser safety rules, and unresolved
  runtime questions; no historical content is redistributed.

The public repository contains independently written findings, not the
historical software corpus. Visit
[Original SPITFIRE Software & Documentation](https://spitfirebbs.com/) for
legal preservation downloads and original manuals.

## Development and contributing

- [Contributing](../CONTRIBUTING.md) — Build, test, compatibility, provenance,
  issue, and pull-request expectations.
- [Licensing and Provenance](licensing-and-provenance.md) — What the project
  license covers and what remains external.
- [Security Policy](../SECURITY.md)
- [Status](../STATUS.md)
- [Roadmap](../ROADMAP.md)

## Release material

- [0.1.0 Development Preview Release Notes](../release/RELEASE-NOTES-0.1.0-DEVELOPMENT-PREVIEW.md)
- [0.1.0 Release-Candidate Manifest](../release/RELEASE-CANDIDATE-MANIFEST.md)

The release notes and manifest identify the accepted artifact. The repository
may continue improving its public documentation without changing those
already-validated package bytes.

Current `main` includes post-0.1.0 source improvements. The release documents
continue to describe the unchanged 0.1.0 Development Preview binary.
