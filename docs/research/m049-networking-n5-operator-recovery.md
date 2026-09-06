# M049 / N5 — networking operator cockpit and recovery

Status: **COMPLETE / ACCEPTED** (2026-09-06).
This is the canonical rights-safe N5 implementation and acceptance report.
[M044](m044-networking-foundation-gate.md), [N1](m045-networking-n1-qwk-offline.md),
[N2](m046-networking-n2-qwk-dove.md), [N3](m047-networking-n3-ftn-core.md) and
[N4](m048-networking-n4-binkp.md) remain binding and accepted.
The [Sysop manual](../manual/network-operations.md) owns operation instructions;
[Technical Reference](../technical/network-operations.md) owns exact interfaces,
limits and recovery semantics.

## Baseline and authority

Private starting checkpoint: `9d23ff361968c70d22c48a56753b0b795206c47f`.
Accepted N4 private source: `d95b853a17dc73cc358c49ec7c794b9a6882d27f`.
Public starting checkpoint: `d98bbede2a8ceb4a86afffc5632873642156def7`.
The private worktree started clean and HEAD equaled origin/main and the requested
checkpoint. Schema **24 → 24**: no UI-driven migration or speculative table.
Category B remains **15 VERIFIED / 2 IMPLEMENTED / 3 PARTIAL / 5 NOT STARTED**;
B-021 VERIFIED, B-022 NOT STARTED.

Native SPITFIRE message authority remains canonical. FTN provenance, scanner,
tosser, routing, per-AKA serials, duplicate/loop history, directory generations and
the shared outbound queue retain their owners. BinkP is transport only; SMB remains
unimplemented. N5 changes operability and verified restore reconciliation, not wire
message semantics. FTSC remained primary technical authority.

