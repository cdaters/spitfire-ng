# M047 / N3 — native FTN core

Status: **COMPLETE / ACCEPTED; sanitized public source** (2026-09-05).
This is the canonical N3 report. Native SPITFIRE NG messages remain canonical.
FTN is an adapter around that authority. **BinkP is not implemented; no live
public FidoNet participation is claimed.**

## Checkpoints and scope

Private start: `01520369150df51254972099d6a07296328835c9`.
Public start: `d58a5c28f7caf4d02ba4a19b531d4b7283be876f`.
Schema **22 → 23**. M044 remains the binding architecture; N1 and N2 remain
accepted. Category B is unchanged: **15 VERIFIED / 2 IMPLEMENTED / 3 PARTIAL /
5 NOT STARTED**, B-021 VERIFIED and B-022 NOT STARTED.

N3 implements native NetMail/EchoMail, points, domains, multiple AKAs, packet
interchange, scanning/tossing, routing, durable queues/receipts, loop prevention
and effective directory authority. It does not implement transport. The
[Technical Reference](../technical/ftn-core.md) owns exact interfaces, schema,
security, limits and recovery; the [Sysop manual](../manual/ftn-core.md) owns the
usable isolated configuration and manual exchange procedure.

## Evidence and authority

The refreshed private corpus contains 381 physical files / 8,534,385 bytes,
excluding macOS metadata: the original 182 files plus 199 newly collected
Synchronet documents/assets. The complete private index records per-file title,
revision/date where discoverable, authority, feature, classification, provenance
and digest. Some web assets have no document revision/date; these remain unknown.
The original corpus is unchanged. No standards text, downloaded wiki document,
source reference, real list sample or acceptance packet is redistributed.

Primary technical authority remained FTSC. The selected documents actually used:

| Document | Use / N3 classification |
|---|---|
| FTS-0001.016 | REQUIRED: Type 2 header, packed message framing, attributes and timestamp |
| FSC-0048.002 | REQUIRED selected interoperability profile: Type-2+ capability/header/point encoding; proposal status retained |
| FSC-0039.004 | Compatibility evidence for validated extended header behavior; not an authority over FTS |
| FTS-0004.001 | REQUIRED: EchoMail AREA, origin, tear line, SEEN-BY and PATH |
| FTS-0009.001 | REQUIRED: MSGID / REPLY identity |
| FTS-4000.001 | REQUIRED: control syntax and preservation boundary |
| FTS-4001.001 | REQUIRED: INTL / FMPT / TOPT NetMail addressing |
| FTS-4008.002 | REQUIRED selected timezone metadata: TZUTC |
| FTS-4009.001 | Relevant Via routing-history evidence |
| FTS-5000.005 | REQUIRED directory profile: full nodelist hierarchy/fields/CRC |
| FTS-5001.006 | Directory Internet flags and endpoint metadata; no endpoint connection |
| FTS-5002.002 | REQUIRED point foundation: preferred Boss and combined Point lists |
| FTS-5003.001 | Charset declarations and conversion policy |
| FTSC product list .020 | Informational: unallocated product ID 00FE |
| FTA-1004.025 | Informational pinned document status/catalog; not a latest-revision guarantee |
| Policy 4 | Governance/history only; no technical override |

Type 2 and selected Type-2+ are N3 profiles. Type 2.2, fake-net/FidoUser list
compatibility and NODEDIFF remain later; Type 1/3 are rejected. Poss is deprecated.
The corpus's FTS/FSC/FTA/FRL status distinctions are retained. Live FTSC retrieval
failed, so no newly verified current-revision or universal-variant claim is made.
The pinned primary bytes and independent interoperability provide the technical
basis. No large passages are copied into documentation.

Downloaded Synchronet references reviewed include FidoNet setup/terminology,
packet format descriptions, SBBSecho use and configuration, BinkIT workflow,
NetMail/EchoMail, AKA/area/link expectations, routing and nodelists. BinkIT is
workflow context only. Review of the independent source checkout was bounded to
FTN address helpers, packet dump/definitions, selected SBBSecho packet/message/
point/SEEN-BY/PATH/route sections, and public JS configuration/message APIs.
Reference checkout: `60b70e526213285b61ae11f82dc8412ad5b405b4`.

