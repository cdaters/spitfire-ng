# FTN hub services

N6 extends the [native FTN authority](ftn-core.md). The interfaces below were defined before implementation. The implemented
contract is exercised by the [M050 acceptance report](../research/m050-networking-n6-ftn-hub.md).

## Ownership and roles

Routing remains explicit FTN link transit permission and next-hop routes. Hub
capability is the presence of enabled downstream relationships. Point-boss
capability additionally requires a downstream's nonzero point address to match
its configured node AKA's boss address and domain. These capabilities coexist;
there is no exclusive system-role enum. BinkP owns transport only.

`Downstream` references an existing immutable FTN link binding; it owns enabled
state, hold state, optional boss AKA, AreaFix permission, rescan permission and finite bounds.
It has no endpoint or transport credential. `Subscription` separately owns one
downstream/domain/AREA association, state, source and revision. Native conference
mappings retain peer/uplink membership; downstream membership cannot also be
edited through those mappings. Converting an existing peer to a downstream must
transfer its subscriptions transactionally. Retained downstreams are disabled,
never silently deleted. Static link removal must reject retained dependencies.

## Services and transactions

`configure_ftn_downstream`, `configure_ftn_subscription` and
`configure_ftn_area_access` are audited expected-version CAS transactions.
Area access defaults to operator-only; explicitly open areas permit downstream
self-service. Manual subscriptions remain possible for restricted areas.
AreaFix cannot grant itself restricted access. All ordinary distribution reads
the committed subscription authority in the native message transaction.

Unsubscription stops future queue creation and holds unsent work for that
link/area. An already claimed delivery in the current AreaFix transport session
may complete: its offer predates the request. Manual edits reject active sessions
with a retryable conflict. Unclaimed work is held atomically. It preserves accepted deliveries, native messages and other links.
Disabled downstreams retain subscriptions and queue truth; no new fanout is
created for them and existing work is held. The separate **held** flag continues
queue creation while refusing exchange. Releasing a downstream hold clears that
link’s transport retry hold/backoff; per-item policy/restore holds still require
reviewed release. Other links continue independently.

AreaFix is private NetMail to a local AKA and `AreaFix` or `AreaMgr`. Only actual
authenticated BinkP ingress, exact configured remote source identity, an enabled
downstream policy and its write-only secret may authorize commands. Claimed From
addresses, manual packet handoffs and directory observations grant no authority.
The subject carries the secret; it must never enter operator projections,
audit, responses or diagnostic text. Original inbound packet custody remains
private evidence under existing artifact protection.

The conservative command grammar is `%HELP`, `%LIST`, `%QUERY`, `+AREA`, `-AREA`
and `%RESCAN AREA [R=count]`. Bare positive counts are accepted for the rescan
form. Blank lines and the FTN tear line are ignored. No wildcard, address target,
password change, shell/configuration interpretation or all-history rescan exists.
Requests are limited to 4 KiB and 32 commands. All commands and access checks
validate before mutation; rejection changes no subscriptions or rescan queue.
The transaction commits changes, safe activity, replay identity and a native
response NetMail with its normal outbound queue intent. Responses never quote
unvalidated input or the subject. A strong MSGID is required for mutation.

## Fanout, points and replay

Normal forwarding preserves native publication and FTN identity and queues one
delivery per eligible link. Ingress, source, PATH, SEEN-BY and retained target
decisions suppress reflection and duplicates. Point delivery never treats the
boss's two-dimensional SEEN-BY as a point acknowledgement. Same-zone N3 control
history remains the selected interoperability profile; cross-zone gateway
conversion is not inferred from two-dimensional history.

`rescan_ftn` selects only active public native deliveries already published in
the exact mapped AREA/conference. It preserves original publication/MSGID,
content and caller read state. A rescan request ID distinguishes intentional
per-target replay from normal delivery; queue retries reuse the same artifact.
Bounds are 1–500 messages per area, 1–1000 total per request, with a configured
cooldown of at least 60 seconds and at most one outstanding rescan per link.
Rescan requires both link and area permission and an active subscription.
It never selects private NetMail or implicitly bridges another network adapter.
FSC-0057.003's `RESCANNED <address@domain>` control marks each replay packet.
Inbound rescanned traffic preserves provenance but creates no normal onward
fanout; ordinary duplicate and PATH checks still apply. Rescanning previously
rescanned content replaces only the delivery marker, never MSGID/REPLY, Origin,
body or original native provenance. Received replay of an already held identity
is suppression, not a second native import. This is a selected proposal profile,
not a new FTSC standard or a global bypass of loop checks.

Queue receipts, partial success, hold/release and verified backup reconciliation
remain [N5 authority](network-operations.md). Accepted targets are not retried
because another target failed. Restores retain subscriptions, relationships,
request identities and queue work; no session is restored as authenticated/live.
No second FTN, hub or AreaFix message store exists. FileEcho, TIC, FREQ, hatching
and general scheduling remain outside N6.


## Schema 25 and implementation map

