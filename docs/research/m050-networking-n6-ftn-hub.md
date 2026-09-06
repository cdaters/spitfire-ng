# M050 / N6 — FTN hub, AreaFix, rescan and point-boss services

Status: **COMPLETE / ACCEPTED** (2026-09-06 UTC).
This is the canonical N6 implementation and evidence report. The
[Sysop manual](../manual/ftn-hub.md) owns workflows and the
[Technical Reference](../technical/ftn-hub.md) owns exact service contracts.
[M044](m044-networking-foundation-gate.md) and accepted
[N1](m045-networking-n1-qwk-offline.md), [N2](m046-networking-n2-qwk-dove.md),
[N3](m047-networking-n3-ftn-core.md), [N4](m048-networking-n4-binkp.md) and
[N5](m049-networking-n5-operator-recovery.md) remain binding.

## Baselines and research authority — requested items 1–3

Private start: `3bba1f65ef5b8ecedf4384dfb5e0bba7e01e87a7`.
Previously accepted private source: `c79889ef793c3002999dc291f2264de145e28227`.
Public start: `49536465a0ea4520bedf2d65eb36afe35fa7cff3`.
The canonical private worktree started clean at the requested checkpoint.
Schema **24 → 25** adds only missing hub policy/subscription/replay authority.

Primary references were the indexed local revisions of
[FTS-0004.001](https://ftsc.org/docs/fts-0004.001),
[FTS-4001.001](https://ftsc.org/docs/fts-4001.001) and
[FTS-0009.001](https://ftsc.org/docs/fts-0009.001): EchoMail history, point
addressing and preserved message identity. [FSC-0057.003](https://ftsc.org/docs/fsc-0057.003)
is explicitly a **proposal**, selected for compatible conference-manager requests
and no-forward rescan marking; N6 does not claim complete proposal conformance.
Live FTSC retrieval returned HTTP 502; no latest-revision claim is made. The
private corpus remains **382 files / 8,599,056 bytes**, with every indexed hash
unchanged. No standards prose or private corpus is redistributed.

The bounded read-only SBBSecho review covered per-link distribution, AreaFix
request conventions, rescan/no-forward handling and point addressing. Decisions:

| Finding | Disposition |
|---|---|
| Subject-password NetMail and +/- area commands | ADOPT the FTSC proposal profile with actual authenticated link identity required |
| `%RESCAN AREA [R=count]` in SBBSecho | ADAPT a finite per-area/total subset; bare positive count is also accepted |
| RESCANNED prevents onward distribution | ADOPT as an explicit delivery exception; normal duplicate/PATH checks stay active |
| Broad remote config/password changes, wildcard/unbounded rescans | REJECT for this bounded service |
| Independent SMB/BSO ownership and configuration layout | PEER-SPECIFIC; no native storage, queue, UI or runtime dependency copied |
| Public point publication and automatic address assignment | DEFER; explicit local relationships remain authoritative |
| Terminal/transport profiles | Existing NG terminal behavior reused; no capability/encoding/emulation redesign or FireComm change |

## Implementation accounting — requested items 4–49

| Items | Result |
|---|---|
| 4–5 Architecture / roles | Native SPITFIRE messages remain canonical. Existing transit permission/routes, downstream distribution, subscription-management and point relationships compose without an exclusive role enum. BinkP remains transport only; SMB is unimplemented. |
| 6 Downstream model | Relational policy references the immutable N3 FTN binding and the existing N4 BinkP link. It owns enabled/held, optional boss AKA, AreaFix/rescan permissions, finite bounds and revision. No second endpoint or credential authority. |
| 7–9 Fanout / subscription authority / persistence | One native publication produces one normal delivery per eligible target. `ftn_subscriptions` separately owns downstream AREA membership, state, manual/AreaFix source and revision. Conversion from existing peer membership is atomic. SQLite/restart/backup retain it. |
| 10 Upstream/downstream flows | Upstream→subscribed downstreams, downstream→upstream/other downstreams and point→subscribed peers pass. No native conference duplication. |
| 11 NetMail routing node | Original FTN source and final destination remain separate from explicit next hop. Transit uses private native transit containers and never becomes local caller mail without a local destination and explicit mailbox alias. |
| 12–15 AreaFix architecture/auth/commands/response | NetMail to a local AKA and AreaFix/AreaMgr; exact configured downstream source, actual authenticated BinkP ingress, enabled policy and separate write-only secret required. HELP/LIST/QUERY, +/- AREA and bounded RESCAN are supported. Responses are native private NetMail to the configured requester and never echo the password. |
| 16 Area access | Default operator-only; explicit per-AREA remote-subscribe permission and separate rescan permission. Manual restricted subscriptions remain possible. No expression-language ACL. |
| 17–18 Atomicity / replay | Validate whole request before mutation. Changes, rescan intent, safe activity, native response and queue commit together. SQL failure rolls everything back. Strong link/MSGID receipt suppresses repack/replay; changed content rejects. No secret enters the receipt fingerprint. |
| 19–22 Rescan | Native active public messages in the exact mapped AREA/conference only. Per-area 1–500, total 1–1000, 4 KiB/32-command requests, cooldown 60–86400 seconds, one unfinished rescan per link. Request-specific delivery keys and RESCANNED controls preserve original identity and prevent onward fanout. No private export, duplicate native post or caller read-state change. |
| 23–27 Point boss | Explicit local node AKA/domain plus a nonzero point link matching that boss. True 4-D identity survives private transit and EchoMail. Point AreaFix can change only its own subscriptions. sfconfig adds a typed point relationship after its ordinary FTN/BinkP link is configured. No pointlist override/public assignment. |
| 28 sfconfig | Networks Hub menu provides downstream/point relationships, subscription edits, area access, bounded operator rescan and masked separate AreaFix credential update/clear. Typed review/save and revision conflicts preserve drafts. |
| 29–31 sfmonitor / activity | Networks section 8 shows downstream/point policy, queue counts, subscription source/revision, safe AreaFix results/counts/times and rescan requested/queued/accepted progress. No message-body view. |
| 32 Routing visibility | Queue details retain final/next-hop, normal EchoMail, authorized-rescan and AreaFix-response reasons; private transit is distinguished from ordinary NetMail. |
| 33–34 History / loops | N3 same-zone SEEN-BY and ordered PATH are preserved/updated. Ingress, asserted origin, seen/path nodes and retained per-target receipts suppress reflection. Points are never represented as fake 2-D nodes. Rescan markers suppress onward fanout without disabling ordinary duplicates or loops. |
| 35–36 Queue / partial success | Existing shared queue and BinkP M_GOT receipts remain authoritative. A/point accepted while B fails leaves only B pending/retry. Releasing B does not regenerate A/point work. |
| 37 Hold/release | Downstream hold keeps queueing and pauses only that link. Release clears its transport hold/backoff; per-packet policy/restore holds remain explicit. Pre-build connection failure can be retried through the existing audited release action. |
| 38 Disable/removal | Disable retains relationships/subscriptions/receipts and holds pending work; new fanout stops. Retained FTN and BinkP references cannot be silently removed/rebound. |
| 39 Subscription removal | Future normal fanout stops; unclaimed unsent work is held. Already claimed work in the current AreaFix session may finish. Accepted/native/other-link history is unchanged. |
| 40 Concurrency | Immediate SQLite transactions plus per-association/downstream/access CAS. Manual edits reject active sessions; stale manual edits after AreaFix and actual two-client UI edits fail explicitly, retaining the draft. |
| 41–44 Backup/restore | Exact subscriptions/source/revisions, point relationships, policies/credential files and fanout/rescan work survive. Actual cold restore retains accepted A/point and held pending B; reviewed B delivery completes once. Separate rescan test retains one accepted member and offers only two pending members. No fake active sessions. |
| 45–46 Privacy / secrets | Private transit and responses stay in native private authority; operator projections contain no subject/body. AreaFix credential updates reuse protected write-only files and redacted Debug/command receipts. Raw packets/backups remain private evidence. |
| 47–48 Authorization / audit | Existing 21 explicit capabilities fit bound 32; no bootstrap mutation grant. Sensitive config and NetworkRun checks remain current at dispatch. Manual policy/subscription/rescan/release actions are audited; remote requests produce safe operational events and immutable activity. |
| 49 Observability | Protected protocol 1.10 adds FtnHub discovery/gating, bounded pages and receipt-derived progress; no second log or completion authority. |

## Acceptance — requested items 50–68

Host: Apple Silicon macOS. Every native board, peer endpoint, packet, credential,
log and terminal capture was disposable/private. All sockets used host loopback
or a verified Docker **internal-only** network, with no published peer ports,
public FTN route or production/DDEV attachment. A raw byte relay has no FTN/BinkP
logic; both actual implementations own authentication, packets and acknowledgement.

Independent peer: pinned **Synchronet 3.19c**, **SBBSecho 3.15**, revision
**a5de4b9**, image digest
`sha256:8b5da5117126f31a4209fd1ca4101febd67c5a23655c9955b516b09685f66564`.
Its GPL-family source/configuration/message stores remain external private
acceptance material. No peer implementation source was copied into NG.

| Items | Evidence |
|---|---|
| 50 Independent interoperability | First BinkP exchange: peer sent two SBBSecho-generated packets (EchoMail and AreaFix), received hub EchoMail. Native hub forwarded the independent post to another native peer. Second exchange replayed both inputs and received four packets: AreaFix response, two bounded rescan packets and native-peer EchoMail forwarded through the hub. Independent native MsgBase verification confirmed hub origin `10:100/1`, forwarded origin `10:100/3`, the subscription/rescan response and secret absence. One AreaFix receipt, one rescan request, two peer-accepted rescan members remained. Existing-history rescan messages were independently suppressed as duplicate/circular rather than posted twice. Independent point-boss coverage was not added here; native five-daemon acceptance owns that proof. |
| 51–52 Native/macOS journey | Five real native daemons: hub, upstream, A, B and point. Two areas; three dependent subscriptions; both-direction EchoMail, no reflection, true point origins, both-direction point NetMail, private access denial, valid/wrong AreaFix, +/- area behavior, bounded rescan, B outage/retry, hold/release, graceful stop/restart and cold restore with accepted-versus-pending fanout all pass. Real sfconfig/sfmonitor PTYs verify masked BinkP/AreaFix updates, hub policy, two-client CAS, activity/route reasons and terminal flags/alternate-screen restoration. |
| 53–57 N1–N5 regressions | Full workspace covers caller QWK, QWK/DOVE networking, FTN packets/routing/provenance, real BinkP and N5 verified recovery/operator controls. Prior acceptance remains intact. |
| 58 Tests added | Nine focused hub tests cover subscription/fanout, point/no-reflection, authentication/parser/atomicity/replay/CAS, access/disable, privacy, rescan/no-forward and partial rescan/fanout recovery. Schema-25 rollback/retained-identity test; actual five-daemon journey; one explicit opt-in private preparation helper. Existing credential-redaction test extended. |
| 59 Workspace total | Final gate accounting is recorded below. |
| 60 Localization | en-US **1.23.0 / 1,249 messages**, 32 implemented-surface strings added. Presentation profiles and terminal capabilities are unchanged. |
| 61–63 Documentation | New practical [manual](../manual/ftn-hub.md), canonical [technical contract](../technical/ftn-hub.md) and this report; current state, roadmap/milestones, decisions/session log and relevant security/configuration/recovery/index documents reconciled. |
| 64–66 Quality / doctests / links | Exact final gate results recorded below. |
| 67 Privacy/provenance | Corpus hashes unchanged; private raw packets, secrets, peer sources/configuration, captures and private history excluded from publication. No body/credential in NG operator projections/audit/events. Independent peer diagnostic subjects were suppressed/redacted in private harness logs. |
| 68 cargo-audit | Availability is reported below; no clean security-audit claim is inferred from an unavailable tool. |

The initial sandboxed workspace attempt failed local socket creation with
`Operation not permitted`; the authorized unsandboxed loopback run passed. The
five-daemon test found an ordinary pre-build retry defect: a failed connection
left pending work that the release action rejected. Explicit audited release now
covers pending/ready work without permitting accepted-work resend. Test fixtures
were corrected to freeze artifacts before later-acknowledgement reconciliation
and to distinguish point-AKA PATH behavior. The RESCANNED proposal control was
added to the bounded codec's recognized space-delimited controls. These were
fixed and retested, not accepted exceptions.

## Final gates and private publication — requested items 69–73

Private gates: **664 passed / 0 failed / 6 opt-in ignored**, all **8 doctest
suites**, **129 source headers**, fmt, Clippy with warnings denied, diff and
**174 Markdown/local-link documents** pass. cargo-audit is unavailable.
Privacy/secret/provenance checks pass; all private corpus hashes are unchanged.
Real macOS, independent peer and native five-daemon evidence above are accepted;
all disposable resources shut down cleanly. **N6 COMPLETE / ACCEPTED.**
Accepted private source: `03ff31f724bbd66ba5e47a338928c9a969052590`, pushed,
aligned with private origin/main and clean before public synchronization.
Implementation commit: `Complete FTN hub semantics`. Controlled scope
creep includes per-link queue/subscription metadata, safe retained activity,
downstream hold/disable, explicit route reasons and operator rescan. Subscription
export, rescan preview/date ranges, NodelistDB lookup and public point publication
remain deferred. Same-zone EchoMail is the selected profile; no cross-zone gateway
transformation is inferred from 2-D history.

## Sanitized public synchronization — requested items 74–88

Public starts at `49536465a0ea4520bedf2d65eb36afe35fa7cff3`, with independent
history. Accepted private source: `03ff31f724bbd66ba5e47a338928c9a969052590`.
Published **8 added / 41 updated files**: five new shared source/test/schema files,
three canonical documents, existing FTN/BinkP/operator/localization implementation
and reviewed public status, roadmap, security, architecture and recovery docs.
All **23 shared implementation/catalog files are byte-identical** to private source.
Cargo.lock is unchanged and differs from private only by its two excluded research
package records. Schema **24 → 25** migration is included and tested in this public
build; hub, subscriptions, AreaFix, rescan and point-boss services are all included.

Public gates: **602 passed / 0 failed / 6 opt-in ignored**, all **6 doctest suites**,
**108 source headers**, fmt, Clippy with warnings denied, diff and **120 Markdown/
local-link documents** pass. Public privacy/secret/provenance scans verify no added
host paths, acceptance secrets, private corpus hashes or proprietary artifacts.
Private corpora/index, nodelists, peer source/configuration, credentials,
packets/logs/screens, proprietary CircuitNET material and private Git history are
excluded. No public Git object history was imported from the private repository.

The final accepted native binary repeated a bounded independent rescan after the
SEEN-BY refinement: one new operator request queued two members only to the
independent link; both received genuine peer acknowledgements. Replayed inbound
AreaFix retained its one existing request receipt. SBBSecho again recognized the
historical EchoMail identities without duplicate EchoMail posting. The isolated
hub and independent container shut down cleanly afterward.

The public publication commit introduces this report in independent public history.
Its exact hash and final push/alignment are recorded in the private canonical
closure; no self-referential commit hash is fabricated in this file.

## Boundaries — requested items 89–104

| Items | Result |
|---|---|
| 89 | Live public FidoNet traffic: **NONE**. No membership/production address claim. |
| 90–94 | FileEcho, TIC, FREQ, hatching and general scheduler: **UNTOUCHED**. No N7 schema/services. |
| 95–97 | CircuitNET implementation, B-022 and doors: **UNTOUCHED**. |
| 98–100 | DDEV, production and FireComm: **UNCHANGED**. |
| 101 | Private corpora and independent acceptance assets: **UNPUBLISHED**. |
| 102 | Release/tag/binary distribution and OS service packaging: **UNCHANGED**. |
| 103 | Windows live acceptance: **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**. |
| 104 | Exact next action: stop after accepted N6 source publication. A subsequent milestone requires a separately scoped pass; no N7 or excluded work begins here. |

N1–N5 remain accepted. Native SPITFIRE message authority remains canonical; BinkP
remains transport only. Routing-node, EchoMail hub and point-boss behavior preserve
real FTN identity, authoritative durable subscriptions and per-target queue truth.
AreaFix is authenticated and bounded; rescan is bounded and privacy-safe. Partial
failures do not duplicate successful deliveries; backup/restore preserves hub
truth. Private NetMail bodies and secrets remain absent from operator surfaces.
No public FidoNet traffic or excluded feature/release/production work occurred.
