# SPITFIRE NG Native Backup and Restore

## Purpose and Evidence Boundary

This document is the canonical specification for the native board-state
recovery workflow completed for Category-A A-060. It explains what a snapshot
contains, how validation and replacement work, and what a returning developer
or Sysop must do to recover a board safely.

The preserved SPITFIRE 3.7 manual is the primary historical source. Section
5.8 confirms that original SPITFIRE automatically made `SFMCONF.$$$` and
`SFFAREA.$$$` copies before conference or file-area maintenance; sections 8
and 12 expose removal of those and caller-database backup files. Those facts
establish backup/recovery as part of stock operation, but not a complete
portable board-image format. SPITFIRE NG therefore preserves the recovery
outcome through a documented native snapshot while leaving legacy `.$$$` /
`.$??` compatibility and configuration-history behavior in Category-B B-025.

## Sysop Commands

Stop the board before either operation. The commands fail closed if `run`,
`shell`, `console`, `config`, another backup, or another restore currently owns
the board-wide operation lock.

```text
spitfire backup <CONFIG-FILE> <BACKUP-DIRECTORY>
spitfire restore <BACKUP-DIRECTORY> <NEW-BOARD-DIRECTORY>
spitfire restore <BACKUP-DIRECTORY> <EXISTING-BOARD-DIRECTORY> --replace
```

Both destination forms must name a directory whose parent already exists. A
new restore refuses an existing target. Replacing a board requires the
explicit `--replace` flag and requires the existing configuration to identify
the same board and Sysop as the snapshot. A backup destination must not exist
and must be outside the board and every managed logical path.

## Authoritative Contents

The format-1 snapshot is a directory headed by `spitfire-backup.toml`. The
manifest records the backup format, producing SPITFIRE NG version, creation
time, exact SQLite schema, board/Sysop identity, configuration filename, and a
sorted entry inventory with kind, portable relative path, byte length, and
lowercase SHA-256.

| Boundary | Snapshot content | Restore destination |
|---|---|---|
| Static configuration | Exact validated TOML bytes | Board root, retaining the configuration filename |
| Operational state | A transactionally consistent SQLite backup at exact current schema | Configured logical `WORK` database filename |
| SYSTEM resources | Every regular file below logical `SYSTEM`, recursively, including presentation-profile descriptors, provenance, assets, JOKER policy, and SSH host key | Same relative path below restored `SYSTEM`; configured profiles and SSH key location are revalidated before acceptance |
| DISPLAY resources | Every regular file below logical `DISPLAY`, recursively | Same relative path below restored `DISPLAY` |
| Cataloged file bytes | Every cataloged row's independently verified managed bytes, including retained recoverable tombstones | `EXTERNAL/files/<storage-key>/<filename>` |

SQLite remains authoritative for board identity; callers and Argon2id
credentials; private profiles and preferences; statistics and new-file
checkpoints; schema-12 caller lifecycle versions, authoritative base security,
subscription expiration, reasoned security adjustments, purge protection,
recoverable tombstones, and privacy-safe caller-access events; conferences,
schema-13 login identifiers, public handles, private real names, and privacy-
safe identity events; schema-14 public-directory policy, caller opt-outs and
versions, ordered Other BBS state/contributors, public-resource generations,
and privacy-safe public-information events; schema-15 file lifecycle/integrity
and versions, private requests/review, upload policy, normalized operation
state, legacy-publication generations, and semantic file events;
immutable message payload/fan-out identities,
separately numbered delivery recipients/audiences, tombstones, visibility,
receipts, last-read, Copy/Forward lineage, and privacy-safe mutation audit; and
file areas, catalog metadata, hashes, attribution, state, and accounting.
Schema 18 also preserves retained operational events, daily summaries,
versioned activity retention, operator notifications, and their separate
observability-action audit. Schema 19 additionally preserves bounded operator
command receipts and the append-only operator-control audit. It does not
preserve live operator connections, challenges, subscriptions, endpoint
files, or daemon generation.
`SYSTEM/JOKER.DAT` and the configured board-local SSH host key, when present,
are preserved as exact resource bytes. The snapshot does not duplicate that
metadata in a second model.

Transient `WORK/runtime-status.toml`, incomplete `WORK/upload-staging` bytes,
logs or other uncataloged working files, and uncataloged external bytes are
not snapshot state. The schema-18 memory-only live event ring and schema-19
operator endpoint/session state are also excluded.
Git source recovery, historical samples, research work,
emulator images, cloud copies, and external storage
providers are also outside this workflow.

## Backup Validation and Publication

Backup acquires an OS-backed exclusive lock stored beside the board root, then
performs these operations while the board is cold:

1. canonicalize and validate the real configuration file;
2. require relative, non-overlapping SYSTEM/WORK/DISPLAY/MESSAGE/EXTERNAL
   paths so the snapshot is portable and the whole restore can be staged;
3. open SQLite and require current schema 19, exact migration names,
   and no nonterminal file operation or active transfer/inspection use,
   `PRAGMA quick_check = ok`, no foreign-key violations, and configuration /
   database identity agreement;
