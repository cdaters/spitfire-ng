# C4 — directed CircuitNET routing and authenticated Dossier controls

This independently authored summary describes modern SPITFIRE NG behavior. It
contains no historical source, packet layout, private archaeology or acceptance
credentials. No legacy compatibility is claimed.

C1 remains historical authority; C2 remains the native/offline foundation; C3
remains live encrypted/authenticated transport. C4 adds typed destination routing
and administrative subscription controls around canonical native messages.
CircuitNET remains independent of FTN/QWK/BinkP.

## Implemented behavior

Schema 31 adds immutable destination intent and metadata, durable incoming/outgoing
request/result state, policy and audit linkage. No second conference payload store
exists. The configured connected acyclic END/HOST/ROOT tree supplies unique paths;
only the next direct neighbor receives directed intent. Destination mapping must
permit receive. Intermediate records stay in native transit containers. Directed
traffic does not require or mutate broadcast Dossiers and does not broadcast.
Unknown destinations leave durable failure intent excluded from broadcast scanning.

Typed Subscribe, Unsubscribe, QuerySubscriptions and SubscriptionResult semantics
are carried by bounded Controls/ControlResults frames on the same mutual-TLS
session. Only an authenticated configured direct child can request its own Dossier
at its parent. ROOT has no upstream target. Require approval is the default;
auto-approve and deny are explicit. Approval rechecks policy/mapping and commits
Dossier mutation and result atomically. Exact replay returns the retained result;
conflicting identity reuse rejects. No-op subscriptions succeed deterministically.

Protocol major 1 supports minors 0–2. Minor 2 negotiates the independent optional
`directed-routing` and `remote-dossier-control` capabilities. Minors 0–1 keep their
existing conference exchange and receive no C4 work. Absent destinations preserve
C2 encoding/fingerprints. Directed metadata participates in fingerprints. Operator
IPC minor 14 gates C4 mutations separately from C3 controls.

The [native contract](../technical/circuitnet.md#c4-interface-gate),
[wire specification](../technical/circuitnet-transport.md#c4-compatible-minor-2)
and [human manual](../manual/circuitnet.md#how-directed-routing-works) describe
exact authority, bounds, framing, commands and recovery.

## Privacy and deliberate limits

CircuitNET transport is encrypted. Delivered conference messages are readable by
users authorized for the destination conference. Directed routing chooses a node;
it is not private messaging or end-to-end encrypted messaging. Intermediate
operators can access native transit records. Local/private BBS messages elsewhere
are restricted by BBS access controls unless an explicit feature provides E2EE.

C4 destination selection is operator-only before publication; caller composition
is unchanged and caller help explains visibility. Historical subject syntax remains
a deferred compatibility layer. Offline directed exchange uses C2 trusted custody;
offline remote controls are deferred because files do not establish live TLS
identity. No CircuitNET private mail, files, governance, automatic catalog, legacy
CNP/CND or E2EE is added. No production system or external CircuitNET peer changed.
No binary release, tag, assigned public port or production rollout accompanies C4.

## Verification

Focused tests cover six-node routes, wrong profile/unknown nodes, END prohibition,
no fanout, final mappings, directed threading, request authorization, policies,
approval/denial, replay conflicts, no-op results, pending/apply restart, migration
rollback and privacy/help labels. The six-daemon Darwin arm64 acceptance uses
ROOT1, HOST1, HOST2, END1, END2 and END3 with independent native databases and TLS
identities. It checks same-branch, cross-branch and ROOT-directed paths and exact
per-hop metadata/queue neighbors; unrelated boards retain neither import nor intent.
Independent TLS probes exercise lost intermediate/destination ACK and lost control
result. Cold sender and HOST restore preserve routing/control truth and replay
without reapplying an older operation. The original offline C2 and live C3 broadcast
and reconnect tests remain independent regressions.

Reproduce with `cargo test --workspace`. The six-node test is in
`crates/sf-bbs/tests/circuitnet_live.rs`; its optional `SPITFIRE_C4_EVIDENCE` directory
retains disposable private artifacts outside public source. All daemon tests use
loopback only and require local listener permission. No generated board, key,
certificate, backup, log or packet is published.

**C4 COMPLETE / ACCEPTED.** Public workspace: **720 passed / 0 failed / 7 existing
ignored**, all six doctest groups. All 131 headers, fmt, all-target Clippy with
warnings denied, diff and local documentation/provenance gates pass. C4 adds 13
automated tests. The live process campaigns serialize to avoid localhost startup
contention. Intermittent pre-TLS loopback connect failures were also observed
after disposable HOST restart. The fixture allows at most two additional explicit
Polls for connect-only failures, preserving all exact state assertions; production
retry limits, deadlines and TLS admission remain unchanged. Final standalone
six-node acceptance passes. No OS/Rust defect repair is claimed.
en-US is 1.28.0 / 1,354 messages. cargo-audit
is unavailable in the acceptance environment; no dependency audit pass is claimed.
The public delta preserves earlier caller-guide sanitization and includes only
reviewed source, tests, localization, manuals and independently authored continuity.

**Exact next action after C4 publication:** stop and review C4. Do not begin C5.
