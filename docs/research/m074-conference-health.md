# M074 — Native Conference Health

C8 COMPLETE / ACCEPTED. This is the sanitized public source report. Starting private 9f79ba5305a89e2abef1649ff9a104f8fcceab25;
public 86e8e78531dd4ecaa96d26fc1a11dc7b3c5f2273. Schema starts at 35.

## Interfaces and authority before implementation

Conference Health belongs to native conferences. One retained analytical projection
serves local, FTN, QWK and CircuitNET conferences. Adapter mappings are descriptive
context and never multiply message counts. No message body or caller display name
belongs in a normal health snapshot or bulletin.

Read activity means a successful native high-water advance, not individual-message
reading or attention. Daily reader progress uses opaque random per-account tokens;
only aggregate distinct counts leave the core. Delete the account-to-token link on
account deletion while retaining bounded aggregate facts. Prior read timestamps
cannot reconstruct old readership. Record monitoring start and distinguish missing
history from observed zero activity. Native publication/placement time determines
message windows, not an untrusted remote author's date.

A bounded work queue and historical message cursor build a small retained message
projection needed for idempotent corrections, thread links and distinct counts.
It contains IDs/time/provenance classification, not bodies or names. Native changes
invalidate only affected conferences. Snapshot aggregation occurs through the generic
Events scheduler and explicit operator refresh, never per TUI repaint or login.
A batch has fixed work limits; incomplete snapshots expose backlog/freshness instead
of pretending to be complete. Retention covers current and previous 90-day windows.

Public metrics exclude private/local-recipient messages. Standard windows are
7/30/90 UTC days; previous-window comparisons remain visible. Statuses are rule-based
with evidence, no opaque score. New/insufficient monitoring history suppresses false
dormancy and locally-unread claims. No analytics action deletes, retires, unmaps or
unsubscribes a conference. Retired local/network contexts preserve history.

Operator projections require existing statistics authority; mutations require
explicit configuration/maintenance authority. Caller Hot Conferences must apply
current native access rules at rendering time and never publish private readership.
No external analytics, telemetry, public service deployment or C9 work is in scope.

## Implemented contract and deliberate limits

The [human manual](../manual/conference-health.md) describes operation; the
[technical specification](../technical/conference-health.md) defines storage,
windows, formulas, boundaries and recovery. Historical evidence is summarized in
[M073](m073-conference-health-sfcnfuse.md). Originals are not distribution material.

Schema 36 is a native derived projection, not a second message authority. Operator
IPC advances to 1.18 / conference-health; CIRCUITNET-NG remains 1.4. en-US advances
to 1.33.0 with 1,475 strings (43 added). Local FTN/QWK/CircuitNET context shares the same native message counts.

Defaults: tracking enabled, retention 365 days, dormancy 30 days, bulletin disabled,
limit ten. The operator deliberately installs a five-minute Scheduled Event or
uses manual rollup. No new timer or implicit job is installed. This action accepts
Scheduled/Manual policy; networking Immediate/Hybrid behavior is unchanged.

Projection work is bounded to 1,000 messages and 32 conferences per transaction;
a daemon action checks shutdown/time between at most 16 batches. Expired records
are pruned in bounded batches. Per-message facts contain no message bodies or
names. They support distinct threads, corrections and idempotency; daily anonymous
reader rows support distinct window counts. This is not a separate store per adapter.

Counts use retained public/all-callers native placement, not source-authored dates.
Readership means high-water progress, including native online reads and committed
QWK download progress, excluding explicit pointer resets. No old readership is
fabricated. No exact-read, attention, sentiment, external-person identity or opaque
score is claimed. Known local poster counts and raw reply/thread/progress/post components are
available in aggregate JSON. Native API helpers derive ratios; they are not
additional JSON fields or prominent TUI columns. Lifetime totals, per-caller backlog,
outbound delivery metrics and longest-thread ranking are not invented.

Hot Conferences uses the existing caller Bulletin flow, plain text and current
native access checks each time. It is not a shared static HOTCONF file or new
Bulletin/ANSI subsystem. Native Sysop-only classification still controls access.
Normal operator views require Board Statistics and expose no individual reader
identity. Settings require sensitive configuration authority; direct rollup requires
online configuration authority. Existing generic Event controls retain their gates.

## Acceptance record

Disposable Apple Silicon/macOS 26.6.2 tests use synthetic boards/messages/accounts.
The native daemon journey passed scheduled rollup, real local operator requests,
retry receipt replay, restart, cold backup/restore and graceful shutdown. No public
listener, external peer or production board participates in this C8 journey.
The caller Bulletin journey uses the actual session engine with an in-memory
terminal adapter; the TUI renders through its test backend at 80x25 and 60x20.
The full existing network regressions use isolated loopback peers as before.

