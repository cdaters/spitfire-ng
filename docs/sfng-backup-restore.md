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
| SYSTEM resources | Every regular file below logical `SYSTEM`, recursively, including presentation-profile descriptors, provenance, and assets | Same relative path below restored `SYSTEM`; configured profiles are revalidated before acceptance |
| DISPLAY resources | Every regular file below logical `DISPLAY`, recursively | Same relative path below restored `DISPLAY` |
| Cataloged file bytes | Every available or disabled catalog row's independently verified bytes | `EXTERNAL/files/<storage-key>/<filename>` |

SQLite remains authoritative for board identity; callers and Argon2id
credentials; private profiles and preferences; statistics and new-file
checkpoints; schema-12 caller lifecycle versions, authoritative base security,
subscription expiration, reasoned security adjustments, purge protection,
recoverable tombstones, and privacy-safe caller-access events; conferences,
immutable message payload/fan-out identities,
separately numbered delivery recipients/audiences, tombstones, visibility,
receipts, last-read, Copy/Forward lineage, and privacy-safe mutation audit; and
file areas, catalog metadata, hashes, attribution, state, and accounting.
`SYSTEM/JOKER.DAT`, when present, is preserved as exact resource bytes. The
snapshot does not duplicate that metadata in a second model.

Transient `WORK/runtime-status.toml`, incomplete `WORK/upload-staging` bytes,
logs or other uncataloged working files, and uncataloged external bytes are
not snapshot state. Git source recovery, historical samples, research work,
emulator images, cloud copies, retention policy, and external storage
providers are also outside this workflow.

## Backup Validation and Publication

Backup acquires an OS-backed exclusive lock stored beside the board root, then
performs these operations while the board is cold:

1. canonicalize and validate the real configuration file;
2. require relative, non-overlapping SYSTEM/WORK/DISPLAY/MESSAGE/EXTERNAL
   paths so the snapshot is portable and the whole restore can be staged;
3. open SQLite read-only and require current schema 12, exact migration names,
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
target. This build accepts exact schema-10, schema-11, and schema-12 snapshots.
An older schema is restored unchanged; only subsequent normal writable startup
applies the transactional 10→11 and/or 11→12 migrations. Validation rejects
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
  directory with access controls appropriate to the host.
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

Additional tests cover SQLite snapshot consistency, schema-10 exact restore
followed by normal migration, schema-11 exact restore, older/newer refusal,
board-lock exclusion, missing catalog bytes, checksum corruption, manifest
traversal, undeclared files, explicit replacement, rollback cleanup, and the
Sysop CLI report. The combined workspace and existing transport,
message, file-transfer, privacy, paging, and multinode regression suites remain
the final release gate.

See also [Setup and Configuration](sfng-setup-configuration.md),
[Native File System](sfng-file-system.md), and the
[Stock SPITFIRE 3.7 Parity Checklist](stock-spitfire-3.7-parity.md).

## Schema-12 caller-access recovery boundary

Schema 12 extends the schema-10/11 restore implementation with
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
[Caller Access Lifecycle and Security](sfng-caller-access.md) for the public
caller-access model.
