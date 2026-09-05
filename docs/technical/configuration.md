# Typed configuration authority and sfconfig

> Current source; database schema 19. This document describes B021-C, not a
> release/package or a generic configuration-file editor. B021-D reconciles its
> operator recovery and lifecycle boundaries.

## Authority and historical basis

SPITFIRE's configuration concepts remain board/Sysop identity, caller access,
nodes, terminal services, presentation, conferences, and file areas. The
[operator-controls gate](../research/m039-tranche-7-b021-operator-controls-gate.md)
records the bounded historical evidence and modern ownership/effect contract.
The [setup specification](../sfng-setup-configuration.md) remains canonical for
individual legacy/native fields. No new historical format inference is involved.

`RuntimeConfig` remains typed static configuration. `sf-core::configuration`
defines a closed field vocabulary, field labels/sections, sensitivity/effect
metadata, staged candidates, version identities, and shared validation. The
sf-bbs configuration service owns persistence, recovery, and authorization.
`OperatorService`, `OperatorClient`, offline authority, and sfconfig reuse these
interfaces; no frontend owns SQL or writes TOML. Setup continues using the same
RuntimeConfig validator and atomic writer. The legacy stopped-board
`spitfire config` also advances the revision and retains one prior configuration.

Database-owned conferences/file areas remain in their existing domain services.
Snapshots expose bounded safe read-only summaries (up to 128 of each), including
file-area versions. Their editors and transactions are not duplicated in the
static-config service. Identity renaming and root replacement similarly retain
existing offline authority rather than becoming cross-store online edits.

## Online protocol and authorization

Protocol 1.5 retains D-064's baseline hello. The `configuration` feature and
new capability vocabulary appear only in authenticated minor-gated discovery;
older peers never receive unfamiliar enum members. `ConfigurationSnapshot` and
`ApplyConfiguration` are typed operations over the existing protected Unix socket
or Windows named pipe. The session, request deadline, daemon generation, bounded
frame, and verified OS peer remain mandatory.

ReadConfiguration authorizes snapshots. ChangeOnlineConfiguration authorizes
ordinary mutations; ChangeSensitiveConfiguration is additionally required for
security, admission/listeners, and operator-profile changes. The service checks
current policy inside its serialized configuration transition. The unchanged
six-read bootstrap does not acquire these grants. Unknown, duplicate, empty,
oversized, or malformed capability profiles fail shared domain validation.

The daemon is sole online authority. Configuration changes serialize through
one short service lock; UI clients hold no global editing lease. Caller policy
publication is an infallible in-memory replacement after atomic file commit.
Each new caller captures the published configuration for its session. Existing
sessions keep their policy and existing B021-B controls remain independent.

## Version, conflict, and effect model

The file carries a monotonically increasing `revision` (old files default to
zero). The snapshot version includes revision plus SHA-256 of deterministic
validated configuration serialization, excluding its recovery link. A mutation
must supply the complete expected identity. A changed revision or digest returns
a typed conflict without writing the candidate. The digest also detects external
recovery edits that did not increment the counter. Restoring an older board is
an explicit offline recovery boundary, not an online version rollback.

All static domains share one atomic aggregate revision, so cross-field candidates
cannot partially succeed. Database resources retain their independent versions.
There is no schema-20 migration: existing canonical TOML and schema-19 command
receipts represent all required durability. Older binaries with closed config
parsers cannot consume newly written revision/recovery metadata; use source
checkpoints consistently and preserve a cold backup before changing versions.

| Class | Implemented boundary |
|---|---|
| Applied online | Operator capability policy at dispatch; Windows admission additions require pipe recreation/restart. |
| New sessions | Caller access, time/registration/subscription/profile policy, and credential length/login bounds. |
| Restart required | Node pool, listener enabled/bind settings, board timezone, presentation profile/menu mode, and default locale. |
| Offline only | Existing identity maintenance, backup/restore, and root/database replacement. These are not new online commands. |

Snapshots compare persisted restart-class fields with the daemon's startup
configuration. A pending change remains labeled restart required until a normal
external restart consumes it. There is no automatic restart or new restart
control. Existing sessions and caller transports remain owned by their original
runtime/session services.

## Atomic commit and recovery

The online transition is:

1. Serialize through configuration authority and reconcile prior recovery state.
2. Reauthorize the principal; accept/check the existing principal/generation-bound
   CommandId and canonical candidate fingerprint in schema 19.
3. Compare the expected version, validate the full candidate, and record semantic
   preparation/field-category audit without raw values. Preparation success is
   recorded as configuration.prepare; only durable commit records a successful
   configuration.apply.
4. Write, sync, and atomically replace one prior-generation `.toml.previous` file.
5. Write and sync a complete candidate in the same directory, then atomically
   replace the current file. The candidate carries a bounded recovery link to its
   CommandId, principal, generation, fingerprint, result class, and digest.
6. Publish the session-policy snapshot in memory, sync the containing directory
   on Unix, and transactionally finalize the receipt and success audit.

