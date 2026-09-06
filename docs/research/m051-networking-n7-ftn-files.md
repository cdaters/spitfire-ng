# M051 — N7 FTN file networking

Date: 2026-09-06 UTC. Status: **COMPLETE / ACCEPTED — sanitized source publication**.
Private start: `44271f85acec5ed54a0e17bdb2f65995bfeb4996`.
Accepted N6 source: `03ff31f724bbd66ba5e47a338928c9a969052590`.
Public baseline: `f3130181c5959b7f569d4841e4ab184824396d9a`.
Schema **25 → 26**. Localization **en-US 1.24.0 / 1,291 messages** (42 additions).

## What N7 adds

SPITFIRE NG can be a FileEcho leaf/hub, hatch origin and explicit FREQ
requester/servicer. Native `files`, file areas, managed storage, descriptions,
lifecycle and SHA-256 remain canonical. File network rows own only FTN mapping,
subscription, provenance, bounded staging, request authorization and per-link
payload/TIC acknowledgement truth. Native message authority and N1–N6 are retained;
BinkP transports all artifacts through the accepted N4 engine.

The [Technical Reference](../technical/ftn-files.md) is the canonical interface,
format, trust, bounds and recovery contract. The [Sysop manual](../manual/ftn-files.md)
contains the actual sfconfig/sfmonitor workflows. These interfaces were defined
before implementing the adapters. [Architectural decision](../technical/ftn-files.md#schema-26-and-implementation-map)
records the architectural decision rather than duplicating the specification.

## Evidence and provenance

Primary references: FTSC FTS-5006.001 TIC, FTS-1026.001 BinkP,
FTS-0006.002 request-file lines/naming and FTS-5005.003 context. The existing
indexed private corpus was reviewed read-only; no research sample became a test
fixture or published asset. Live FTSC fetch attempts failed, so this is a precise
versioned profile, not a claim that supersession was verified online.

Secondary read-only engineering review covered Synchronet TickIT, HatchIT,
FREQIT, BinkIT/BinkP and configuration loading. Adopted concepts: exact TIC/payload
pairing, conventional CRC and per-link controls. Adapted: durable native-ID
publication, transactional fanout, write-only credentials, bounded requests and
safe staging. Rejected: executable file hooks, general filesystem resolution,
automatic archives and implicit magic publication. SBBSecho is principally the
message tosser; independent file acceptance uses Synchronet's TickIT/BinkP.
No peer source, comments, strings, config layout or runtime dependency was copied.
The isolated relay/harness is independently authored acceptance infrastructure.

The bounded FireComm review is recorded in the contract. Its transport lessons
are engineering guidance only; FireComm and terminal behavior remain unchanged.

## Independent and macOS acceptance

Native acceptance uses four actual Apple Silicon macOS daemons: hub, upstream,
A and B, on disposable roots and loopback endpoints. Two native areas map to
FILEAREA1/FILEAREA2. A subscribes only to the first; B to both. The integration
journey proves hatch, upstream inbound, selective area fanout, A-origin traffic
through the hub, no ingress reflection, exact native counts, acknowledged FREQ,
held-subscription future queue intent, failed B with completed A, cold restore,
explicit release, B completion and no A resend. Graceful restarts preserve
subscriptions, credentials and acceptance; no restored live session is invented.

A separate authenticated wire journey against the real daemon proves wrong TIC
password, CRC, size and traversal rejection; TIC-first/payload-first admission;
interruption before payload completion, graceful restart and full retry; replay
without a second native record; unapproved/traversal/oversized-list FREQ denial.
Partial wire bytes never enter native storage or durable complete staging.
A cold backup/restore also exercises an authenticated incomplete TIC before its
matching payload, independently of the relational snapshot test.

The independent peer is the same pinned controlled Synchronet **3.19c** /
SBBSecho **3.15** environment used in N6, isolated with container networking
**none**, connected only by a transparent loopback byte relay. Native SPITFIRE
runs on Apple Silicon; the legacy peer image runs under amd64 emulation.

- SPITFIRE hatch → independent BinkP custody → TickIT validation/import passed.
- Independent TickIT validates a synthetic upstream file, constructs its outgoing
  TIC with From/Path/Seenby and CRC, and sends TIC before payload to SPITFIRE.
  SPITFIRE imports once and forwards only to the other native subscriber.
- Replaying that independently produced pair retains one native record and one
  accepted downstream delivery. No replay fanout or reflection occurs.
- A hatch queued by real sfconfig receives independent payload/TIC acknowledgements
  and independent TickIT processing.
- Independent FREQ is **not claimed**. Exact-request service and response delivery
  are proven native-to-native and with authenticated wire rejection cases.
  No old HatchIT single-link output is claimed as a conforming origin profile.

Real sfconfig PTYs exercise named policy fields, stale-draft rejection after a
second authenticated operator changes policy, hatch preview/confirmation and
masked TIC credential input. sfmonitor exposes Files, safe queue detail, staging
and failure history. PTY checks assert no secret echo, restored terminal flags
and alternate-screen exit. No screenshot or private acceptance artifact ships.

## Verification and limits

Focused tests cover the TIC codec, mandatory fields, CP437, filename/device/path
rejection, CRC, unknown metadata, native hatching, pairing, authentication,
secret redaction/rotation, replay, subscriptions, reflection, partial acknowledgement,
retry, holds, FREQ grants/public-area eligibility/traversal/session bounds,
request offer identity, symlink refusal, transaction rollback and storage capacity.
Schema tests cover fresh migration, schema-25 upgrade and atomic rollback on
failure. Snapshot tests reconcile later A/B acknowledgement truth against an older
backup while retaining incomplete staging. Existing N1–N6 suites remain regression
authority. Full gates and exact totals are recorded below at closure.

Disk-full behavior is exercised through bounded admission and injected database
publication failure/rollback, together with native journal recovery regressions;
a real full-volume destructive test is not claimed. No partial native record or
false completion results. Payloads are opaque, including archives/executables.

Optional scope completed: hatch recipient preview, deterministic long-name
mapping, strong SHA-256 provenance and bounded recent file/FREQ history.
Optional scope deferred: FileFix/FileMgr, wildcard/magic FREQ, quarantine
reprocess/discard, Replaces deletion, persistent wire resume and scanner hooks.
Replaces/unknown TIC metadata are retained without action. Same-zone EchoMail
remains the implemented message profile; FileEcho's four-dimensional metadata
does not introduce a message gateway transformation.

The finite 10,000-row publication/delivery admission ceiling retains provenance
without automatic compaction. Stale staging cleanup runs on later arrival, not a
scheduler. Loss of remote acknowledgement history cannot be reconstructed from
an older backup: uncertain work remains held until surviving proof/review.

## Required final-report register

| Items | Result / canonical evidence |
|---|---|
| 1–3 Starting checkpoints and schema | Exact hashes above; 25 → 26. |
| 4–7 Architecture, native authority, maps, subscriptions | One native catalog; typed immutable mapping and separate per-link permissions. |
| 8–11 Inbound, outbound, hub, reflection | Native and independent journeys; origin/ingress/Path/Seenby exclusion. |
| 12–17 TIC parser/trust/password/path/pairing/staging | Bounded FTSC codec; authenticated link plus separate credential; no raw Pw in durable staging. |
| 18–20 Identity/duplicates/Replaces | Area + SHA-256/size, required wire CRC; no filename-only identity; Replaces retained without deletion. |
| 21–22 Hatch architecture/UX | Existing native file, typed preview, CAS and confirmation; no storage duplication. |
| 23–27 FREQ architecture/authorization/paths/bounds/aliases | Exact grant-only native resolution; public managed areas; finite request/session bounds; aliases deferred. |
| 28–31 Encoding/hash/quarantine/mutations | CP437 explicit, safe transfer alias with provenance; SHA-256; safe bounded rejection history; mutations deferred. |
| 32–34 Operator surfaces/detail | sfconfig file menu and sfmonitor 9 Files / Enter detail / quarantine. |
| 35–38 Delivery/interruption/partial/hold | Matching offered M_GOT only; artifact-level retry; completed recipients never retried for another failure. |
| 39–40 Storage/archives/executables | Semantic failure/rollback and native journal; no unpacking or execution. |
| 41–45 Backup maps/fanout/staging/hatch/FREQ | Native cold journey and snapshot reconciliation; durable policy/receipts, no fake sessions. |
| 46–48 Authorization/audit/privacy | Existing 21-capability framework, authenticated operator dispatch, transactional safe audit; bootstrap read-only. |
| 49–50 Independent/macOS | Coverage and limits above; no independent FREQ claim. |
| 51–56 N1–N6 | Existing accepted networking regressions rerun. |
| 57–59 Tests/workspace/localization | Focused tests plus full gates below; en-US 1.24.0 / 1,291. |
| 60–62 Manual/reference/report | Linked canonical guides and this report. |
| 63–67 Gates/doctests/links/provenance/audit | Closure below; cargo-audit unavailable (subcommand absent). |
| 68–70 N7 status/optional scope | Status above; completed/deferred optional scope explicitly listed. |
| 71–72 Private commits/push | Closure below. |
| 73–87 Public synchronization | Only after complete private acceptance and pushed clean source; separate closure below. |
| 88 | Live public FidoNet traffic: **NONE**. |
| 89–95 | Scheduler, CircuitNET implementation, B-022, doors, DDEV, production and FireComm unchanged. |
| 96–98 | Private corpora unpublished; no release/tag/binary/service packaging; Windows live acceptance deferred. |
| 99 | Stop after accepted N7 and sanitized source synchronization. Subsequent work requires a new scoped pass. |

## Private closure

Private gates: **682 passed / 0 failed / 7 opt-in ignored**, **8 doctest suites**,
**133 project source headers**, fmt, Clippy with warnings denied, diff whitespace,
and **177 Markdown/local-link documents** pass. Focused final localization and
operator UI tests pass after the final labels/detail edits. All **382 indexed
FTSC corpus files** have unchanged SHA-256; Finder metadata is consistently
outside the corpus manifest. Source-addition scans find no private paths, token
material, corpus hashes or forbidden artifact additions. Synthetic authored test
credentials are fixtures; private credential files and acceptance assets are not
publication inputs. cargo-audit is unavailable because the subcommand is absent.

N7 acceptance conditions are satisfied. Native message and file authority remain
canonical; TIC is authenticated/fail-closed; FREQ serves only explicitly authorized
native files; traversal/arbitrary-file access is blocked. Partial fanout and restore
preserve each proven delivery. No public FidoNet traffic or excluded work occurred.
Initial source commit: `68119a5803d853523dcb017b719f3e471321b947` (pushed via
existing authenticated HTTPS after the configured SSH key was unavailable).
Final review adds audited retry-budget renewal without losing accepted artifacts,
a credential-rotation guard using the existing BinkP worker slots, and native
filename collision/long-hatch coverage. Eighteen tests were added over N6.
The final private gate uses `RUST_TEST_THREADS=2 cargo test --workspace`.
An earlier simultaneous-run setup test hit its existing five-second deadline;
it passed alone and in the public suite. No assertion or deadline was weakened.
Accepted private source: `6a6a7e9dd93b911e9281085e62b116160bffafb9`; pushed/aligned/clean before publication.

## Sanitized public closure

Public baseline: `f3130181c5959b7f569d4841e4ab184824396d9a`.
Accepted private source: `6a6a7e9dd93b911e9281085e62b116160bffafb9`.
The publication is a direct source commit after that public baseline; no private
Git history is imported. **8 added / 44 updated files** publish FileEcho/TIC,
native hatching, FREQ, schema 26, operator surfaces, tests, guides and localization.
All **25 shared implementation/catalog files** are byte-identical to accepted
private source. Cargo.lock retains the existing public package set unchanged.

Public gates: **620 passed / 0 failed / 7 opt-in ignored**, **6 doctest suites**,
**112 source headers**, fmt, Clippy with warnings denied, diff whitespace and
**123 Markdown/local-link documents** pass. Schema-25 upgrade, fresh migration,
rollback and cold file-network recovery run in the public workspace. No private
path/token/corpus hash or forbidden binary/archive artifact was introduced.
Private project state/decision/session logs and corpus indexes, private corpora,
peer source/config, credential files and acceptance artifacts are excluded.
The safe architectural decision is in the public Technical Reference.

The exact public commit and both remote alignments are recorded in the private
session closure and final delivery report. No release, tag or binary is created.
Exact next action: stop after accepted N7 source synchronization. No subsequent
networking or excluded milestone work begins.
