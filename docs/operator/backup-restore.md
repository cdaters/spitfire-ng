# Backup and Restore

## Managed installations (D1)

After [adoption](upgrades.md), use the installed launcher:

```sh
spitfire backup
spitfire backup list
spitfire restore BACKUP-ID --replace
```

Backup creates a verified durable recovery snapshot and reports copied managed
payloads and excluded external bytes. List re-verifies recorded backups and shows
source version, schema, type, creation time and stored content size. Updates create
pre-upgrade recovery checkpoints automatically. Nothing is automatically pruned.

**Restore discards ordinary changes made after the snapshot.** `--replace` is
required explicitly. Restore validates integrity and compatibility and applies the
existing CircuitNET catalog and FTN custody protections below. It never means
runtime rollback: [runtime rollback](upgrades.md#runtime-rollback-preserves-current-data)
preserves current board state and changes only the compatible executable.

[Storage policy](deployment-storage.md) explains managed/external payloads,
capacity limits, omissions and separately retained runtime binaries. A managed
restore requires the same installation's verified history and a compatible active
runtime. Whole-host reconstruction is not an installer feature in D1.

## Existing native directory workflow

- **Verified:** Cold backup, new-root restore, replacement rollback behavior,
  and restored caller/message/file/profile persistence have passed acceptance.
- **Development Preview:** Native format-1 directory snapshots are the current
  board recovery mechanism and must be created while the board is stopped.
- **Planned:** Live snapshots, retention automation, encryption, cloud
  providers, replication, and incompatible-version conversion are not
  implemented.

## Before every operation

Gracefully stop `run`, `console`, or `shell`, and quit offline sfconfig and
other cold-board tools. Confirm:

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
resources, catalog metadata, cataloged board-managed payload sizes/SHA-256
hashes and native content objects, and the completed manifest before publishing the directory.

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
- durable SYSTEM resources (including presentation and language package descriptors,
  catalogs/assets, licenses, provenance, JOKER policy, and any generated SSH
  host key) plus DISPLAY override resources; transient QWK/FTN handoff
  candidates are excluded;
  and
- cataloged board-managed file bytes, including recoverable tombstones, and
  native content-addressed payloads. External storage roots retain metadata
  and locators only; their payload bytes are neither read nor copied.
  The managed D1 metadata-only policy also omits managed payload copies, but
  still reads and validates their content; see [storage policy](deployment-storage.md).

It excludes executables, runtime status, incomplete upload staging, logs,
external-media payloads, uncataloged bytes, source code, research samples, and
emulator images. Keep compatible runtime binaries and external media separately.
After restore, external roots require verification before being marked available.

## Restore to a new board

The new target must not exist:

```bash
spitfire restore /path/to/backups/board-001 /path/to/restored-board
spitfire status /path/to/restored-board/spitfire.toml
```

Start the restored board and verify one Sysop login, one message, one file
listing/download, and configuration identity before depending on it.

Current source restores supported schema-10 through schema-36 backups.
An older snapshot remains at its schema during restore and migrates
transactionally only on the first normal writable startup. Keep compatible
executables and backups for disaster recovery; there is no in-place schema
downgrade. Restore applies network/Events recovery and external-storage
normalization, so the restored database is not necessarily byte-identical.

## Replace an existing board

Use replacement only when the target is stopped, identifies the same board
and Sysop, and losing post-snapshot changes is intended:

```bash
spitfire restore /path/to/backups/board-001 /path/to/board --replace
```

Restore validates the complete snapshot before mutation, stages beside the
target, keeps a deterministic rollback directory during publication, and
attempts to put the original target back if publication fails. That recovery
can also fail; see the interruption runbook below. A pre-existing rollback
directory causes refusal instead of guessing which board is authoritative.

Replacement also refuses a snapshot that discards a newer known signed
CircuitNET catalog revision. It retains proven later FTN serial and peer
acknowledgement evidence. Stop on refusal; restoring to a new directory to
bypass these protections is not a safe rollback procedure. The temporary
replacement rollback directory is removed on success, not retained as a
versioned installation restore point.

## Interrupted manual restore: keep the board stopped

**Automatic pre-commit update recovery is not manual restore recovery. Runtime
rollback changes an executable; it cannot resolve a partly published restore.**
Do not repeat restore, start either board candidate, delete a rollback/staging
directory, rename directories, edit the management journal, or remove the board
locator to bypass a refusal. D1 has no supported command that deterministically
finishes or reverses an ambiguous manual restore after a crash or power loss.

1. Keep the target and every surviving copy stopped, including service restart
   supervision and offline writers. Preserve the backup, installation control
   state, target, sibling directories, and the original command/error output as
   recovery evidence. Do not publish that material; it can contain private data.
2. Record the exact target and backup from the attempted command. Native restore
   stages in a sibling named `.spitfire-restore-*`. Replacement moves the prior
   target to the exact sibling `.<target-name>.spitfire-restore-rollback` before
   publishing the staged board. Managed restore also uses temporary material
   under `INSTALL/transactions/.manual-restore-*`. These are implementation
   artifacts, not independently identified rollback points.
3. For an adopted installation, `INSTALL/bin/spitfire version` reads management
   state and reports `Recovery pending: ManualRestore` when that record survives.
   Save its output or error. It does not validate the board or prove that restore
   completed. Neither `update --recover` nor `deployment recover INSTALL` can
   resolve `ManualRestore`; they refuse automatic recovery of that phase.
4. Interpret the surviving state using the cases below. A directory name,
   timestamp, absent staging directory, or successful `version` is not proof of
   board authority or restore completion. Do not use `spitfire status` to inspect
   uncertain candidates: it opens SQLite through the writable API, rather than
   a read-only recovery validator. D1 provides no general read-only
   candidate-validation CLI.

| Observed result | Meaning and safe action |
|---|---|
| Command returned success and no pending managed restore remains | Publication completed, including removal of the prior replacement directory. Use the normal post-restore checks below before returning to service. |
| Error explicitly before target mutation (for example invalid backup, incompatible evidence or existing rollback directory) | The attempted restore did not replace the target. A pre-existing rollback directory still requires investigation of the earlier attempt; its presence is not permission to resume. |
| `publish replacement board` error with successful automatic rename-back | The native function returned the original directory to the target. For managed restore, the wrapper clears its record only after all inventoried prior file hashes match. Confirm this error path and no pending operation before normal checks; do not infer it from a generic failure. |
| `prepare restore rollback` error | The first rename failed; staged material may remain. Confirm the original target survived unchanged and exclude any earlier unresolved restore before normal checks. If not provable, stop for intervention. |
| `remove completed restore rollback` error | The staged board was published, but deleting the prior directory failed and may have partly deleted it. Neither the error nor the rollback directory proves that the old board is intact. Stop for intervention. |
| Target absent, rollback directory present | Consistent with interruption between renames or failed rename-back. The directory is a prior-board candidate, not a validated recovery result. Do not recreate the target or rename it manually; intervention is required. |
| Target present after a crash, with or without rollback/staging directories | It may be the original board or the restored board; absence of rollback can mean cleanup completed before the managed completion record. Completion and authority are unresolved. Keep stopped and request intervention. |
| Failed rename-back, target absent without rollback, or multiple candidate artifacts | No supported deterministic recovery is available. Preserve all survivors; do not choose the newest directory or retry publication. |

Operator intervention means an offline recovery review using the exact command,
errors, trusted backup inventory, managed transaction/checkpoint identity (where
present), and surviving board data. The review must establish which copy was the
prior board, validate configuration and database health/identity/schema, compare
covered resources and payloads with trusted inventories, and preserve newer
CircuitNET catalog and FTN custody evidence. A rollback directory can identify the
prior target only when correlated with this particular replacement attempt and
validated; it might be an older artifact or partially removed. D1 has no persisted
native publication journal or supported manual-restore finalization command that
performs this review. If authority or intactness cannot be established, remain
stopped and obtain recovery assistance; do not teach the board a guessed history.

After **confirmed completion or confirmed intact return of the original board**,
and with no unresolved managed transaction, use the selected compatible runtime's
normal `spitfire status /path/to/board/spitfire.toml`, then verify identity,
resources and the intended message/file state before following normal startup
checks. Status is an operational check, not a forensic integrity verifier. A
managed pending restore must be resolved by reviewed recovery intervention before
this step; ordinary update recovery cannot clear it safely.

The [native publication contract](../sfng-backup-restore.md#restore-validation-and-determinism)
explains the rename boundary. There is no whole-host or power-loss-safe manual
restore promise in D1.

## Protect the snapshot

A native backup is sensitive. SQLite contains password hashes, caller contact
profiles, private messages, receipts, statistics, and operational history.
SYSTEM may contain the SSH private host key. Possession of that key can
impersonate the restored board's SSH host identity, so it requires the same
protection as the database.
Protect the whole directory with appropriate host permissions and copy it as
one unit. Do not edit the manifest or contents. Verification detects inventory,
size and checksum mismatches against the recorded manifest. The native manifest
is an unsigned integrity inventory: someone able to change both content and its
manifest can recompute those checksums. D1 additionally records the manifest hash
in installation control state, but this does not authenticate a backup against an
attacker who can alter that state too. Release signatures authenticate runtime
packages under the separate pinned release authority; they do not sign backups.

Physical retention/pruning, encryption, removable/cloud copies and replication
belong to the host operator. D1 adds capacity limits and verified history; it
does not automatically delete or export backups.

The authoritative format, validation, rollback, and exclusion contract is
[SPITFIRE NG Native Backup and Restore](../sfng-backup-restore.md).

For self-revoked permissions, damaged configuration, and restored-board operator
enrollment, follow [Operator Startup and Recovery](../manual/operator-recovery.md).

## N2 QWK networking

Schema-21/22 network queues, private envelopes, receipts and provenance participate in cold backup. Restored unsent work is held for operator review; manual inbox candidates and live sessions do not resume.
See [QWK networking](../manual/qwk-networking.md) for the implemented scope and interoperability limits.

## N3 FTN integration

See the [FTN Sysop procedure](../manual/ftn-core.md) for isolated operation and
[Technical Reference](../technical/ftn-core.md) for authority/privacy/recovery.
Native messages remain canonical; private FTN mail is separate from QWK private
mail. Native [BinkP transport](../manual/binkp.md) is implemented; live
public FidoNet participation is not claimed.

N4 BinkP recovery retains private credentials and accepted custody, clears live
claims and holds uncertain work. See [the recovery workflow](../manual/binkp.md#queues-failures-and-recovery).

## N5 operator and recovery extension

[Operating networking](../manual/network-operations.md) now provides the Networks cockpit, typed
configuration, safe queue actions, directory/quarantine visibility and verified
restore recovery. Same-root replacement retains proven later FTN serial floors
and matching peer acknowledgements. New-root recovery uses a stopped surviving
source, retires its origination, and keeps uncertain work held. No live public
FidoNet participation is claimed. Later accepted N6 [hub operations](../manual/ftn-hub.md)
and N7 [file networking](../manual/ftn-files.md) extend this recovery foundation.

## Identity data in schema 28

Cold backups preserve private First/Last Name fields, unclassified legacy full
names, immutable message authors, identity policy, queue sender snapshots and
name-change audit metadata along with existing configuration/artifacts. Backup
contents remain private; the manifest does not list caller names. Restore keeps
old posted and queued names even when the profile now differs.

A schema-27 upgrade preserves legacy name values exactly and leaves new components
unset. It never splits a Handle or assumes a copied full-name field is reliable.
Complete components through caller or authorized operator profile entry. The
migration is transactional; a failure leaves schema 27 unchanged. See the
[identity migration contract](../technical/identity-policy.md).
