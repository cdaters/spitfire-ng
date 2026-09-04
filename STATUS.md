# SPITFIRE NG Status

## Development Preview 0.1.0

SPITFIRE NG 0.1.0 Development Preview is publicly available. The published
Apple Silicon macOS release passed packaging, runtime, terminal-client,
backup/restore, license, privacy, public-redownload, checksum, and first-run
checks.

The public `main` branch is now ahead of that binary release. Current source
adds post-0.1.0 advanced message discovery, auditable message mutation, an
auditable caller-access lifecycle, schema-13 caller identity, secure SSH caller
transport, schema-14 privacy-bounded public information, schema-15 file
inspection/maintenance, schema-16 transfer policy and storage, and the
schema-17 zero-byte invariant. Schema 18 adds privacy-safe operational events,
daily summaries, retention, notifications, and status/maintenance projections;
the downloadable 0.1.0 archive is unchanged and does not contain those
additions.

## Current source additions

The Category-B ledger now records 14 VERIFIED, 2 IMPLEMENTED, 4 PARTIAL, and
5 NOT STARTED rows. B-024, B-011, B-014, and B-023 are VERIFIED, and M039
Tranche 6 is semantically closed. B-017 is VERIFIED; B-021 remains PARTIAL and
B-022 remains NOT STARTED.

- Stock-style Message-menu Specific Caller and Text Search
- Current, all, or queued conference scope
- Existing message visibility/security reused for every search and result open
- Read-only discovery with no last-read or receipt mutation
- ASCII-case-insensitive body substring matching; Subject is not searched
- All whitespace-delimited terms required regardless of order or separation
- One primary recipient plus up to nine separately numbered CC deliveries
- Per-delivery receipt, public/private visibility, and deleted/tombstone state
- Authorized caller deletion plus threshold-Sysop Delete, contextual
  Undelete, and public/private audience transitions
- Source-retaining same/cross-conference Copy and Forward through Copy with a
  changed recipient
- Schema-11 immutable payloads, delivery identities, Copy/Forward lineage,
  privacy-safe mutation audit, and state-version conflict protection
- Schema-10→11 migration plus schema-10/schema-11 cold backup and restore
- Active, Disabled/Locked Out, and recoverable Deleted caller lifecycle
- Disable/enable, tombstone/restore, and persisted purge protection without a
  physical purge
- Operator-assigned base security plus derived effective security and
  reasoned adjustments
- Board-local nullable subscription expiration, warning window, post-date
  expiry, and renewal that restores current base security where appropriate
- Bounded `JOKER.DAT` complete-name and `@` substring policy with generic,
  privacy-safe denials
- Named-Sysop identity protection separate from threshold privilege
- Active-session invalidation and lifecycle/security reauthorization at every
  main, message, and file command dispatch
- Append-only privacy-safe caller-access audit and optimistic state-version
  conflict handling
- Transactional schema-11→12 migration plus exact schema-10/schema-11/
  schema-12 cold backup and restore
- Schema-13 separation of stable caller ID, normalized login identifier,
  public handle, and optional private real name without rewriting historical
  attribution or merging migration collisions
- Disabled-by-default SSH-2 caller transport through the common node/session
  engine, using ordinary SQLite/Argon2id caller authority and no second BBS
  password prompt
- Board-local Ed25519 host identity, modern `russh` defaults, bounded
  authentication resources, PTY/resize/encoding propagation, lifecycle
  invalidation, privacy-safe diagnostics, and cold-backup continuity
- No OS shell, Unix account login, SCP, SFTP, command execution, forwarding,
  remote filesystem, X11, agent, or subsystem access
- Transactional schema-13→14 migration with private defaults, rollback, and
  exact schema-13 restore followed by writable migration
- Board-disabled and caller-opt-in public directory using only Active callers'
  public handles, plus policy-controlled board-local last-call/location fields
- Bounded handle-only partial locate with deterministic ordering, a 50-result
  cap, sequential confirmation, and visibility recheck before disclosure
- Native ordered/versioned Other BBS authority with stable IDs, lifecycle,
  optional stable contributor identity, conflict-safe operator maintenance,
  and caller additions disabled by default
- Board-owned numbered bulletins, newsletter, safe system facts, and bounded
  project-native `THOUGHTS.NG` through the shared resource, presentation,
  localization, paging, and encoding boundaries
- Privacy-safe semantic audit and cold recovery for directory policy,
  publicity state, Other BBS rows, resource generations, and board resources
- Transactional schema-14→15 migration preserving callers, messages, files,
  transfers, SSH, and public-information authority without fabricated file
  requests, review rows, or operation journals
- Stable file IDs with separate Active/Offline/PendingReview/Disabled/
  Tombstoned lifecycle and Unknown/Present/Missing/DigestMismatch integrity
- Confined, bounded, sanitized text inspection and metadata-only Stored/
  Deflated ZIP inspection with no extraction or execution
- Bounded FILE_ID.DIZ discovery and explicit versioned review before any
  authoritative description replacement
- Preview-area inspection separated from upload/download authority
- Private versioned Offline/Missing requests, PendingReview uploads, duplicate
  warnings, native SFNOUP denial rules, and optional description normalization
