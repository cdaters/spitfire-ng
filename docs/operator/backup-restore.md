# Backup and Restore

## Status

- **Verified:** Cold backup, new-root restore, replacement rollback behavior,
  and restored caller/message/file/profile persistence have passed acceptance.
- **Development Preview:** Native format-1 directory snapshots are the current
  board recovery mechanism and must be created while the board is stopped.
- **Planned:** Live snapshots, retention automation, encryption, cloud
  providers, replication, and incompatible-version conversion are not
  implemented.

## Before every operation

Stop `run`, `console`, and `shell`. Confirm:

```bash
spitfire status /path/to/board/spitfire.toml
```

The native workflow is cold by design because SQLite state and cataloged file
bytes must describe one consistent board. It is not a live snapshot service.

## Create a backup

The destination parent must exist and the destination itself must not:

```bash
spitfire backup /path/to/board/spitfire.toml /path/to/backups/board-001
```

The command validates configuration, SQLite schema/integrity/identity,
resources, catalog metadata, every cataloged byte's size/SHA-256, and the
completed manifest before publishing the directory.

The snapshot contains:

- exact static configuration;
- consistent SQLite operational state, including message payload/delivery
  identities, recipient/audience relations, tombstones, receipts,
  Copy/Forward lineage, mutation audit, caller login identifiers, handles,
  private real names, lifecycle/subscription/security state, and identity/access
  audit; public-directory policy, caller listing preferences/versions, ordered
  Other BBS state, public-resource generations, and public-information audit;
  schema-15 file lifecycle/integrity, requests/review, upload policy,
  normalized operation state, publications, and semantic file audit;
  schema-16 transfer policy, board-day usage, terminal reservation/settlement
  history, storage-root/locator authority, and transfer audit;
- complete SYSTEM (including presentation and language package descriptors,
  catalogs/assets, licenses, provenance, JOKER policy, and any generated SSH
  host key) plus DISPLAY override resources;
  and
- every cataloged file's retained managed bytes, including recoverable
  tombstones.

It excludes runtime status, incomplete upload staging, logs, uncataloged
bytes, source code, research samples, and emulator images.

## Restore to a new board

The new target must not exist:

```bash
spitfire restore /path/to/backups/board-001 /path/to/restored-board
spitfire status /path/to/restored-board/spitfire.toml
```

Start the restored board and verify one Sysop login, one message, one file
listing/download, and configuration identity before depending on it.

Current source restores exact supported schema-10 through schema-17 backups.
An older snapshot remains at its exact schema during restore and migrates
transactionally only on the first normal writable startup. Keep the old
executable and pre-upgrade backup for rollback; there is no in-place schema
downgrade.

## Replace an existing board

Use replacement only when the target is stopped, identifies the same board
and Sysop, and losing post-snapshot changes is intended:

```bash
spitfire restore /path/to/backups/board-001 /path/to/board --replace
```

Restore validates the complete snapshot before mutation, stages beside the
target, keeps a deterministic rollback directory during publication, and
restores the original target if publication fails. A pre-existing rollback
directory causes refusal instead of guessing which board is authoritative.

## Protect the snapshot

A native backup is sensitive. SQLite contains password hashes, caller contact
profiles, private messages, receipts, statistics, and operational history.
SYSTEM may contain the SSH private host key. Possession of that key can
impersonate the restored board's SSH host identity, so it requires the same
protection as the database.
Protect the whole directory with appropriate host permissions and copy it as
one unit. Do not edit the manifest or contents; any inventory/byte change is
detected.

Retention, encryption, removable/cloud copies, replication, and enterprise
backup policy belong to the host operator. The SPITFIRE command neither
implements nor claims them.

The authoritative format, validation, rollback, and exclusion contract is
[SPITFIRE NG Native Backup and Restore](../sfng-backup-restore.md).