Migration 24→25 is transactional, including the per-target decision-table rebuild
and foreign-key validation. Existing rows gain delivery key `normal`; no old
queue, receipt, message or membership is fabricated. Fresh/24/rollback tests live
in `sf-core::database::tests`. A name conflict after the first new table rolls
back the complete migration. There is no N7 schema.

| Durable authority | Relation and owner |
|---|---|
| Dependent roles and policy | `ftn_downstreams`, core typed CAS; immutable link binding remains N3 |
| Downstream AREA membership | `ftn_subscriptions`; subscribed flag, manual/AreaFix source, per-association revision |
| Remote access and rescan permission | `ftn_area_access`, core typed CAS; absent row denies remote subscribe/rescan |
| AreaFix replay and safe results | `ftn_areafix_requests`, immutable link/MSGID receipt with no subject/body/credential |
| Explicit historical replay | `ftn_rescans` and `ftn_rescan_areas`, immutable request/area/count intent |
| Per-target delivery | existing `ftn_routing_decisions`, unique publication/link/delivery key |
| Current authenticated transport observation | session-scoped `binkp_link_health.authenticated`, cleared at begin/end/recovery |

AreaFix consumes a service NetMail envelope; it does not enroll a caller alias
or store its password-bearing subject in a native payload. Original packet
custody remains private evidence. Only the response is a native private message,
with no caller-readable local container at the hub. Its queue targets the exact
configured requester, not a route inferred from a claimed From address.

Strong request MSGID is mandatory. Recompressed/repacked retries with the same
link/MSGID and request content reuse the receipt and queue; changed content under
the same identity rejects. The replay fingerprint omits the subject so it cannot
be used as a stored password verifier. Retrying a previously rejected request
with corrected credentials requires a new MSGID. Missing authentication never
invokes subscription changes or rescan. Replies to wrong passwords are sent only
through an authenticated configured transport with the exact remote source;
spoofed and unauthenticated envelopes receive no reflected reply.

Credential files use the existing write-only private-file implementation under
an independent `areafix-credentials` namespace (1–71 printable ASCII bytes).
The runtime verifier supplies a constant-time MAC verification through the
existing cryptographic implementation. No secret field exists in relational
policy or projections. Credential updates invalidate stale workers. Cold backup
includes these files and restores private directory/file permissions, alongside
native SQLite/packet authority. Backups and raw packets remain private artifacts.

Core services and tests live in `sf-core/src/ftn/{hub,mail_hub,hub_tests}.rs`;
transport admission remains in `ftn/binkp.rs` and the existing daemon worker.
Protocol 1.10 advertises `FtnHub` after authentication. N1–N5 feature negotiation
remains compatible. Mutation actions reuse `ChangeSensitiveConfiguration` or
`NetworkRun`; all 21 capabilities still fit the bound 32 and bootstrap has no new
mutation authority. Static FTN/BinkP edits reject removal of retained downstream
references; disable/hold remains available. No pointlist overrides explicit policy.

Normal AreaFix mutation and safe result/response/receipt commit together. Capacity
or SQL failure rolls back all changes and permits retry. Up to 32 commands and
256 listed areas keep responses bounded. Contradictory duplicate edits to the
same area reject the whole request. Requests, rescans and ordinary delivery use
shared retained-history and queue budgets; exhaustion refuses admission rather
than forgetting accepted history. Histories are retained and paged (100 rows),
not destructively pruned. A full downstream queue can refuse a new atomic fanout;
it cannot silently drop that downstream's intent.

Safe operator projections include downstream/point policy, subscription source,
request authentication/result/counts, rescan requested/queued/accepted counts and
per-target route reason. Rescan progress is derived from queue receipts, not a
second completion flag. Private NetMail origin/final/next-hop and transit kind may
be shown; subjects and bodies remain absent. Releasing a pending/ready item after
a pre-build connection failure is an explicit audited retry; accepted items remain
ineligible. The existing verified recovery evidence matches queue/artifact/native
publication/link/policy before transferring later acknowledgements.

## Evidence and selected limits

FTS-0004.001 (EchoMail/SEEN-BY/PATH), FTS-4001.001 (real point addressing) and
FTS-0009.001 (MSGID/REPLY) remain primary normative references. FSC-0057.003 is a
proposal used for subject-password NetMail, +/- area changes and no-forward rescan
marking. The `%RESCAN AREA [R=count]` form is the bounded interoperable SBBSecho
practice independently exercised in M050. We deliberately reject wildcard edits,
remote credential/configuration changes and unbounded history requests. No full
FSC-0057 conformance claim is made. Native control preservation and same-zone
history remain N3's accepted profile; cross-zone EchoMail gateway transformation,
date-range rescans and automatic public point assignment are not implemented.

Run `cargo test --workspace` for synthetic/parser/native socket/recovery coverage.
The five-daemon test is `sf-bbs --test binkp five_daemon_hub`; the opt-in
`prepare_n6_operator_acceptance` prepares disposable boards when
`SPITFIRE_N6_EVIDENCE` names a private evidence directory. Independent peer assets
are deliberately external, not bundled fixtures. See M050 for exactly what was
independently observed and what remains a native-to-native proof.