4. use SQLite's backup API to create one consistent database copy, then apply
   the same read-only validation to the copy;
5. reject resource symlinks, special files, non-UTF-8 portable names,
   traversal, excessive inventory, and case-conflicting manifest paths;
6. enumerate every catalog row, open its bytes through `FileStorage`, and
   require the catalog size and SHA-256 before copying;
7. hash and synchronize every copied entry, write the manifest last, and
   re-read the complete directory through the restore validator; and
8. publish the staging directory under the requested nonexistent name.

Failure before publication removes the private staging directory and leaves
no claimed backup. The persistent lock file is only a coordination location;
the operating-system lock, not file existence, determines ownership.

## Restore Validation and Determinism

Restore validates the entire backup before it creates or renames any board
target. This build accepts exact schema-10 through schema-19 snapshots.
An older schema is restored unchanged; only subsequent normal writable startup
applies the transactional migrations through schema 19. Validation rejects
unknown manifest fields, an unsupported older/newer schema, unsafe or duplicate
paths, missing or undeclared files,
incorrect lengths or hashes, identity disagreement, and any mismatch between
SQLite catalog rows and declared byte entries. Restore is intentionally not a
schema migration or incompatible-version conversion path.

The validated snapshot is copied into a sibling temporary board on the same
filesystem as the target. The backed configuration resolves relative to that
new root; all logical directories are created; configuration, database,
resources, and catalog bytes are placed at their authoritative paths. Before
publication, SPITFIRE NG again validates configuration and identity, opens the
database read-only, verifies every cataloged byte through `FileStorage`, and
loads the stock menus/help/display resources.

A new restore renames the complete staging board to its nonexistent target.
For `--replace`, the stopped target is first renamed to the deterministic
hidden sibling `.<board>.spitfire-restore-rollback`; the staged board is then
renamed into place. If publication fails, the original directory is renamed
back. The completed rollback directory is removed only after the new target is
published. If a rollback directory already exists, restore refuses to proceed
so a Sysop can inspect it instead of guessing which copy is authoritative.

This yields deterministic covered state: the restored configuration, SQLite,
SYSTEM/DISPLAY trees, and cataloged file tree are exactly the validated
snapshot. Data created after the snapshot is intentionally absent after an
explicit replacement. Untrusted partial uploads and stale runtime state do not
reappear.

## Security and Operational Limits

- Treat a backup as sensitive: SQLite includes password hashes, private caller
  profiles, private messages, and operational history. Protect and copy the
  directory with access controls appropriate to the host. SYSTEM may contain
  the SSH private host key and must receive the same protection.
- Do not edit the manifest or contents. Any byte or inventory change is
  detected before restore mutation.
- Symlinks and filesystem-special objects are rejected; manifest paths cannot
  be absolute, contain `..`, or use alternate separators.
- The native workflow currently requires the relative, disjoint logical layout
  created by normal setup. Boards configured with absolute or overlapping
  logical paths must be relocated through a separately designed audited
  workflow before native backup.
- Backup is cold/offline by policy. SQLite's consistent copy primitive does
  not authorize live/hot snapshots because cataloged filesystem bytes form a
  separate authority boundary.
- The format is an unpacked directory snapshot, not a general archive,
  replication, cloud, or enterprise backup system.

## Verification

Focused tests create a board through the normal setup service, add a caller and
message, preserve generated catalog files and custom resources, exclude stale
status/incomplete staging, restore to a new board, and authenticate the
persisted caller through the common session engine. Replacement tests prove
that post-snapshot caller/resource state is removed only after validation.

Additional tests cover SQLite snapshot consistency, schema-10/11 exact restore
followed by normal migration, schema-13 identity restore followed by writable
migration, exact SSH-key/configuration preservation, schema-14 public
policy/opt-out/Other-BBS/event/resource-generation preservation, schema-15
requests/policy/lifecycle/event preservation, schema-14 exact restore then
writable migration, schema-18 event/summary/retention/notification/audit
preservation and EventId continuation, schema-17 exact restore followed by an
honest empty-history migration, safe failed-backup notification, older/newer refusal,
board-lock exclusion, missing catalog bytes, checksum corruption, manifest
traversal, undeclared files, explicit replacement, rollback cleanup, and the
Sysop CLI report. The combined workspace and existing transport,
message, file-transfer, privacy, paging, and multinode regression suites remain
the final release gate.

See also [Setup and Configuration](sfng-setup-configuration.md),
[Native File System](sfng-file-system.md), and the
[Stock SPITFIRE 3.7 Parity Checklist](stock-spitfire-3.7-parity.md).

## Schema-15 file-operation recovery boundary

M039 Tranche 5 cold backup rejects every nonterminal file-operation journal
row and active transfer/inspection use before snapshot. It preserves versioned file lifecycle/integrity,
requests/review, denial policy, semantic audit, recoverable quarantine, and
legacy listing generations/digests together with all authoritative managed
bytes. In-flight stages are not copied or declared consistent. See the
[Tranche 5 implementation report](research/m039-tranche-5-safe-file-inspection-request-maintenance-implementation.md).

