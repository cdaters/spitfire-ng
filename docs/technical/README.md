# SPITFIRE NG Technical Reference

<!-- help-topic: technical.reference -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)
>
> **Latest downloadable release:** SPITFIRE NG 0.1.0 Development Preview
>
> This reference follows current source. Release-specific behavior must be
> checked against the documentation shipped with that release.

The Technical Reference is the implementation-level map for advanced Sysops,
contributors, integrators, and future maintainers. It points to the current
canonical specifications rather than duplicating them into a second body of
prose.

For task-oriented board operation, use the [Sysop Reference
Manual](../manual/README.md). For caller-facing instructions, use the [Caller
Guide](../caller-guide/README.md). Historical evidence, original-runtime
findings, and interoperability provenance remain in the research layer.

## How authority is organized

The runtime is split into a transport-neutral caller/session engine, durable
domain state in SQLite, confined project-managed bytes, versioned resource
packages, and compatibility adapters. Stable identifiers, transactions,
dispatch-time reauthorization, optimistic versions, leases, bounded parsers,
and cold recovery protect that authority across concurrent nodes and failures.

Historical SPITFIRE documentation and controlled evidence define historical
compatibility behavior. Modern comparison projects and protocol peers can
inform implementation technique, but they do not redefine SPITFIRE.

## Reference map

### Project and system architecture

- [Project Charter](../01-project-charter.md)
- [Compatibility Principles](../02-compatibility-principles.md)
- [System Architecture](../04-system-architecture.md)
- [Directory Layout](../13-directory-layout.md)
- [Development Roadmap](../../ROADMAP.md)

The Rust workspace currently contains `sf-bbs` for application/runtime
orchestration, `sf-core` for portable domain and persistence behavior,
`sf-monitor` for the read-only operator TUI, `sf-legacy` for bounded legacy
parsing, and separately scoped preservation research components. The workspace
manifest remains the package-membership authority.

### Database, schema history, and transactions

- [System Architecture](../04-system-architecture.md)
- [Native Backup and Restore](../sfng-backup-restore.md)
- [Caller Authentication and Identity](../sfng-caller-authentication.md)
- [Message System](../sfng-message-system.md)
- [File System](../sfng-file-system.md)
- [File Transfers](../sfng-file-transfers.md)

Schema history is documented in the component specifications and migration
tests that introduced each version. Native runtime startup is the migration
boundary; restore preserves an older supported snapshot exactly until normal
writable startup migrates it.

### Caller identity, access, and privacy

- [Authentication and Privacy](../11-authentication-privacy.md)
- [Caller Authentication](../sfng-caller-authentication.md)
- [Caller and Sysop Interaction](../sfng-caller-sysop-interaction.md)
- [Public Information](../sfng-public-information.md)
- [Security Philosophy](../03-security-philosophy.md)

These documents define login identifiers, public handles, private profile
data, credentials, caller lifecycle, security adjustments, subscription and
name policy, caller-directory publication, session invalidation, and
privacy-safe audit.

### Messages and conferences

- [Message Architecture](../07-message-system.md)
- [Native Message System](../sfng-message-system.md)
- [Caller/Sysop Interaction](../sfng-caller-sysop-interaction.md)

The message specifications cover conference authority, immutable shared
payloads, delivery identities, CC fan-out, receipts, visibility, tombstones,
copy/forward lineage, discovery, paging, and concurrent mutation.

### Files, transfers, and storage

- [Legacy File Formats](../06-legacy-file-formats.md)
- [Native File System](../sfng-file-system.md)
- [Transfer Runtime](../sfng-file-transfers.md)
- [Backup and Restore](../sfng-backup-restore.md)

These are the canonical sources for stable `FileId`, lifecycle and integrity,
inspection, requests and review, duplicate/policy handling, operation sagas,
protocol engines, ephemeral batch queues, reservations and settlement, daily
and ratio policy, zero-byte authority, storage roots/locators, read-only media,
staging, active-use conflicts, and recovery.

### Transports, nodes, and sessions

- [Network Architecture](../08-network-architecture.md)
- [Nodes and Events](../14-nodes-events.md)
- [Multinode Runtime](../sfng-multinode-runtime.md)
- [Secure SSH Caller Transport](../sfng-secure-ssh-transport.md)

The common session engine owns caller semantics. Telnet, RAW, RLogin, SSH,
stdio, serial, and modem adapters provide byte transport and bounded
capability information; presentation and file-transfer protocols do not gain
authorization from the carrier.

### Presentation and localization

- [Presentation Profiles](../presentation-profiles.md)
- [Classic Presentation Profile](../classic-presentation-profile.md)
- [Localization Architecture](../localization.md)
- [Cross-Project Reference Policy](../cross-project-reference-policy.md)