The first larger fixture measured 200 conferences / 20,000 native messages:
45.897 seconds to seed through native posting, 3.183 seconds to roll up, and
0.403 seconds for ten cached pages (about 40 ms/page). Each batch asserted fixed
message/conference bounds; each page stayed below the local IPC frame limit.
The final focused run measured 35.869s seed, 1.350s rollup and 0.259s for ten
cached pages. Measurements are local evidence, not a performance promise for
arbitrary boards; peak process RSS was not separately measured. The rebuilt public
suite, including the final expiry refinement, measured 43.326s seed, 1.346s rollup
and 0.245s for ten cached pages.

During implementation, tests caught and resolved an outer-upsert/trigger conflict
on repeated read progress, a fixture attempting to rewrite immutable author identity,
old-schema fixture cleanup, and operator feature-order compatibility. The actual caller test also corrected
the visible Bulletin key (B, not its internal Y action) and used native update
rather than ensure when changing its synthetic access policy. Historical
identity safeguards were retained; synthetic inbound posts now use valid native
external-origin insertion rather than mutating posted authors. The existing schema-26 FREQ fixture likewise removes later analytics objects when
constructing its old database. Monitor navigation retains Dashboard as the first
view and the existing wrap order.

During overlapping development validation, tests encountered a hub custody failure,
a synthetic TLS first-frame failure and a daemon startup deadline. The unchanged hub and reconnect
journeys passed isolated checks; diagnostics now retain the first-frame/startup
error. Shared artifacts rebuilt during concurrent Cargo commands also invalidated
two private doctest groups. Those development runs are not recorded as green;
final acceptance runs use stable artifacts and serialized tests. Final review added
one-row indexed expiry on read progress, closing the no-rollup storage-growth edge
case; the retention test exercises both first and subsequent pointer observations. No transport,
certificate verification or networking authority was weakened to make tests pass.

The public full workspace ran all 790 non-ignored tests: 786 passed and four failed
before fixture/navigation corrections and isolated startup/reconnect rechecks.
The final rebuilt public targets passed 446 core tests (one existing ignored),
37 monitor tests, reconnect/lost-ACK and the native Health daemon journey: 485
passes, zero failures. This replaces all four earlier failed cases without claiming
that the earlier full command was green. Six public doctest groups passed. The
one-row expiry refinement was validated in this rebuilt core suite and daemon
journey after the private full workspace's compilation; implementation bytes match.

The final private workspace passed 852 tests, zero failures and seven existing
ignored tests, plus all eight doctest groups. Six Network Kit regression tests also
passed. Private/public source headers, formatting, strict all-target Clippy,
whitespace and local-link checks pass. cargo-audit remains unavailable.
Publication follows the accepted implementation checkpoint; no public service is
deployed as part of this work.

## Requested report