- Typed, versioned staged file maintenance with leases, name reservations,
  semantic audit, crash reconciliation, recoverable tombstones, and legacy
  SFFILES publication while SQLite plus confined managed bytes remain native
  authority
- Schema-15 cold backup and restore, including safe rejection of nonterminal
  operations and exact schema-14 restore followed by writable migration
- Transactional schema-15→16 transfer policy, board-day usage, atomic quota
  reservation, idempotent settlement, transfer history, storage roots, and
  per-file locators
- Session-ephemeral stable-FileId queues, multi-file YMODEM/YMODEM-g/ZMODEM,
  cancellation, per-item reauthorization, and multinode-safe accounting
- Native file-count/byte ratios, DAILYLMT policy, no-charge accounting,
  Preview denial before negotiation, and capped completed-upload time credit
- Managed and external read-only storage, versioned rebind/probe,
  StorageUnavailable distinct from Missing, active-use conflicts, and bounded
  seekable large-source streaming
- All nine B-024 choices: ASCII, XMODEM Checksum, XMODEM CRC, 1K-XMODEM,
  1K-XMODEM-g, YMODEM Batch, YMODEM-g Batch, ZMODEM Batch, and TeLink
- Transactional schema-16→17 migration with rollback and exact zero-byte
  catalog, upload, protocol, backup, and restore support

The all-term behavior intentionally improves the historical contiguous-phrase
limitation without changing SPITFIRE's Text Search command flow, conference
selection, visibility, or result presentation.

## Available today

- Stock SPITFIRE 3.7 Core Parity for the defined core scope
- ANSI/text caller and operator experience parity
- Modern, Classic SPITFIRE-inspired, and Minimal Terminal profiles
- Generated stock menus and exact-security `.BBS`/`.CLR` overrides
- Telnet, RAW TCP, and RLogin compatibility listeners
- Secure SSH caller transport when built from current post-0.1.0 source
- Caller registration, authentication, privacy, profiles, and security levels
- Message conferences, mail, replies, threads, queues, and receipts
- Advanced caller/text discovery and auditable message mutation when built
  from current post-0.1.0 source
- Auditable caller lifecycle, base/effective security, subscription policy,
  JOKER name denial, named-Sysop protection, and schema-12 recovery when built
  from current post-0.1.0 source
- Privacy-bounded caller directory, locate, Other BBS, bulletins, newsletter,
  system information, and native thoughts when built from current post-0.1.0
  source
- File areas, catalogs, search, uploads, downloads, and new-file checks
- Schema-15 bounded text/ZIP inspection, Preview inspection, private requests,
  PendingReview, and staged maintenance when built from current post-0.1.0
  source. B-013 is VERIFIED; B-015 and B-012 remain IMPLEMENTED.
- Schema-16/17 transfer, batch-policy, accounting, and extended-storage source.
  B-024, B-011, B-014, and B-023 are VERIFIED.
- Schema-18 privacy-bounded operational events, board-day statistics,
  retention, notifications, board/node status, recent caller and activity
  projections, and maintenance/error views. B-017 is VERIFIED.
- ASCII, XMODEM, YMODEM, ZMODEM, and TeLink transfer support as documented
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
| Tag | `v0.1.0-development-preview` |
| Platform | Apple Silicon macOS |
| Target | `aarch64-apple-darwin` |
| Archive | `spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz` |
| SHA-256 | `6c4d7ad492b1acee92481a3a577b49934c08e79822e98de50e918489a8fc9c97` |
| Signing | No Apple Developer ID signature |
| Notarization | Not notarized |
| Publication | [Published on GitHub](https://github.com/cdaters/spitfire-ng/releases/tag/v0.1.0-development-preview) |

This table describes the downloadable binary release, not every later source
change on `main`.

The archive was downloaded again from the public GitHub Release and matched
the canonical SHA-256. Expected unsigned/unnotarized Gatekeeper behavior was
observed on Apple Silicon macOS; **System Settings → Privacy & Security → Open
Anyway** succeeded, after which `spitfire --version` returned:

```text
SPITFIRE NG Bulletin Board System 0.1.0
```

Apple Developer ID signing and notarization are intentionally deferred and do
not block this Development Preview.

Only this target has completed package and live-client acceptance. Source code
is intended to remain portable, but other prebuilt platforms are not claimed
until they are built and tested.

## Not implemented yet

- RIP graphics and RIP terminal behavior
- Caller-selectable presentation profiles
- Production non-English translations and caller locale selection
- Remaining advanced Category-B commands and resources
- QWK, DOVE-Net, and FidoNet networking
- CircuitNet adapter/revival work beyond preserved compatibility knowledge
- `sfmonitor` and comprehensive `sfconfig` operator clients
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

Preserve the accepted 0.1.0 release boundary. Schema 18 and B-017 are available
in current source, while B-021 remains PARTIAL and B-022 remains NOT STARTED.
The next separate work item is the B-021 Local/Sysop Operator Controls
architecture/interface gate; it has not begun in this synchronization.