Character repertoire, terminal behavior, graphics protocol, and font/visual
profile are separate capability layers. Planned presentation research does not
become runtime capability until its interface, security, fallback, and client
acceptance are implemented.

### Backup, recovery, audit, and operator authority

- [Native Backup and Restore](../sfng-backup-restore.md)
- [Operator Observability](observability.md)
- [Protected Operator Attachment](operator-control.md)
- [sfmonitor Technical Architecture](sfmonitor.md)
- [Security Philosophy](../03-security-philosophy.md)
- [Multinode Runtime](../sfng-multinode-runtime.md)

Cold backup owns a validated consistent checkpoint. Online mutations remain
daemon-authoritative and use typed domain commands; maintenance and offline
operations require the ownership mode defined by the operator architecture.
Audit records outcomes and identifiers without retaining secrets, raw terminal
input, transferred content, or host paths unnecessarily.

### Operator observability and reports

- [Implemented Schema-18/B-017 Observability](observability.md)
- [B-017 Implementation and Verification](../research/m039-tranche-7-b017-observability-implementation.md)
- [Tranche 7 Operator Observability and Reports Gate](../research/m039-tranche-7-operator-observability-reports-gate.md)
- [B-021 Local/Sysop Operator Controls Gate](../research/m039-tranche-7-b021-operator-controls-gate.md)
- [B021-A Protected Operator Attachment](../research/m039-tranche-7-b021a-protected-operator-attachment.md)
- [B021-AW Windows Operator Attachment](../research/m039-tranche-7-b021aw-windows-operator-attachment.md)
- [sfmonitor 0.1 Implementation](../research/m039-sfmonitor-read-only-mvp.md)
- [System Architecture operator boundary](../04-system-architecture.md)
- [Nodes and Events](../14-nodes-events.md)

Schema 18 and B-017 implement retained operational events, daily
summaries, retention, notifications, privacy-safe maintenance/status views,
and bounded daemon-authoritative read APIs. Schema 19/B021-A transports those
views through protected Unix sockets and Windows named pipes plus a
noninteractive operator client. The protocol uses OS-backed local identity,
version/feature negotiation, distinct capabilities, daemon generation, and
dispatch-time authorization. `sfmonitor` 0.1 now presents those same reads in
a responsive local TUI. Screen/export formats, atomic publication, live
control, and `sfconfig` remain future work rather than alternate data owners.

### Compatibility adapters and historical formats

- [Compatibility Principles](../02-compatibility-principles.md)
- [Compatibility Matrix](../05-compatibility-matrix.md)
- [Legacy File Formats](../06-legacy-file-formats.md)
- [Historical SPITFIRE](../HISTORICAL-SPITFIRE.md)
- [Stock SPITFIRE 3.7 Parity](../stock-spitfire-3.7-parity.md)

Native semantic state remains distinct from legacy import, export, and
publication. Parsers bounds-check every read, preserve unknown bytes when
required, avoid unsafe structure casts, and use synthetic public fixtures.

### Protocol and interoperability evidence

- [Transfer Runtime](../sfng-file-transfers.md)
- [Tranche 6 Transfer/Storage Contract](../research/m039-tranche-6-batch-transfer-policy-extended-storage-gate.md)
- [Tranche 6 Verification](../research/m039-tranche-6-verification.md)

The runtime implements ASCII, XMODEM Checksum, XMODEM CRC, 1K-XMODEM,
1K-XMODEM-g, YMODEM Batch, YMODEM-g Batch, ZMODEM Batch, and TeLink behind one
transport-neutral engine boundary. Interoperability reports are evidence, not
bundled peer software.

### Extension and future-component boundaries

- [Door Runtime Architecture](../12-doors-runtime.md)
- [Network Architecture](../08-network-architecture.md)
- [CircuitNet](../09-circuitnet.md)
- [Web Terminal](../10-web-terminal.md)
- [SFDraw](../sfdraw.md)

These documents describe boundaries and future work. A specification in this
section does not, by itself, mean the component is implemented.

## Technical documentation rules

- Link to one canonical specification rather than cloning its contents.
- State whether a claim is confirmed, inferred, modernized, or unresolved.
- Keep historical evidence separate from modern engineering comparisons.
- Document security, bounds, concurrency, failure, and recovery behavior with
  every state-changing interface.
- Keep public technical documents free of private sample paths, credentials,
  caller identities, screenshots, and proprietary bytes.
- Update the Sysop Manual or Caller Guide when an internal change alters what
  those readers must do or will observe.

## Existing-document migration

Current top-level specifications remain canonical. Future work may move or
summarize them under this directory one subject at a time, but only with link
updates and an explicit replacement pointer. Research records and private
continuity are not folded into this reference merely to simplify the tree.

The governing documentation and source-header rules are in [Documentation
Architecture and Source-Header Policy](../documentation-policy.md).
