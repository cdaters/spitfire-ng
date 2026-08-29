# Caller Access Lifecycle and Security

Current SPITFIRE NG source keeps caller identity and access policy separate.
The caller record owns a stable identity, credentials, private profile, base
security, lifecycle state, optional subscription expiration date, purge-
protection flag, and optimistic state version. Authorization derives effective
security from that record and any active, reasoned restriction.

## Lifecycle

A caller is in exactly one lifecycle state:

- **Active** — may authenticate, subject to normal board and security policy.
- **Disabled / Locked Out** — cannot authenticate; identity and related data
  remain intact so the account can be enabled later.
- **Deleted** — a recoverable tombstone. The stable caller identity and related
  data remain intact and can be restored. This is not a physical purge.

Disabling or deleting a caller requests disconnection of that caller's active
sessions. Every main, message, and file command dispatch reloads lifecycle and
effective security before authorizing the command, so an already connected
caller cannot retain stale access. Disabled and deleted callers are also
rejected by authentication.

Purge protection is a separate persisted eligibility flag for a future
retention/packing service. New and migrated callers are protected by default.
Current source does not physically purge callers.

## Base and effective security

Base security is the operator-assigned numeric level stored on the caller.
Effective security is derived when access is checked:

```text
effective security = minimum(base security, every active adjustment target)
```

The current adjustment kind is subscription expiry. Expiry can lower effective
security without overwriting base security. A later base-security change does
not bypass the restriction, and renewal removes the adjustment so current base
security becomes effective again. Menus, conferences, files, private-board
admission, message mutation, and Sysop thresholds use effective security.

The configured named Sysop identity is protected separately from threshold
privilege. A caller who merely meets the Sysop threshold does not become the
named Sysop. The named Sysop cannot be disabled, deleted, made purge-eligible,
lowered below the configured Sysop threshold, denied by an accepted
`JOKER.DAT` policy, or reduced by subscription expiry.

## Subscription policy

Each caller has either a nullable expiration date or a permanent/no-expiry
state. Dates use full `YYYY-MM-DD` form and the board's configured local time
zone. The displayed expiration date remains valid through that board-local
date; expiry begins after local midnight, when the board date is later than the
stored date.

The optional board policy is configured under `[caller.subscription]`:

```toml
[caller.subscription]
enabled = true
warning_days = 7
expired_security = 5
```

`warning_days` is an inclusive window ending on the expiration date. A caller
within that window receives the subscription warning after authentication.
When an expired caller authenticates or reaches another command dispatch, an
idempotent reasoned adjustment lowers effective security to no more than
`expired_security`. Setting an expiration date that is today or later, or
selecting permanent/no-expiry, resolves the active expiry adjustment and
restores current base security where no other adjustment applies.

## `JOKER.DAT` name-denial policy

The optional current-source policy is `SYSTEM/JOKER.DAT`, loaded as one
immutable generation when the board starts:

- a normal non-empty line denies that complete caller name;
- a line beginning with `@` denies a caller name containing the remaining
  text;
- matching is ASCII case-insensitive after normal caller-name space handling;
- no Unicode normalization is performed; and
- phone, address, email, and other contact fields are never matched.

The current parser does not define comments, wildcards, regular expressions,
or broader legacy syntax. It accepts ASCII policy lines only and enforces
bounded file, line, and rule counts. A malformed policy, an empty `@` rule, or
a rule that would deny the configured named Sysop prevents board startup.

New and returning names are checked. A denial uses generic `LOCKOUT`
presentation and neither caller output nor the audit event reveals the matching
rule or supplied name. Edit the file only while the board is stopped, then
restart deliberately.

## Mutations, audit, and concurrency

Lifecycle, base-security, subscription, and purge-protection mutations run in
transactions and reauthorize the actor at dispatch. A threshold Sysop must
still be active and meet the effective-security threshold at that moment;
local operator and system-policy actors remain distinct in the audit.

Every caller has a monotonically increasing state version. A mutation supplies
the version it read, and a mismatch fails as a stale conflict instead of
overwriting a newer change. Committed changes and policy outcomes create
append-only caller-access events with operation, actor category, subject ID,
relevant before/after state, and adjustment or policy-generation metadata.
Events exclude passwords, password hashes, caller names, supplied denial
values, matching JOKER rules, subscription dates, and private/contact fields.

## Schema 11 to 12 and recovery

The transactional schema 11 to 12 migration preserves existing caller IDs,
credentials, private profiles, statistics, and lifecycle states. Existing
stored security becomes base security; state versions start at zero,
expiration is nullable, and callers begin purge-protected. Schema 12 adds the
reasoned-adjustment and append-only access-event tables plus integrity checks.
Migration failure leaves schema 11 unchanged.

Cold backup preserves all schema-12 caller-access state. Because
`JOKER.DAT` is under `SYSTEM`, it is copied byte-for-byte when present. Restore
accepts exact schema-10, schema-11, and schema-12 snapshots, restores the
snapshot's original version, and leaves older-to-current migration to the next
normal writable startup. Unsupported or structurally inconsistent snapshots
fail before publication.

See [Caller Management](operator/caller-management.md) for commands and
[Native Backup and Restore](sfng-backup-restore.md) for the recovery contract.