The local primary [FTS-0009.001](https://ftsc.org/docs/fts-0009.001), dated
17-Dec-1991, was read for originating identity, eight-hex serial uniqueness,
MSGID/REPLY preservation and its three-year uniqueness requirement. M044's
monotonic retained-floor/hold policy is the project-specific implementation choice;
the standard itself leaves serial generation to the implementer. N3/N4's already
accepted packet and BinkP standards remain unchanged; their reports retain the
complete standards inventory. No large standards passages are reproduced.

Secondary references remained read-only:

| Reference | Bounded use and decision |
|---|---|
| Existing Synchronet Fido docs / N4 report | ADOPT existing Sysop distinction between identity, credentials, link status and packet custody; no new protocol semantics inferred |
| Synchronet `exec/load/binkp.js` | ADAPT documented authentication callback/API use in the private independent harness, including an explicit protocol rejection; no source/algorithm/UI copying |
| NodelistDB OpenAPI metadata, OpenAPI 3.0.3 / API 1.0.0 | DEFER optional client. No endpoint was called; external information is neither local directory nor routing authority |
| FireComm profile/troubleshooting reference | ADAPT geometry/encoding diagnostics and separation of presentation from binary transport; preserve existing NG terminal guards; no FireComm edits or dependency |
| CircuitNET surviving evidence | ROADMAP ONLY: approved future independent binary/file-format study; no new archaeology or implementation in N5 |

## Implementation accounting

The numbering below corresponds to the requested private/public/boundary report.

| Items | Implemented result |
|---|---|
| 1–3 | Baselines above; schema unchanged at 24 |
| 4 | Protocol 1.9 bounded Networks projection over accepted authority; separate static and relational CAS; no new message/queue/log authority |
| 5 | Overview: configured QWK/FTN counts and enabled state, listener policy, queue states, quarantine/directory counts and compact durable counters |
| 6–7 | FTN/BinkP and QWK link rows/details: domains/AKAs, directions, endpoint source, secret status, current/held state, timestamps, latency, last authenticated addresses/capabilities, queue count |
| 8–10 | Confirmed typed Test/Poll; Test admits no mail. Poll starts bounded daemon work; link/queue projections distinguish initiation from final peer custody. Semantic authentication/address/connection/timeout/custody guidance |
| 11–13 | Queue pages/details include safe IDs, adapter/type/link, final destination/next hop, route/provenance, age/attempt/retry/artifact metadata. QWK hold added as closed CAS; existing QWK retry and FTN hold/release reused. No arbitrary cancellation/deletion |
| 14 | QWK/DOVE and EchoMail mappings visible; private NetMail is not an area |
| 15–17 | Directory generations/date/activation/source/priority/encoding class, counts/issues/staleness; configured pointlist/nodelist sources; bounded typed domain+node/point lookup |
| 18–19 | NodelistDB client deferred; no external dependency, call, cache or authority override |
| 20–21 | Privacy-safe quarantine summaries; reprocess/discard deferred because N3 has no accepted deterministic lifecycle for them |
| 22 | Existing import/duplicate/loop/custody and queue/directory/quarantine counters; no global analytics |
| 23–30 | sfconfig Networks forms for FTN domains via typed AKAs, primary/enabled identities, links/BinkP, EchoMail maps, exact/direct/boss/net/zone/default routing policy, directory sources, listener/retry/auth settings and QWK/DOVE partners/maps. Conference selection by name; referenced identity/source validation prevents orphaning |
| 31–32 | Existing effect classes retained: listener restart, link policy subsequent work/stale-session cancellation, relational maps live. Static aggregate CAS and separate mapping/partner CAS; real two-client conflict retains draft |
| 33 | Safe queue reasons, finite link diagnostics, local source/point visibility and explicit stopped-board recovery flow |
| 34–36 | Restore invalidates BinkP sessions/claims and daemon generation, retains directory/source provenance/priority and snapshot duplicate history, holds uncertain work, preserves matching proven acceptance |
| 37–38 | Verified monotonic FTN serial floor, source retirement, matching later accepted-attempt reconciliation; replacement and new-root hard collision journey passes |
| 39–43 | No private body/subject/path or credential in projections, review, health, audit or events. Existing explicit 21 capabilities fit bound 32; bootstrap remains read-only. Typed actions/configuration use existing audit and observability |
| 44 | Graceful native shutdown, queue persistence, restart and new-generation recovery tested; terminal flags and alternate-screen restoration checked |
| 45 | Controlled independent Synchronet BinkP/SBBSecho regression passes; no live public network connection |
| 46–49 | N1/N2 QWK and N3/N4 FTN/BinkP regressions remain part of full workspace acceptance |
| 50 | CircuitNET planned preservation/revival and independent reverse-engineering policy recorded; no implementation or proprietary redistribution |
| 51–52 | Focused additions and final workspace totals are recorded in the gate section below |
| 53 | en-US **1.22.0 / 1,217 messages**, 146 added implemented-surface strings |
| 54–56 | New practical manual, technical contract and this canonical report; existing architecture/security/recovery/configuration/localization/index documentation reconciled |
| 57–60 | Required gates, links, privacy/provenance and cargo-audit availability recorded below |
| 61–65 | Final status, scope decisions, private commit/push/alignment recorded at closure below |

Static domains remain part of N3 typed AKA/link/source endpoint policy; N5 does not
invent a parallel domain table. Ingested source parsing/priority profiles are
immutable under their stable ID; correction uses a new source ID while disable
remains possible. Existing explicit native recipient aliases/private service
semantics are unchanged; polished caller NetMail composition is not part of N5.
QWK artifact handoff remains the accepted controlled manual workflow, with no new
QWK transport or general scheduler. History browsing, full hub workflows and
quarantine mutations are deliberately absent rather than empty UI sections.

## Real macOS and independent acceptance

Host: **Apple Silicon arm64, macOS 26.6.2 (25G83)**. All boards, peer credentials,
addresses, artifacts, logs and screenshots are disposable private acceptance
material excluded from publication. The peer uses the already cached pinned
Synchronet image and an internal-only container network. A loopback byte relay
carries bytes without FTN/BinkP semantics; no public network routes or credentials
were configured. Only the native daemon owns SPITFIRE mailer sessions.

Independent peer: **Synchronet 3.19c**, BinkP JavaScript revision 4,
**SBBSecho 3.15**, compiled image revision **a5de4b9**, pinned image digest
`sha256:8b5da5117126f31a4209fd1ca4101febd67c5a23655c9955b516b09685f66564`.
The distribution identifies Synchronet's GPL licensing and SBBSecho GPL-2.0-or-later;
no peer source/configuration is part of NG publication. The peer was built earlier;
N5 used the cached image, not copied source or a new runtime dependency.

The real terminal journey used sfmonitor and two sfconfig clients attached through
PTYs to the actual disposable macOS daemon. Captured screens were rendered and
visually inspected; no secret or private message body was present. It covered:

- Networks overview, FTN link health and a newly configured DOVE-compatible QWK
  partner; all seven sections, queue details, EchoMail mappings, active pointlist
  provenance, local point lookup and safe malformed-packet quarantine summary.
- A masked credential update through the new Networks form, immediate status,
  named link/EchoMail/route/directory-source edits, listener effect review and
  two simultaneous sfconfig clients. The stale second save failed with its draft
  retained, without overwriting the first client.
- Test Link against the independent answerer: successful authentication and point/
  multiple-AKA negotiation, **zero files sent/received**, two native queued items
  unchanged. Queue hold and release were exercised through the cockpit.
- Poll Link through sfmonitor: independent BinkP received two packets; native queue
  became accepted only after M_GOT. SBBSecho imported native private NetMail and
  EchoMail, checking point source `FMPT` semantics, private attribute and body.
- Wrong authentication and wrong presented address: fail closed, no mail custody,
  understandable semantic diagnostics. The original private secret was restored
  through the masked form. Last authenticated addresses remain explicitly labeled
  historical observations after a rejected identity.
- Actual sfmonitor-to-sfconfig handoff and return; conference selection by name;
  offline new-root recovery form with explicit TRANSFER confirmation, source
  retirement, restored status and independent post-restore poll with zero resends.
- Graceful daemon stop and queue truth; terminal ECHO/canonical/output flags and
  alternate-screen exit passed for both sfconfig clients and sfmonitor.

Initial disposable peer setup lacked stock configuration and its user directory;
these were initialized inside the isolated peer. An initial peer NetMail toss
could not resolve the synthetic recipient; adding that local recipient and
re-tossing delivered it privately. Neither defect changed NG message semantics.
The first negative peer callback closed without a protocol rejection, correctly
producing interruption; the harness then emitted the peer library's documented
M_ERR so authentication guidance could be verified. No wire implementation was
copied or modified to manufacture interoperability.

### Hard restore collision proof

The automated real two-daemon macOS journey uses native authoring/scanner/queue,
real BinkP sockets and a real peer native database, not mocked transport:

1. Queue point NetMail and EchoMail; Test transfers no mail; hold/release and Poll
   deliver two native messages. Typed QWK configuration and all Networks reads work.
2. Stop the board, originate two more messages and freeze their queued artifacts;
   take a cold backup. Restart, send those artifacts, originate/send two more,
   and record the surviving per-AKA serial floors.
3. Restore the older backup over the stopped original. Verified later floors and
   matching peer acknowledgements reconcile before replacement. Accepted work
   stays accepted. New NetMail/EchoMail reaches the peer as messages seven/eight,
   not duplicates and not re-sends of the older frozen artifacts.
4. Restore the older backup into a new root. Origination initially stays held.
   Offline recovery locks/retire-disables the surviving source and transfers its
   verified floors/acceptance. Two new messages reach the peer, for **ten unique
   native imports** across the journey. Directory generation and secret/queue state
   survive; no session is restored as active; normal shutdown succeeds.

Core regression additionally captures five frozen queue items before backup,
records five true acceptances and eight later identities, reconciles the restored
copy, proves the next MSGID serial is `0000000e`, and proves replay cannot rewind
it. A held source cannot authorize clearing an origination hold. Existing N3/N4
restore and duplicate/retry tests remain required.

This is deliberately not a claim that an old backup contains all later history.
Missing/uncertain source evidence leaves holds. Only matching frozen artifacts
inherit later acknowledgement; unrelated newer messages/inbound history are not
copied. A lost post-backup inbound duplicate receipt cannot be invented. The
operator must not run independent clones of one AKA. These explicit limitations
are part of the safe recovery contract, not hidden counter-reset behavior.

## Tests and final quality gates

Focused additions cover core verified floors/acceptance/replay/held-source failure,
projection privacy and typed point destination, retained-reference conflicts;
QWK queue hold/stale-version/retry in the established restart journey;
network confirmation invalidation and explicit reconnect after daemon loss, navigation and terminal
bounds/control filtering; named route/point/domain forms, added/removed record
review, conference input, and lookup text containing `q`; and the real two-daemon
configuration/CAS/restore collision journey. An explicit ignored fixture prepares
only a disposable operator-acceptance board; private peer assets are not tests
published in the workspace.

Final private source: **653 passed / 0 failed / 5 opt-in ignored**, including
**13 added passing tests** plus expanded existing QWK hold/retry coverage and one
new explicit ignored operator-fixture preparation. Doctests pass. **125 source
headers**, fmt, workspace Clippy with warnings denied, diff hygiene and **171
Markdown/local-link/anchor/fence documents** pass. cargo-audit remains unavailable
(`cargo audit` is not installed). Privacy checks cover 71 private screen/log
projections and five final clean terminal closure records. All **382 indexed
corpus file hashes are unchanged**; Finder metadata is not research corpus content.

An initial full run caught Networks discovery ordering inconsistent with the
existing feature-list contract. The append order was corrected and final full
workspace runs passed. Terminal checks corrected submenu heartbeat/status refresh,
complete added-field review, lookup `q` handling, missing lookup feedback and
explicit disconnected reconnect. Private harness Escape timing, atomic command
files and terminal cursor-position responses were corrected before final handoff
acceptance; no terminal protocol implementation was changed.

The source remains independently authored; no FTSC/Synchronet/NodelistDB source,
private peer assets or proprietary CircuitNET material was copied into it.
Disposable native daemons, peer and acceptance terminals are stopped after the
final checks. The accepted private milestone **Complete networking operator cockpit** is
`c79889ef793c3002999dc291f2264de145e28227`, pushed/aligned with a clean worktree
before public synchronization. Public source/gates are recorded below; the commit
publishing this report supplies the independent public history checkpoint.
The required commands are `ruby tools/verify-source-headers.rb`,
`cargo fmt --all --check`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` and `git diff --check`.
Tests include doctests. Cargo uses cached dependencies in offline mode while
socket acceptance is authorized only for isolated loopback peers. Markdown/local
links, private-content/secret/provenance scans and clean Git alignment are separate
gates. Genuine Windows live acceptance remains deferred under standing policy;
no long Windows CI or VM work was performed.

## Publication accounting and boundaries

Items 66–81: sanitized source is based directly on public
`d98bbede2a8ceb4a86afffc5632873642156def7`, copied from accepted private
`c79889ef793c3002999dc291f2264de145e28227` after its push/clean verification.
Public gates pass: **591 tests / 0 failures / 5 opt-in ignored**, doctests,
**104 source headers**, fmt, workspace Clippy with warnings denied, diff hygiene,
**117 Markdown/local-link/anchor/fence documents**, and privacy/provenance scans.
Windows live acceptance remains deferred under the standing policy.

Published **8 added / 46 updated files**. Five new independently written modules
provide core projection/recovery, daemon projection, sfmonitor and sfconfig
surfaces; three new documents provide the manual, technical contract and this
report. Existing source/catalog/manifests and 26 public documentation/status files
are updated. **24 shared source/catalog/manifest files are byte-identical** to
private source; Cargo.lock differs only by removal of the two private research
package records. Schema remains 24 with no migration. Networks/recovery UI,
public-safe tests, en-US 1.22.0 and CircuitNET's future research/roadmap note are
included. NodelistDB client code is absent because the feature is deferred.

The publication commit introduces this report in the independent public history;
its exact hash and final push/alignment are recorded in the private canonical
closure. No private Git objects/history are imported.
Private history, standards/wiki/source corpora, proprietary CircuitNET material,
peer configuration/credentials/identities/packets/logs/screens and uncertain-rights
nodelists are excluded. Optional NodelistDB code is absent because it was deferred.

| Items | Boundary |
|---|---|
| 82 | Live public FidoNet traffic: **NONE** |
| 83–85 | AreaFix, FileEcho/TIC/FREQ and general scheduler untouched |
| 86 | CircuitNET implementation/codec untouched; roadmap/research policy only |
| 87–88 | B-022 and doors untouched |
| 89–91 | DDEV, production and FireComm unchanged |
| 92 | All private corpora unpublished; secondary references read-only |
| 93 | No release, tag, installer, package or binary distribution changed; local development executables only |
| 94 | After N5 acceptance/publication, stop. The next separately scoped action is N6 hub/operator semantics under M044: downstream subscriptions/AreaFix/rescan/point-boss administration. It does not start in N5. |

N1–N4 remain accepted. Test/Poll stay typed and bounded; queue actions preserve
native authority. Secrets are not exposed. Optional external intelligence is
non-required and unimplemented. CircuitNET revival is planned but unimplemented.
No public FidoNet membership or production use is claimed. Windows live networking
acceptance remains **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**.
