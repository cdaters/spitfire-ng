# Deployment and recovery contract (D1)

This interface contract separates update failure recovery from data-preserving
runtime rollback. Implementation and acceptance results are recorded separately.
It does not change board schema 36 or any networking protocol.

## Three distinct operations

* A backup is a durable disaster-recovery snapshot with explicit copied and
  referenced content. Restore is destructive and retains native catalog/FTN
  recovery protections.
* A pre-upgrade recovery checkpoint protects an exclusively stopped board until
  COMMIT. Failure before COMMIT restores the checkpoint's database/configuration
  and old runtime. No candidate listener, Events executor or outbound networking
  is started during validation.
* Runtime rollback selects a previous runtime compatible with the **current**
  schema, configuration and required durable feature generations. It never
  replaces the board database or replays restore normalization. An incompatible
  candidate is refused without modifying state.

## Ownership and layout

The existing board root and relative disjoint logical paths remain in place.
External storage remains operator-owned. A separate installation directory owns
`bin/spitfire` (a stable forwarding launcher), `releases/<version>/`,
`installation.toml`, `deployment.sqlite3`, `backups/`, and `transactions/`.
An exact board-local `.spitfire-installation.toml` locator connects the roots.
Adoption is explicit and requires a signed package matching the running runtime;
it does not take ownership of Cargo's bin directory or a system service.

SQLite with FULL synchronization owns active/previous runtime metadata and the
transaction journal. A transaction records phase, old/target runtime, checkpoint
identity and manifest hash before live mutation. The active pointer and COMMIT
are one SQLite transaction. No symlink is the commit authority. The launcher
forwards to the selected immutable release; ordinary startup checks the locator,
journal and selected executable. The board's existing OS operation lock remains
held throughout maintenance. Managed metadata has an independent OS lock.

Candidate code receives a bounded internal worker request over stdin and runs
offline against a private staged board. Only staged database/config changes may
be adopted in D1. Board-owned resources and payloads are not release defaults.
Configuration paths/identity cannot change during an update. Config migration
must preserve unowned fields; D1's format-1 to format-2 migration edits the TOML
document value and validates before writing, and leaves format 2 bytes alone.
Forward database migrations remain ordered per-migration transactions.

## Release and runtime metadata

A local source contains package directories with `release.json`,
`release.sig` and `spitfire` (`spitfire.exe` on Windows). Format 1 uses exact
JSON bytes authenticated by Ed25519 over the domain
`SPITFIRE-NG-RELEASE-V1\n` followed by those bytes. `release.sig` is lowercase
hex. The operator pins a separate 32-byte SPITFIRE release public key in the
installation policy. It cannot be replaced by a package or discovery source.
Artifact length and SHA-256 bind the executable. No CircuitNET key is reused.
Unknown fields, duplicate/unsafe paths, incompatible versions/hosts and
unsupported manager protocol fail closed. A binary's offline descriptor must
exactly equal the signed descriptor before it can be activated.

The signed runtime descriptor includes product, Cargo SemVer, OS, architecture,
manager protocol, readable/writable schema ranges, migratable source range,
target schema, supported config formats and supported durable feature
generations. The release adds minimum upgrade version and artifact integrity.
The board report includes current schema/config and required feature generations;
requirements never decrease during update or runtime rollback. Current runtimes
conservatively advertise read/write schema 36 only. An older version number is
not evidence of compatibility. Future releases must update capabilities honestly.

## Protected state classification

| Class | Examples and handling |
|---|---|
| Ordinary recoverable critical state | Config, users/profiles, messages, conferences/mappings, Files metadata, operator policy: copied in every checkpoint; preserved during runtime rollback |
| Protected durable evidence | CircuitNET catalog revisions/hash history/publisher epochs, custody receipts and message/file publication identities; FTN serial/acknowledgement/custody history; QWK receipts/queues; message/File identities; trust enrollment; durable Events execution history: database/resource authority is never rewound by runtime rollback |
| Managed payload | Native content objects and legacy managed catalog bytes: copied or explicitly referenced according to policy, independently verified by size/hash |
| External reference | External media/NAS file locators, expected size/hash and availability: metadata retained, bytes never automatically copied by D1 |
| Regenerable/transient | Status/endpoints, incomplete staging, logs, derived content views: excluded; native recovery reconstructs or resets what its existing contract permits |

