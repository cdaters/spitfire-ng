# Deployment storage and backup contents

Every managed backup protects the configuration, consistent database and SYSTEM
and DISPLAY resources. This includes users, messages, conferences, Files metadata,
Events, network state, trust enrollment, generated host keys and operator settings.
The manifest also records the installation policy, pinned release authority,
source runtime/schema/configuration, time, hashes and restore requirements.

File payloads fall into two ownership groups:

- **Managed:** bytes stored in SPITFIRE-owned Files storage, including native
  content objects. The default copies them. Metadata-only policy records their
  paths, sizes and hashes and requires matching separately retained bytes.
- **External:** libraries on operator-managed volumes, NAS or removable media.
  Backups retain metadata, expected paths, sizes, hashes and availability. D1
  neither reads nor copies their bytes automatically, even with managed-only policy.

Runtime binaries are retained under `releases/`, separately from the board
snapshot. Preserve the installation directory as well as your backups for loss
of the whole host. The managed restore command uses an existing installation's
verified history; it is not a new-host installer. Work files, runtime endpoints,
incomplete staging, uncataloged payloads and logs outside critical resource trees
are excluded. Existing domain recovery reconstructs its permitted caches.
SYSTEM and DISPLAY are copied conservatively in full.

## Choose a storage policy

Edit `[storage]` in `INSTALL/installation.toml` while maintenance is stopped.
All values below are bytes, except the payload policy:

```toml
[storage]
file_payloads = "managed-only" # or "metadata-only"
backup_max_bytes = 17179869184
backup_retention_bytes = 68719476736
minimum_free_bytes = 67108864
reserve_free_bytes = 268435456
hard_stop_free_bytes = 67108864
low_space_warning_bytes = 1073741824
max_managed_files_bytes = 18446744073709551615
max_file_area_bytes = 18446744073709551615
max_single_file_bytes = 1073741824
```

These are backup/update eligibility limits. They do not replace existing File
Area upload limits or implement live disk quotas. `max_managed_files_bytes`
counts unique managed paths; area limits count that area's file references.
The default total and area values mean no practical extra deployment limit.
Set smaller limits for your installation if appropriate.

Run `spitfire update --dry-run` after policy changes. It reports the required
backup and temporary workspace budgets and any low-space warning. Planning
includes critical state, selected payloads, a manifest allowance, staging,
activation and failure recovery. Migrated-state growth is checked again before
activation. Capacity must be measurable; unavailable capacity information fails
closed. Filesystem overhead and other processes can still exhaust a volume;
keep a reserve and dedicated space for critical recovery.

A backup over its size or retention budget, insufficient reserve, oversized
managed file or exceeded managed/area threshold stops the operation. No policy
silently changes a managed-only backup into metadata-only. Critical state is
never omitted to make an update fit. All external exclusions and managed
references are recorded in the manifest and counted in the operator report.

## Validation reads and large libraries

Metadata-only reduces copying and backup storage. It does **not** eliminate
managed-content validation reads: D1 still streams and hashes cataloged managed
payloads and managed content objects against their recorded identities. External
payload bytes are not read; their recorded references are not proof of current
media availability or current byte integrity.

Very large or slow managed libraries can therefore take substantial time to
validate, even when their backup payload policy is metadata-only. Runtime
validation workers currently have a ten-minute time bound; a library can exceed
that bound and cause refusal/failure rather than an unchecked update. Other
planning/hash work is not a promised ten-minute end-to-end operation. Use
`spitfire update --dry-run` with the intended release and policy to exercise
preflight, and plan a stopped-board maintenance window for the validation reads,
backup, staging and repeat validation. A successful dry-run does not guarantee
later performance or capacity. D1 has no throughput guarantee or terabyte-scale
acceptance claim.

## Which thresholds warn or refuse

| Setting | Actual backup/update preflight behavior |
|---|---|
| `minimum_free_bytes` | Refuses if measured free space is below this value before budgeting the operation. |
| `hard_stop_free_bytes` | Refuses if initial free space is below it, or projected remaining space falls below it. |
| `reserve_free_bytes` | Projected remaining space must be at least the larger of this reserve and the hard-stop threshold. |
| `low_space_warning_bytes` | Warns when projected remaining space is below this value after the refusal checks pass. It does not authorize crossing a reserve or hard stop. |
| `backup_max_bytes` | Refuses if copied snapshot content plus the 16 MiB manifest allowance exceeds the limit. Referenced-only payload sizes are not charged as copied bytes. |
| `backup_retention_bytes` | Refuses if regular-file bytes already under `INSTALL/backups` plus the new backup budget exceed the limit. No old backup is deleted to make room. |
| Managed total, area and single-file limits | Refuse ineligible managed content during planning, including with metadata-only policy; these are not a fallback payload-exclusion policy. |

For an update, installation workspace budgeting adds two critical-state copies,
the candidate runtime bytes, and another 16 MiB allowance to the backup budget.
Board workspace budgeting adds two further critical-state copies. Preflight
conservatively subtracts the **combined** installation and board budget from the
free-space measurement at each root, even when they are on separate volumes.
It never adds the two free-space measurements together. An ordinary backup has
no update staging allowance. Update checks migrated database/configuration growth
and recovery scratch space again before activation; dry-run figures are an
estimate, not a disk reservation. Restore and runtime rollback also check their
own temporary workspace against the reserve/hard-stop floor.

Already retained runtimes and transaction staging reduce measured free space but
are **not** charged to `backup_retention_bytes`. Neither that retention budget nor
`backup_max_bytes` caps total installation disk use. Configuring a larger backup
budget does not bypass the actual-free-space checks. Critical state is never
silently dropped: an operation that cannot meet these checks stops explicitly.

## Retention and low space

D1 never automatically prunes backups, checkpoints or previous releases. A full
retention budget refuses new backups/updates. Transaction staging is retained for
diagnosis and also consumes real disk space. There is no automatic garbage
collector. Expand capacity or archive the complete stopped installation to
operator-managed storage before making a reviewed retention change. Do not delete
the only known-good checkpoint, a pending transaction or its referenced runtime.

Backups are sensitive: they contain credential hashes, private messages and
possibly private host keys. Protect the entire directory. D1 does not encrypt,
replicate or upload it. External media and referenced managed payloads need their
own backup plan. A recorded reference identifies expected content; it does not verify current
external bytes or present media availability. Missing referenced managed bytes cause restore refusal before any
board replacement.

See [Updating](upgrades.md) for recovery and runtime rollback, and
[Backup and Restore](backup-restore.md) for destructive disaster recovery.
