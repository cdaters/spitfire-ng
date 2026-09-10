# Caller admission, ownership and safe account writes

D2 caller/session contract (2026-09-10). This document defines authoritative
admission, session ownership and safe mutable account writes.

Authentication identifies a stable caller ID using the existing Login and
Argon2id credentials. Password input and hashing happen outside write transactions.
Admission uses a short SQLite Immediate transaction to read current lifecycle,
effective access and daily counters, require eligibility, claim exclusive account
ownership, reserve the available per-call allowance, and record the successful
call. Only a committed admission may mark the session authenticated.

The existing runtime state and authentication substate form one session engine:

| Runtime state | Authentication substate | Meaning |
|---|---|---|
| `Created` | `NotStarted` | Connection has not entered the session engine |
| `Active` | `Unauthenticated` | Presentation and returning/new-user choice |
| `Active` | `ExistingCallerLogin` | Bounded credential prompts and admission |
| `Active` | `NewCallerRegistration` | Uncommitted application and validation |
| `Active` | `Authenticated(CallerId)` | Committed admission, entry journey and Main |
| `Closed` | Prior authentication retained for settlement | No further caller interaction or state transition |

Here `Active` means the connection is running; authenticated privileges require
the authenticated substate. Admission commits before that transition. Closure
keeps the identity needed for exact-owner settlement and cannot reopen the session.

One active session per account is the fixed policy, including Sysops. Historical
SPITFIRE 3.7 documents refusal when already logged on another node. NG strengthens
this with an atomic claim rather than depending on a stale caller snapshot.
There is no cross-namespace Login/Handle fallback and no special login string
that grants privileges. Private names and immutable posted authors remain governed
by [identity policy](identity-policy.md).

Schema 37 adds active claims keyed by caller ID, with runtime generation, node,
session, acquisition time, board-local usage day and reserved seconds. Uniqueness
serializes competing admissions. The reservation is the existing eligible
per-call allowance, capped by remaining daily time. No database transaction stays
open while a caller interacts. Explicit authorized operator time adjustments
retain their existing authority; a reservation does not redefine that policy.

Settlement requires the exact runtime/node/session/caller owner. Duration and
the completion event commit in the transaction that releases the claim. A stale
or unrelated owner cannot charge time or delete another claim. Normal cleanup
also clears node, interaction, time-control and transport registrations. An
unwinding connection releases only its own surviving claim; durable failures are
recoverable at startup.

Only a board runtime holding the existing exclusive board-operation lock may
recover interrupted claims before accepting callers. It records bounded recovery
events and releases the old reservations. No old runtime can still serve that
board under the lock. No wall-clock TTL or heartbeat is required. Previously
committed login counts and settled durations survive; elapsed time that never
committed before a process crash is not invented. An interrupted reservation is
released, not recorded as measured caller duration. This is deliberate recovery
policy, not a guarantee of accounting unobserved crash-time usage.

Terminal preferences and the entire private profile use the existing account
state_version as a compare-and-swap token captured before prompting, together with the immutable caller ID. Every actual
profile/preference mutation advances it, including contact-only changes. A stale
save preserves the committed row, records a bounded conflict and asks the caller
to retry. Lifecycle, security, names and login retain their authorized services;
ordinary preference/profile updates cannot write those fields. Publicity has its
own existing revision. Accounting uses additive/current-row transactions, never
a saved caller record. Reader/checkpoint writes retain their existing narrow pointer and version
contracts.

Backup and restore include claims as part of the native database. D1 descriptors
must advertise schema 37 and caller-session custody. A schema-36 runtime is not a
compatible rollback target. Compatible runtime rollback preserves all current
data; normal runtime startup then performs the documented interrupted-claim
recovery. Networking identity, custody and protocol contracts do not change.

Local operator IPC minor 19 carries the new caller policy fields. For older
configuration clients, the server projects the existing snapshot shape without
those three fields while retaining the actual opaque revision/digest. Their
field-scoped edits cannot erase the omitted policy. D2 configuration clients
require a minor-19 server before displaying/editing the extended policy, rather
than presenting default values as an older server's actual policy. Other operator
features and all external network protocols retain their existing boundaries.