The recovery link contains no configuration copy or secret. On startup or the
next mutation, a matching file digest proves the commit; receipt finalization
and audit run once in one SQLite transaction. Remaining accepted configuration
receipts are rejected as not committed. A mismatched link/digest fails closed;
manual edits cannot manufacture proof of a prior command's payload.

Failure before replacement retains the previous current file. Failure after
replacement returns RecoveryRequired rather than claiming rejection; the next
exclusive reconciliation finishes its durable evidence. A lost successful reply
replays its receipt without another increment. A reused CommandId with a changed
fingerprint or principal fails closed. There is no rollback across active callers
or last-writer-wins retry.

The service uses the existing same-directory temporary-file writer, syncs file
contents, and atomically replaces complete files. The fixed prior filename
bounds backup history. Cold backup/restore continues to retain exact current
configuration plus consistent SQLite; the convenience prior-file copy is not
part of its manifest and is not required to restore authoritative state.

## Offline authority and compatibility tooling

`--offline` is explicit. The service acquires the same board operation lock used
by runtime, setup/config, backup, and restore, then validates configuration,
current schema, and database identity/integrity. It retains the lock for the
application lifetime, excludes daemon startup, and uses the same typed candidate,
validation, CAS, atomic persistence, and receipt/recovery implementation.
It never silently takes over after an IPC error.

The legacy BoardAdmin path remains a cold-board compatibility interface for
identity and database-owned editors. Static saves compare their loaded version,
advance the revision, retain the previous file, and retire the old commit link.
Its established identity-database update/rollback ordering is unchanged.

## Privacy and secret boundary

Static configuration contains policy and key paths, not caller credential or
private-key values. Opaque modem initialization/answer commands can contain device
credentials: snapshots redact both while unrelated saves preserve their canonical
bytes. Snapshot projection also removes the recovery link and exposes
SSH private-key state as Missing, Configured, or Invalid. There is no secret
input field, replacement, or clear command in this MVP: SSH key rotation is a
separate maintenance operation. Future secret fields must use explicit unchanged,
replace, and clear operations, write-only inputs, and status-only projections;
they may not be added to the generic candidate fingerprint or diagnostic output.

Configuration audit records operation, field category, authenticated principal,
CommandId, outcome, and revision/result class. It contains no old/new raw policy
values, endpoint/path dumps, private key, credential, caller content, or transcript.
Capability UI diffs show semantic additions/removals before save. No raw TOML,
SQLite, filesystem browser, host shell, or arbitrary command surface exists.

## TUI and handoff

`sf-config` is a separate workspace crate with the `sfconfig` executable, using
existing Ratatui/Crossterm versions. It owns staged edits, navigation, review,
dirty/conflict status, and contextual `configuration.*` help only. IPC is bounded;
a five-second status probe detects connection loss without changing authority.
The first loss latches disconnected state and stops automatic probes, avoiding
repeated authorization-denial audit. It retains the candidate/CommandId, closes
save review, blocks further save/reload dispatch, and requires explicit process
reopen. Offline errors retain offline identity. A recovered successful receipt
cleans the proven candidate before any follow-up read, including read revocation.
Reload is explicit when drafts exist. Review/help scroll; resize preserves state;
undersized screens accept no hidden configuration edit/save.

sfmonitor restores raw mode/alternate screen before invoking the sibling
sfconfig executable with the explicit board argument. It waits for the child,
then restores its own terminal and refreshes daemon state. No shell is involved,
no secrets are passed in argv/environment, and the two processes do not compete
for full-screen terminal ownership. Q quits only the current operator tool.

## Verification, platforms, and extension boundary

Tests cover version/digest, stale CAS, replay, validation, bootstrap/enrollment,
live revocation, online wire clients, offline exclusion, bounded prior backup,
restart persistence, interrupted receipt recovery, and TUI edit/cancel/save,
dirty/conflict state, capability staging, secret-status rendering, and resize.
The [implementation report](../research/m039-tranche-7-b021c-sfconfig.md) records
native acceptance and final quality gates; the [Sysop Manual](../manual/sfconfig.md)
provides operational instructions.

Apple Silicon macOS is the real acceptance platform. Windows named-pipe config,
SID enrollment UI, rendered TUI, handoff, and filesystem atomicity remain
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**. Shared source architecture is
preserved for Windows, Linux, and BSD; no Linux/BSD live acceptance is claimed.
B-022, networking, doors, scheduler, exports, service packaging, and
release distribution remain outside this implementation.


## B021-D recovery acceptance

The [operator recovery chapter](../manual/operator-recovery.md) is the supported
first-start, permission recovery, invalid-configuration, and known-good restore
journey. Reopening explicit offline authority requires the same exclusive lock
as daemon startup; neither mode silently takes over. Invalid persisted bytes are
left unchanged, and full restore into a new root avoids relying on invalid
existing configuration to prove replacement identity. The restored receipt link
is reconciled against the preserved journal before another typed save.

Fault-injection tests require preparation-audit failure to prevent replacement,
post-replacement receipt failure to recover once, and no fabricated success.
No repair, shell, key editor, schema change, or extra durable authority is added.