NodelistDB reference checkout: `a3ad2bc6114d4ec8bcca846bdac32852e9e2e9dd`.
Bounded review covered pointlist documentation, source-format/context parsing,
node metadata, source provenance and operational health concepts. Both checkouts
remained read-only; neither is a build/runtime dependency. No implementation,
algorithm, comment, UI or documentation text was copied.

| Reference lesson | Classification / application |
|---|---|
| SBBSecho packet sender differs from original message source/final destination | ADOPT interoperability requirement, independently implemented from FTSC |
| Separate AKA/link enrollment and explicit EchoMail subscriptions | ADAPT operator workflow to TOML plus native relational policy |
| Native scanner/tosser, independent packet decoder and source charset metadata | ADOPT engineering lesson, with native SPITFIRE ownership |
| SBBSecho calls node extended packets Type 2e and sentinel point packets Type 2+ | Synchronet-specific terminology; ADAPT only validated wire semantics |
| BSO/FLO filenames, SMB, external tosser-owned message bases and AreaFix behavior | REJECT as SPITFIRE authority/dependency; no copying |
| BinkIT/BinkP sessions, endpoints and polling | DEFER to N4 |
| Point source/boss context and raw provenance | ADOPT directory lesson; typed domain/address keys |
| Source priority, complete candidate validation and atomic generations | ADAPT to SQLite relational authority |
| Duplicate/conflicting entries and stale observations | ADOPT explicit issues, quarantine, priority and cadence policy |
| Historical source corrections | ADAPT immutable generations and explicit activation; no in-place rewrite |
| CP437/CP866 and domains | ADAPT explicit source encoding and separate namespaces |
| First-file-wins, heuristic year repair or sign stripping | REJECT silent interpretation |
| ClickHouse/global analytics, probing and external directory runtime | REJECT dependency/architecture; DEFER live health/probing |

## Implementation accounting

The following table maps the requested private report items to their implemented
outcomes. Detailed byte rules and SQL relations are canonical in the Technical
Reference rather than duplicated here.

| Requested items | Result |
|---|---|
| 1–3 checkpoints/schema | Starts above; transactional schema 23 |
| 4–9 corpus/references/hierarchy | Refreshed inventory and primary/secondary findings above; rights-safe summaries only |
| 10–13 address/point/domain/AKAs | Validated 4D Address + Domain/Endpoint; canonical string serde, ordering/hash keys; node/point AKAs, primary per domain; multiple namespaces |
| 14–17 NetMail/local/transit/privacy | Native private containers; explicit active-recipient aliases; source/final point addresses; final destination retained through next-hop routing; no transit reader or body diagnostics |
| 18–19 EchoMail/mappings | Explicit typed AREA-to-native-conference mapping, AKA/origin/send/receive policy and subscribed links; no auto-created areas |
| 20–24 packet/control codecs | Bounded Type 2/Type-2+ parsing/serialization; typed controls; raw unknown controls preserved outside body |
| 25–28 MSGID/REPLY/addressing/history | Native serials and safe reply binding; INTL/FMPT/TOPT; bounded SEEN-BY/PATH; point identity separate from 2D boss history |
| 29–33 duplicates/loops/scanner/tosser/idempotence | Durable domain/scope identity and receipts, content-collision quarantine, native scan publications, transactional toss and onward queues; exact retry never creates another native message |
| 34–37 routing/queue/manual exchange | Documented precedence and fail-closed links; distinct final/next-hop keys; common N2 queue registry; authenticated manual injection/build/extraction |
| 38–45 directory | Full nodelists and Boss/combined pointlists; source/profile/date/digest provenance, priority, candidate validation, atomic activation, conflict quarantine, staleness and typed effective lookup; explicit encodings |
| 46–47 quarantine/security/bounds | Generic private artifact custody extended with FTN link context and finite reason classes; resource bounds and native access checks |
| 48–51 encoding/time/presentation/attributes | Strict ASCII/CP437/CP850/CP866/UTF-8, CHRS; original wall time plus TZUTC/receipt UTC; configured local origin, separate tear/origin metadata; unsupported routing/file/security attributes rejected |
| 52–55 config/monitor/events/audit | sfconfig typed policy import; protected minor-7 IPC actions; minimal read-only counts/status; body-free operational events and audited operator policy/actions |
| 56–57 recovery/migration | Native backup/restore preserves all durable FTN relations/artifacts/policy; unsent work and serials held; manual input slots excluded; schema-22 populated QWK queue and migration-failure tests |
| 58–67 independent and real acceptance | Peer and journeys below; native private delivery, point fields, transit, EchoMail and real forwarded-loop suppression |
| 68–69 QWK regression | N1/N2 codecs, native private access, real daemon manual exchange and online message regressions remain green |
| 70–72 tests/localization | Address/codec/control/charset/directory/core/migration/daemon tests added; en-US 1.20.0; final totals below |
| 73–75 documentation | New Sysop manual, Technical Reference and this canonical M047 report; architecture/privacy/recovery/index/handoff updates |
| 76–79 quality/privacy/audit | Final gate record below; cargo-audit availability reported explicitly |
| 80–81 N3/N4 | Working message semantics before transport; no BinkP. N4 can consume typed AKAs/links/addresses, immutable queued packets and receipts |
| 82–83 private Git | Logical implementation/publication closure commits; final alignment recorded below |