| # | Item | Result / authority |
| --- | --- | --- |
| 1 | Starting private | `9f79ba5305a89e2abef1649ff9a104f8fcceab25` |
| 2 | Starting public | `86e8e78531dd4ecaa96d26fc1a11dc7b3c5f2273` |
| 3 | Schema | 35 -> 36; CircuitNET 1.4 unchanged. |
| 4 | SFCNFUSE | Daily last-read comparison and optional conference-use bulletins; M073 distinguishes documented/inferred/unknown. |
| 5 | Architecture | Board-wide native derived authority; no adapter analytics databases. |
| 6 | Sources | Native conferences, public message placement/provenance/parents, last-read progress; mappings as context. |
| 7 | Readership | Strict high-water advances, including committed offline-packet progress; not exact reads. |
| 8 | Unique readers | Distinct retained account tokens across each window, not sums of daily uniques. Deletion severs linkage; restored/recreated accounts can count again. |
| 9 | Privacy | Aggregate outputs; no names, tokens, bodies or external telemetry. |
| 10 | Storage | Work queue, bounded seed, retained message facts, daily reader observations, cached snapshots. |
| 11 | Retention | Default 365 days, configurable 180–730; bounded rollup pruning plus one indexed expired reader row per progress observation. |
| 12 | Migration | Historical message seed cursor; no historical readership reconstruction; monitoring start exposed. |
| 13 | 7 days | UTC calendar days including current partial day. |
| 14 | 30 days | Same semantics; adjacent previous 30 days retained for comparison. |
| 15 | 90 days | Same semantics; long inactivity evidence without claiming pre-monitoring reads. |
| 16 | Local posts | Native origin, counted once per delivery. |
| 17 | Inbound | External network origin, independent of mapping count and forwarding. |
| 18 | Replies | Native parent linkage. |
| 19 | Threads | Distinct roots active in window; bounded walk and incomplete-chain indicator. |
| 20 | Read/write ratio | Native helper: progress events / public posts; absent on zero denominator. JSON exposes raw components. |
| 21 | Last activity | Retained public placement and local-post timestamps. |
| 22 | Last read | Latest observed progress timestamp; unknown before tracking, not invented. |
| 23 | Trend | 60 observed days, combined sample >=5; rising +3 and +25%, falling -3 and -20%, otherwise stable. |
| 24 | Dormancy | No posts and no progress in selected 7/30/90-day threshold. |
| 25 | New | Effective monitoring age suppresses premature dormancy. |
| 26 | Locally unread | Inbound traffic but zero progress over a fully observed threshold. |
| 27 | Status | Locally Active / Quiet / Locally Unread / Dormant / New; Disabled/Catching Up overrides. |
| 28 | Explanation | Raw counts, observation age, previous readership, dormancy-window counts and last activity. |
| 29 | FTN | Domain/area context from native mapping. |
| 30 | QWK | Network/area context from native links/mappings. |
| 31 | CircuitNET | Profile/codename and retained catalog retirement context. |
| 32 | Multiple mappings | Native message ID is projection key; mappings never multiply counts. |
| 33 | Retired | Hidden by default, included explicitly; local history preserved. |
| 34 | Sysop-only | Native access remains authoritative; ordinary caller bulletin excludes restricted areas. |
| 35 | sfmonitor | First-class paged/filterable/sortable overview and scrollable evidence detail. |
| 36 | sfconfig | Health status/page/configure/rollup/schedule through authenticated local operator IPC. |
| 37 | Hot Conferences | Optional caller `B` then `H`; aggregate readership ranking. |
| 38 | Formats | Existing native plain-text Bulletin flow; no static file/ANSI subsystem. |
| 39 | Caller filtering | Reapplied after selection from current native access and publication policy. |
| 40 | Events | Typed generic conference-health action; no new scheduler. |
| 41 | Restart/idempotency | Durable queue/cursor; replace projections, never replay reader increments. |
| 42 | Backup/restore | SQLite authority included in native cold backup/restore; future activity withheld after clock rollback. |
| 43 | Performance | Final rebuilt public fixture: 200 conferences/20,000 messages; 1.346s rollup, 0.245s for ten cached pages. Peak RSS not separately measured. |
| 44 | Tests added | 16: eleven core Health tests, migration, IPC permissions, TUI, actual caller Bulletin and daemon/Event/recovery. Existing fixtures corrected. |
| 45 | Workspace totals | Private full run: 852 passed / 0 failed / 7 existing ignored; eight doctest groups. Public: all 790 non-ignored cases covered by full run plus final corrective rechecks, six doctest groups; see exact scope above. |
| 46 | macOS | Apple Silicon/macOS 26.6.2; native daemon/Event/recovery, actual caller engine, TUI test backend and synthetic performance fixture passed. |
| 47 | Regressions | FTN/QWK/CircuitNET/Files passed, including all seven private live CircuitNET campaigns; six Network Kit tests passed. |
| 48 | Localization | en-US 1.33.0; native operator/caller strings. |
| 49 | Human docs | Conference Health manual, sfconfig/sfmonitor/Events indexes and caller guide. |
| 50 | Technical docs | Native projection/window/formula/access/recovery specification. |
| 51 | Historical note | Rights-safe M073; no proprietary originals published. |
| 52 | Quality gates | Private/public headers 179/156, fmt, strict all-target Clippy and diff checks passed. |
| 53 | Links | Private 242 Markdown files / 1,638 local links; public 170 / 1,165; zero path/anchor issues. |
| 54 | Provenance/security | Synthetic acceptance, aggregate-only public code/docs, no historical binaries or real readership. |
| 55 | cargo-audit | Unavailable (`cargo audit --version`: no such command); no scan claimed. |
| 56 | Final private | Accepted private implementation: f22f29c638652a5a9e8819496f5743da01278948; final private documentation closure follows. |
| 57 | Final public | The public commit containing this report; exact Git SHA is recorded in the private publication closure. |
| 58 | Public delta | 42 public paths: 10 added / 32 updated. All 28 implementation/test/localization paths match accepted private source. |
| 59 | C7.2 identity changes | NONE. |
| 60 | DNS/web/mail deployment | NONE. |
| 61 | C9 | NONE. |
| 62 | Production changes | NONE. |
| 63 | External live BBS traffic | NONE; only existing disposable loopback regressions. |
| 64 | Exact next action | Stop for C8 review. No C9 or public-service deployment without separate authorization. |

Conference Health is native SPITFIRE NG authority, not CircuitNET-specific.
Network volume is not local readership. Analytics are aggregate/private by default.
SFCNFUSE informed the concept and was not cloned. C1-C7.2 remain accepted. No public
service deployment or production system change occurred. C9 has not begun.

## Publication source

Private implementation `f22f29c638652a5a9e8819496f5743da01278948` was pushed and
verified clean with HEAD equal to origin/main before this public publication.
This public source contains modern code, synthetic tests and rights-safe documents.
It excludes the private research corpus, personal readership and acceptance logs.
