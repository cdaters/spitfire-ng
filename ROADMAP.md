# SPITFIRE NG Roadmap

SPITFIRE NG is moving from historical research and stock-core implementation
into public Development Preview releases. This roadmap describes major
outcomes rather than internal checkpoint chronology.

## Completed foundations

### Preservation model

- Defined the boundary between SPITFIRE identity and obsolete DOS limitations.
- Established safe legacy parsing rules, CP437 preservation, and provenance
  requirements.
- Classified the stock SPITFIRE 3.7 command and caller-experience scope.

### Stock core

- Implemented board setup, configuration, callers, security, nodes, messages,
  files, transfers, and operator control.
- Achieved the defined Stock SPITFIRE 3.7 Core Parity scope.
- Achieved the defined ANSI/text Operator/Caller Experience Parity scope.
- Added generated stock menus, exact-security display overrides, live caller
  context, and security-aware fallback.
- Added post-0.1.0 stock-style Specific Caller and Text Search with bounded,
  read-only, visibility-filtered results. Text Search keeps SPITFIRE's command
  flow while intentionally modernizing the historical phrase limitation with
  whitespace-delimited all-term matching.
- Added post-0.1.0 CC delivery fan-out and auditable message mutation:
  separately numbered primary/CC deliveries, per-delivery receipts and
  tombstones, authorized Delete/Undelete and audience transitions,
  source-retaining Copy/Forward, schema-11 lineage/audit, stale-conflict
  protection, and schema-10/schema-11 recovery.
- Added post-0.1.0 auditable caller access: Active/Locked Out/recoverable
  Deleted lifecycle, disable/restore, purge protection, base/effective
  security, board-local subscription warning/expiry/renewal, bounded JOKER
  name denial, named-Sysop invariants, active-session invalidation,
  dispatch-time reauthorization, privacy-safe audit, stale-conflict handling,
  schema-12 persistence, and schema-10/schema-11/schema-12 recovery.

### Presentation and language

- Added Modern, Classic SPITFIRE-inspired, and Minimal Terminal profiles.
- Added board-owned `display/` overrides without weakening engine command or
  security authority.
- Added a versioned language-package contract and complete en-US baseline.
- Verified Moebius 1.0.29 for bounded macOS `.CLR` authoring.

### Operator readiness

- Added prebuilt Development Preview packaging for Apple Silicon macOS.
- Validated clean setup, Qodem, SyncTERM, RAW Text, status, shutdown,
  backup/restore, and upgrade preservation.
- Added checksums, release metadata, dependency notices, and public operator
  documentation.

## Current release stage

SPITFIRE NG 0.1.0 Development Preview is published for Apple Silicon macOS as
tag `v0.1.0-development-preview`. The public-redownload checksum and bounded
unsigned/unnotarized macOS first-run workflow have passed.

Public source `main` now includes post-0.1.0 message-discovery, auditable
message-mutation, and auditable caller-access improvements. No new binary, tag,
or release has been published; the downloadable binary remains the accepted
0.1.0 Development Preview and does not contain those later source changes.

Near-term release work includes:

- collect sanitized operator feedback and reproducible bug reports;
- decide the next tested binary platforms;
- add signing/notarization when suitable release credentials and process are
  available; and
- improve preview upgrade guidance as real public upgrades occur.

## Future SPITFIRE compatibility

The remaining advanced compatibility work is deliberately separate from the
accepted stock-core tier:

- advanced display/resource types, including RIP;
- questionnaires, bulletins, ratios, batch workflows, and deeper maintenance
  controls;
- QWK/LAKOTA offline mail;
- SMB/DOVE-Net, FidoNet, and CircuitNet interoperability;
- expanded doors and external-program support; and
- deeper events, maintenance, and multinode administration.

Each area needs a documented interface and evidence-based acceptance scope
before implementation.

## Modern improvements

Longer-term modern SPITFIRE NG work may include:

- SSH transport;
- web administration;
- production language packs and community translation workflow;
- caller-selectable installed presentation profiles;
- additional rights-clean community presentation packages;
- broader packaging, service integration, and automatic update support; and
- modern federation and interoperability where it fits SPITFIRE's operating
  model.

Modern work must not weaken privacy, security, data preservation, or command
authority.

## SFDraw

[SFDraw](docs/sfdraw.md) is a planned Rust-based, cross-platform companion
editor for SPITFIRE-compatible `.CLR` and `.BBS` resources. Its goals include
long-form ANSI canvases, classic 80-column presets, VGA/CP437 fidelity,
byte-aware saving, board-local override integration, and optional baud-rate
playback. RIP editing remains deferred until SPITFIRE NG itself supports and
validates RIP.

SFDraw is a future project; no implementation has begun.

## Separate preservation work

SFDATE and SFREG remain private preservation/research streams rather than
features of the 0.1.0 Development Preview. They are not included in this
public source snapshot or release package.

## Development rule

Historical compatibility claims require evidence. Legacy input remains
read-only until its format is understood, and proprietary historical material
is never redistributed merely because it was useful during research.

For current capabilities see [Status](STATUS.md). For detailed implemented
behavior see the [documentation index](docs/README.md) and
[parity checklist](docs/stock-spitfire-3.7-parity.md).