## Independent interoperability and macOS acceptance

Independent tool: Synchronet **3.19c-Linux HEAD/a5de4b9**, SBBSecho
**3.15-Linux HEAD/a5de4b9**, pktdump **revision 1.18**. SBBSecho is GPL-2.0-or-later;
it was executed as an independent peer only. The cached image is pinned to
`sha256:8b5da5117126f31a4209fd1ca4101febd67c5a23655c9955b516b09685f66564`.
The peer ran in a disposable AMD64 container on the Apple Silicon macOS host,
with Docker network mode **none**, no published ports, and only a private temporary
exchange mount. SPITFIRE itself ran natively on macOS. This is not a Linux or
Windows SPITFIRE runtime acceptance claim.

The peer's own CNF configuration API enrolled synthetic node/point AKAs and two
areas. Its native MsgBase API authored NetMail/EchoMail/transit, and SBBSecho
created their packet bytes. No custom reference-derived packet generator was used.
SPITFIRE's native authoring services generated outbound packets; pktdump decoded
them and SBBSecho tossed them into its native mailbox/conference storage.

Observed independent results:

- NetMail `10:100/1.3 → 10:100/2.9`: private attribute, FMPT/TOPT, source point,
  destination point, MSGID `10:100/1.3 00000001`, UTC written time, UTF-8 CHRS and
  exact human body retained in the peer's native mailbox.
- Independent NetMail addressed to local point `10:100/1.3` became a private
  native SPITFIRE message; a different caller could not read it. Re-ingestion
  suppressed duplication.
- Independent transit to `10:200/9.8` was tossed into native transit authority and
  routed to next hop `10:100/4`; pktdump accepted the resulting native packet.
  The packed final destination and TOPT remained unchanged.
- Native TEST2 EchoMail became an independent native conference message with
  AREA, MSGID, original written UTC, human body, SEEN-BY and PATH retained.
- Independent TEST1 EchoMail became an ordinary native SPITFIRE conference
  message. Control metadata did not appear as caller-visible garbage.
- SBBSecho actually forwarded native TEST2 toward another local test AKA.
  SPITFIRE suppressed that returned artifact using retained identity/history.
  Synthetic multi-hop PATH and point-origin cases supplement this real peer loop.

During peer setup, its missing local user directory and incomplete AKA enrollment
caused diagnostic rejections; these were corrected within the disposable peer.
No SPITFIRE semantic accommodation overrode FTSC. Routine implementation defects
found by tests (control injection, stale queue hold persistence, nested IPC tag
collision, replay counters and old backup fixtures) were fixed narrowly and
retested. No gate-only stop was taken.

The real daemon integration test configures three AKAs across two domains, two
native EchoMail areas, a point NetMail route, nodelist and pointlist sources. It
uses protected operator IPC for policy, mappings, enrollment, scanner/tosser,
queue builds, directory activation and quarantine. Native private authoring uses
an authenticated caller actor. It ingests independent packets when supplied,
restarts with ready work, checks unchanged receipts/queues, then performs cold
backup/restore and verifies policy, active generations and held work.