Critical SYSTEM resources include network artifacts and private credentials.
These are never excluded by a Files payload policy. Backup manifests are private
operator data, not public artifacts. Human reports contain counts and sizes,
not credentials, message bodies or private keys.

## Storage policy

Versioned strict TOML in `installation.toml` owns deployment policy. Payload
policy is `managed-only` or `metadata-only`; external bytes remain references in
both. Capacity is checked with overflow-safe arithmetic before checkpoint or
transaction creation. The plan includes critical bytes, copied payload bytes,
manifest allowance, stage/activation/recovery scratch, runtime bytes, existing
retained backups, and free-space reserves on both installation and board volumes.
Unknown free capacity is a hard failure. Tests inject capacity through an internal
provider, not a production CLI bypass.

Policy defines maximum backup bytes, backup retention bytes, minimum free bytes,
reserved free bytes, hard-stop and low-space warning thresholds, maximum managed
Files bytes, maximum per-area bytes, and maximum single-file bytes. D1 applies
these as backup/update eligibility budgets. Existing per-area upload limits remain
the live admission authority; D1 is not a new continuous quota/GC service.
No automatic pruning is performed; a full retention budget refuses new work.
The only known-good checkpoint is never automatically deleted.

Metadata-only restore requires independently verified matching managed payloads
from the stopped target. Missing bytes cause refusal before destructive restore.
External media is reported as referenced and remains governed by native restore
availability/rebinding rules. Recovery checkpoints retain every critical byte;
update workers cannot mutate excluded payloads or live resources.

## State machine and restart contract

CHECK/PREFLIGHT are read-only. Under exclusive maintenance ownership: BACKUP,
VERIFY_BACKUP, STAGE_RELEASE, VERIFY_RELEASE, MIGRATE, VALIDATE, APPLY_RUNTIME,
VALIDATE_ACTIVE, COMMIT, CLEANUP. All validation precedes COMMIT and normal operation. A persisted
pre-COMMIT transaction is recovered to its verified checkpoint and old runtime;
recovery is repeatable if interrupted. COMMIT atomically selects runtime metadata
and marks the transaction committed. Committed transactions only finish cleanup;
they never trigger database recovery. Corrupt/missing journal evidence refuses
startup and requires operator recovery rather than guessing from filenames.

Cold/manual operation is explicit: stop the board before maintenance. D1 does
not install or restart OS services. Real Windows executable/rename/reboot behavior
is **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**; portable contracts and cheap
compile checks do not substitute for that acceptance.

## CLI interface

`spitfire deployment adopt CONFIG INSTALL SOURCE PUBLIC-KEY` establishes ownership.
`INSTALL/bin/spitfire` infers its installation; an unmanaged binary may use
`--installation INSTALL`. Normal commands are `version`, `update --check`,
`update --dry-run`, `update`, `update --recover`, `backup`, `backup list`,
`restore ID --replace`, `rollback --list`, and `rollback --yes`. Rollback without
`--yes` reports impact and requires terminal confirmation. Restore requires
explicit `--replace` consistent with the existing CLI. Existing positional
native backup/restore commands remain supported outside the managed workflow.

## Format 1 reference and release construction

The executable reports its actual compiled descriptor with
`spitfire deployment-describe`. `RuntimeDescriptor` and `ReleaseManifest` in
`crates/sf-bbs/src/deployment/model.rs` are the strict serde schemas. A release is:

```json
{
  "format": 1,
  "runtime": {
    "format": 1,
    "product": "spitfire-ng",
    "version": "0.1.1",
    "platform": "macos",
    "architecture": "aarch64",
    "manager_protocol": 1,
    "read_schema": { "minimum": 36, "maximum": 36 },
    "write_schema": { "minimum": 36, "maximum": 36 },
    "migration_source": { "minimum": 10, "maximum": 36 },
    "target_schema": 36,
    "config_formats": [1, 2],
    "durable_features": {
      "circuitnet-catalog-history": 1,
      "circuitnet-custody": 1,
      "circuitnet-publication": 1,
      "ftn-custody": 1,
      "qwk-custody": 1,
      "message-identity": 1,
      "files-identity": 1,
      "trust-enrollment": 1,
      "events-history": 1
    }
  },
  "minimum_upgrade_version": "0.1.0",
  "executable": "spitfire",
  "size_bytes": 123,
  "sha256": "REPLACE_WITH_ACTUAL_64_LOWERCASE_HEX_DIGITS"
}
```

The example size/hash are explanatory placeholders, not an installable manifest.
Prefer the builder, which takes the descriptor from the compiled executable:

```sh
python3 tools/build-runtime-release.py --binary /release/spitfire \
  --output /release/source --private-key /secure/release-authority.der \
  --minimum-upgrade-version 0.1.0
```

The key is a separate Ed25519 PKCS#8 DER key. The builder refuses to overwrite an
existing package/version or silently replace its public-pin file. Its explicit
`--generate-test-key` option is for disposable acceptance only. Adoption rejects
reuse of a known board CircuitNET catalog authority. Key rotation and an official
release-signing ceremony remain separate operational work; D1 provides no remote
pin update and does not reuse Network Kit signatures.

Cargo SemVer remains project version authority. Format 1 release directory IDs
accept canonical SemVer without build metadata. Source discovery verifies every
package (bounded to 128), then selects the highest newer version. A malformed or
incompatible advertised package is an explicit failure; there is no silent skip
to another release. The local directory is the discovery contract. Restart is
inherent in stopped-board adoption, so a redundant restart flag is unnecessary.
The stable launcher understands manager protocol 1 only. Future protocol changes
must retain that selection contract or define an explicit bootstrap migration.

## Recovery snapshot format

`backups/<id>/spitfire-recovery.json` plus `board/` is a private directory snapshot,
distinct from the pre-existing native format-1 backup. `Snapshot` records:

- format, random ID, installation ID, UTC Unix creation time and type (`manual`
  or `pre-upgrade`);
- installation policy/public pin, source runtime compatibility descriptor,
  source schema/configuration and board identity;
- sorted table counts and deterministic typed-row SHA-256 fingerprints;
- exact included paths, state classes, sizes and SHA-256 hashes;
- managed/external references with file/area identity, storage root, relative
  path, size, hash, availability and whether bytes were copied;
- payload policy, excluded classes, logical reference size, actual stored content
  size and explicit restore requirements.

The SQLite control row records the exact manifest SHA-256, outside the manifest.
Verification checks it, the complete declared file inventory, each included
file, SQLite integrity/schema/identity, table fingerprints and payload reference
coverage against the backed database. Manifests cannot authenticate themselves.
The directory format has no compressed archive size; its exact file-byte size is
`stored_content_bytes + length(spitfire-recovery.json)`. Logical reference size
counts catalog references and can exceed unique copied bytes. Filesystem block
allocation and directory overhead are not confused with either number.

Metadata and inventories are bounded (16 MiB manifest, 100,000 inventory entries
plus references); excess fails explicitly. Physical data uses streamed hashing
and copying. Table hashing uses primary-key ordering where available, avoiding
sorting message bodies as the ordinary ordering strategy. Databases without a
primary key use full deterministic row ordering. The database backup API may
change SQLite header bookkeeping; verification proves rows/schema, not identical
source database file bytes. Backup/restore protection is local integrity and host
permissions, not confidentiality or resistance to an attacker controlling the
same operating-system account and all installed metadata.

## Migration and validation constraints

The selected signed binary must match its descriptor and artifact hash. Its
bounded offline worker opens only the stage for migration, refuses a managed live
board as migration input, and uses current sf-core schema transactions. Config 1
is transformed through a TOML value tree; all fields accepted by the strict
config parser are preserved. Unsupported unknown fields cause preflight refusal.
No config rewrite occurs for format 2. D1 does not migrate resource packages,
board paths, payloads or storage layout.