## Schema-12 caller-access recovery boundary

M042 schema 12 extends the schema-10/11 restore implementation with
exact preservation and validation of caller lifecycle versions, base security,
subscription expiration, purge protection, reasoned security adjustments, and
append-only caller access events. Its confined `SYSTEM/JOKER.DAT` policy is
already within the recursive SYSTEM backup boundary and must restore byte-
exactly.

The implemented migration keeps schema-11 backups restorable at their exact
version and applies 11→12 only during later normal writable startup. Migration
failure must leave schema 11 unchanged; an older executable must refuse schema
12. New-root/replacement restore must reject orphan adjustments, invalid dates,
duplicate active adjustment kinds, broken audit links, named-Sysop invariant
violations, policy corruption, and newer versions before publication. See
[Caller Access Lifecycle and Security](sfng-caller-access.md).

## Schema-13 caller identity and SSH recovery boundary

M042.5 schema 13 preserves stable caller ID and credentials while adding the
stored login identifier, public display handle, optional private real name,
and append-only privacy-safe identity events. Recursive SYSTEM backup includes
the configured Ed25519 host private key exactly. A restored board therefore
retains both caller authentication identity and SSH host fingerprint.

Deliberate key rotation is an operator action after backup, not a restore side
effect. Moving the old key and starting the enabled listener generates a new
one; clients will correctly report a changed host fingerprint. See
[Secure SSH Caller Transport](sfng-secure-ssh-transport.md).

## Schema-14 public-information recovery boundary

M043 schema-14 state remains preserved within current schema 19. Cold backup preserves directory policy, each
caller's opt-out and publicity version, ordered Other BBS rows/lifecycle/
contributors/versions, recognized resource generations/digests, semantic
events, and authoritative bulletin/newsletter/native-thought bytes under the
existing SYSTEM/DISPLAY recursion. Restore reproduces visibility and order.
A schema-13 backup restores exactly and migrates with private defaults only on
later normal writable startup.

See [Public Information](sfng-public-information.md).

## Schema-16/17 transfer and storage recovery boundary

Cold backup preserves transfer policy, board-day usage, terminal/reviewable
history, logical roots, per-file locators, and managed bytes. It rejects an
active transfer or reservation rather than snapshotting ambiguous accounting.
Protocol sockets, frames, byte streams, and session queues are excluded.

External catalog entries and stable FileIds restore without copying external
payloads. Restored external roots begin at Unknown and require a versioned
private rebind plus confined probe; temporary absence is `StorageUnavailable`,
not file `Missing`. Schema 17 additionally preserves valid zero-byte catalog
objects and their empty SHA-256 through new-root restore. Schema-15 and
schema-16 backups restore exactly, then migrate only on normal writable
startup.

## Schema-18 observability recovery boundary

Current schema-18 cold snapshots retain operational events inside policy,
board-day summaries, versioned retention, notification state, and
observability-action audit. Saved report-publication definitions and journals
remain B-022 work and are not part of this schema slice.

The live event ring, subscriber cursors, reconstructable report caches,
rendering staging, diagnostic logs, and ordinary operator exports are not
board authority and remain excluded. Restore preserves EventId sequencing,
starts live state empty, and never fabricates downtime or pre-migration
statistics. A schema-17 backup restores exactly; normal writable startup then
performs the transactional 17-to-18 migration with empty history.

## Schema-19 operator-control recovery boundary

Schema 19 adds only the bounded operator-command receipt journal and
append-only control audit. Cold backup preserves both so a timed-out future
command can retain an accountable result after restore. Live IPC sessions,
authentication challenges, subscription cursors, endpoint objects, and UI
state are excluded. Restore always creates a new daemon generation, so a
preserved receipt cannot make a former live SessionId target valid. See
[Protected Operator Attachment](technical/operator-control.md).

## Versioned configuration recovery

B021-C configuration saves keep one complete prior validated configuration at
`spitfire.toml.previous`. This convenience copy is bounded and separate from the
cold-backup manifest. The authoritative current configuration contains its
revision and a privacy-safe receipt recovery link; existing cold backup/restore
retains it exactly with consistent SQLite. Restored startup reconciles pending
configuration evidence through the same typed authority. Never use the prior
file as an online undo or replace files beneath a running daemon. See the
[sfconfig recovery instructions](manual/sfconfig.md#secrets-and-recovery) and
[atomic configuration contract](technical/configuration.md#atomic-commit-and-recovery).


## Operator-tool recovery integration

B021-D adds no backup execution API. Shutdown remains an enrolled daemon action;
backup/restore remain the existing exclusive offline commands. Offline sfconfig
must exit before either can acquire the board lock. Maintenance/Activity and
notification views report source backup outcomes without claiming that a read
validates a snapshot. Invalid current configuration may prohibit `--replace`;
restore to a new root, inspect with offline sfconfig, and deliberately select the
recovered board instead. The [operator recovery chapter](manual/operator-recovery.md)
is the canonical practical path, including deliberate permission recovery.