Core/wire tests additionally cover malformed/every-truncation/oversized packets,
impossible addresses, duplicate and conflicting directory entries, priority and
domain isolation, transactional migration and receipt rollback, unknown controls,
all high-byte charset round trips, timestamp offsets, routing precedence, private
access, replies, weak identity ambiguity and stale work. Together these satisfy
the requested 50-step macOS journey; no live transport is required for any step.

## Final gates and publication

Private gates: **620 passed / 0 failed / 3 ignored**, including the two earlier
ignored tests and one new independent-artifact opt-in. That independent test passed
explicitly in 0.15 seconds. The final native macOS daemon journey with independent
packets passed in 5.49 seconds; the actual sfconfig offline policy CLI returned
localized success. Doctests ran successfully. **115 source headers**, fmt, Clippy
with warnings denied, diff checks and **165 Markdown/local-link/anchor/fence files**
pass. The expanded corpus's before/after digest map is identical. Added-text
privacy/provenance inspection excludes reference content and private artifacts.
`cargo audit --version` reports that cargo-audit is not installed; no audit claim
is made. Git publication alignment is recorded in the closure below. The independently supplied
peer test is opt-in and is run explicitly for acceptance, without publishing its
artifact inputs. The ordinary workspace retains the two earlier ignored tests.
No long Windows CI, VM or cloud runner was used.

## Boundary accounting and exact next action

Items 100–110: **BinkP untouched; live public FidoNet traffic NONE; FileEcho/TIC/
FREQ, AreaFix, B-022, doors, scheduler untouched; DDEV, production and FireComm
unchanged; private FidoNet corpus unpublished; no release/tag/package/installer
or distributed binary created.** Existing development compilation is not a release.
SMB remains unimplemented in SPITFIRE; the independent peer's private store does
not become a SPITFIRE dependency. QWK N1/N2 remain accepted.

Item 111, exact next action: stop after accepted N3 and sanitized source publication.
The next separately scoped pass is **N4: implement BinkP transport over the accepted
schema-23 FTN links/AKAs/routing decisions and immutable outbound artifacts, with
controlled independent leaf/point acceptance and explicit restored-work
reconciliation; do not move message semantics into the mailer**. No live public
FidoNet traffic is authorized by this handoff. Windows live FTN acceptance remains
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**.

## Sanitized public source

Public baseline `d58a5c28f7caf4d02ba4a19b531d4b7283be876f` receives source from
accepted private `fa44a3aca2c66825f5e291c31c66aaa385a3c287`. All 37 changed source,
manifest and catalog files are byte-identical to that private checkpoint. Public
history remains independent. The private corpus index and sample bytes, downloaded
reference documents/source, acceptance artifacts, private continuity files and
credentials are excluded. Public gate totals are recorded in STATUS.md.

A final directory correction makes conflict activation consult only enabled sources
whose current policy matches their recorded profile. Disabled or superseded source
observations cannot veto replacement activation. The focused regression and full
core suite were rerun, with no schema or accepted packet-semantics change.

Public publication scope: 16 added files and 48 updated files, including schema 23,
FTN codecs/native services/directory authority, synthetic tests, operator surfaces
and en-US 1.20.0. All 37 changed source/catalog/manifest files match accepted
private `fa44a3aca2c66825f5e291c31c66aaa385a3c287` exactly. Public history is
independent. No FTSC corpus/index, downloaded wiki/source, private list sample,
peer artifact, credential or private continuity document is included.

Final public gates: **558 passed / 0 failed / 3 ignored**, including the two prior
ignored tests and the independently supplied opt-in test (passed explicitly in
private acceptance). Doctests, **94 headers**, formatting, Clippy with warnings
denied, diff checks and **111 Markdown/local-link/anchor/fence files** pass.
The corrected directory-source case is included. Added-text privacy/provenance and
corpus-digest scans pass. cargo-audit remains unavailable. No real Windows runtime
acceptance, BinkP, live public FTN traffic or release is claimed.