Validation checks expected version/descriptor and schema, SQLite health, board
identity, configuration, managed Files bytes and native content hashes, resource
startup loading, network artifact custody, and domain readers for CircuitNET,
FTN, QWK and Events. Every pre-existing table's typed-row fingerprint must survive;
only the migration ledger is exempt. Thus additive migrations can succeed, while
structural rewrites of existing rows/columns require a future explicit preservation
proof contract. Merely claiming a larger schema range is insufficient. D1 does
not pretend to support arbitrary destructive migrations.

Worker execution has bounded output and a ten-minute timeout, no listener start
or Event execution. Signature trust authorizes runtime code execution; this is
not an operating-system sandbox for malicious signed code. Maintenance requires
exclusive ownership of the stopped board and installation. Host administrators
must not concurrently hand-edit files or use historical executables predating
the managed-board guard.

## Exact phase recovery table

| Persisted phase | Live authority and restart action |
|---|---|
| No transaction / CHECK / PREFLIGHT | Current installation; read-only inspection, no recovery |
| BACKUP / VERIFY_BACKUP | Old runtime and live state; validate old board, abandon transaction |
| STAGE_RELEASE / VERIFY_RELEASE | Old runtime/state; retained uncommitted runtime is not authority |
| MIGRATE / VALIDATE | Only stage changed; validate old board, abandon staged operation |
| APPLY_RUNTIME | Verified checkpoint and staged hashes already journaled; database/config may be partly adopted; restore checkpoint and validate old runtime |
| VALIDATE_ACTIVE | Adopted files still pre-commit; restore checkpoint and validate old runtime |
| COMMIT / CLEANUP | Active runtime and required features atomically committed; finish cleanup, never restore checkpoint |
| MANUAL_RESTORE | Block automatic startup; no supported automatic replay/finalization command; follow the [manual restore interruption runbook](../operator/backup-restore.md#interrupted-manual-restore-keep-the-board-stopped) for reviewed intervention |

The board and installation locks remain held from preflight through cleanup.
There is no interval of normal operation between offline validation and COMMIT.
The active pointer, previous candidates, required feature generations and COMMIT
share one FULL-synchronous SQLite transaction. A rollback needs no database
recovery journal: its sole accepted write is this atomic pointer/history update
after isolated validation. Temporary validation files are not authority.

Unix uses file and parent-directory synchronization and rename publication;
Windows uses file flushes, SQLite journal authority and separate versioned
executables. The running executable is never overwritten. Windows directory
power-loss behavior is not claimed from a macOS compile check. Linux/*BSD runtime
acceptance remains unperformed. Service account, install prefix and startup
supervision remain operator packaging decisions; paths are explicit and data is
not embedded in runtime packages.

## Reproduce focused acceptance

```sh
cargo test -p sf-bbs --lib deployment::tests
python3 tools/build-d1-fixtures.py /absolute/disposable/d1-fixtures
SPITFIRE_D1_FIXTURES=/absolute/disposable/d1-fixtures \
  cargo test -p sf-bbs --lib deployment::tests::native_cli_acceptance -- --ignored --nocapture
```

The builder compiles actual Cargo versions in a disposable source copy. It does
not add a production version override or mutate workspace manifests. The fixture
contains synthetic users/messages/replies, conference mappings, managed/external
Files, Events history, FTN/QWK configuration/state, a separately signed CircuitNET
catalog and synthetic trust resources. Native acceptance adds a generated test
certificate. No external network traffic or production credentials are used.
The future-feature variant exercises compatibility refusal without changing
CircuitNET/FTN/QWK semantics. Runtime/model fault hooks are test-only. Synthetic
capacity accounting uses small real fixtures, never giant allocations.

Database-resident derived tables remain within the consistent SQLite snapshot;
D1 does not delete rows to shrink a checkpoint. Separately regenerable files and
uncataloged payloads outside critical resource trees are explicitly excluded.
Deployment parser errors omit source text so malformed configuration cannot echo
credentials into operator logs.
