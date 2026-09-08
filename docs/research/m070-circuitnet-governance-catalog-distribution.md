# M070 — CircuitNET catalog authority, governance and Network Kit

**C7 COMPLETE / ACCEPTED. This revision is the sanitized public source publication.**
This is the sanitized modern implementation report. C1 informs historical semantics;
C2–C6 remain the accepted native foundations. No historical binaries, private research,
application submissions, credentials or acceptance artifacts are distributed.

## Review gate and decisions

C5 Charter/Rules/46-area proposal and all dispositions were reviewed first. The
historical reconciliation remains unchanged: **60 active, five deleted, six rejected,
71 records**. The official June 1991 retained list already contains all 23 CONFCHNG
additions. C7 does not reapply them or rewrite historical status. See
[M068](m068-circuitnet-operations-events-distribution.md) and the
separately retained historical catalog.

The initial modern catalog retains **46 areas**, with specific purposes and explicit
C7 review of every C5 disposition. SUPPORT and CHITCHAT are core; all others optional.
Preserve/rename/replace/new are catalog-review decisions, not changes to historical
facts. The separate [review artifact](../circuitnet-ng/config/catalog-review.json)
records every area; no count or fixed list is hard-coded in scheduler/network authority.

The Charter retains seven committee members and quorum five for continuity, with
explicit founding administration and a transition triggered by sufficient independent
participation. Technical ROOT/publisher authority is distinct from the Administrator's
human office and committee decisions. Human approvals are recorded by reference and
rationale, not a voting engine. M061/M061.1's private/public identity separation informs
the modern handle-allowed policy: unlike the historical body-disclosure rule, NG does
not require public real names or append private account identity. This change is explicit.

The [interface contract](../technical/circuitnet-catalog.md) was written before code.
It defines signatures, chain/lifecycle, generation identity, local choice, transport
and recovery. The implementation adds schema 34, wire 1.4/catalog-sync and operator
IPC 17. Existing native message/conference authority, TLS sessions and Events remain.

## Implementation record

| # | Requested result | C7 result / evidence |
| --- | --- | --- |
| 3 | Schema | 33 → 34; signed revisions, pins, choices, generation metadata and peer sync observations. |
| 4 | Protocol | CIRCUITNET-NG 1.4, optional catalog-sync. |
| 5 | Architecture | Signed network metadata around existing native message/conference authority. |
| 6 | Schema | Fixed typed body and signature wrapper, bounded JSON schema; immutable ID independent of codename/local numbering. |
| 7 | Publisher | Explicit pin to network/catalog ID/ROOT Node ID/Ed25519 public key; no automatic received-key trust. |
| 8 | Signatures | Standard ring Ed25519, domain-separated canonical UTF-8 body; no custom cryptographic primitive. |
| 9 | Chain | Consecutive revision number plus SHA-256 predecessor hash; immutable complete snapshots retain history. |
| 10 | Rollback | Older object rejected; replacing a board from an older catalog backup rejected before replacement. |
| 11 | Forks | Alternate same-number object or predecessor hash rejected; exact current replay idempotent. |
| 12 | Lifecycle | Proposed, Active, Deprecated, Retired; retained generation history. |
| 13 | Create | New immutable ID in reviewed typed draft, signed publication with governance reference. |
| 14 | Update | Metadata/effective revision changes preserve local choices and messages. |
| 15 | Deprecate | Operator attention; existing delivery allowed, new subscriptions require Active. |
| 16 | Retire | Stops new distribution/subscription; preserves local conferences/messages and audit/history. |
| 17 | Reuse | Reactivate keeps ID. Explicit privileged reuse requires new ID, retained predecessor, rationale/reference and CLI confirmation; accidental reuse rejected. |
| 18 | Local state | Available, mapped, ignored, needs-attention/deprecated, retired; core compliance separately reported. |
| 19 | Create+map | Operator chooses local number; native API then explicit mapping. Failed map leaves recoverable local conference. |
| 20 | Map existing | Explicit native conference selection; enabled/public eligibility retained. |
| 21 | Ignore | No creation/subscription; existing routing projection disabled without deleting history. |
| 22 | Local-number proof | Six independent boards choose 77/78/80 locally; wire contains immutable network ID only. |
| 23 | Dossiers | Generation-specific authorization; catalog presence never subscribes; reuse does not inherit old generation subscriptions. |
| 24 | Core | SUPPORT/CHITCHAT required by Charter; impossible/missing local mappings surface attention rather than override local policy. |
| 25 | Propagation | Unchanged signed ROOT → HOST → END objects in normal symmetric Poll, consecutive missing revision requests and acknowledgments. |
| 26 | Compatibility | Minor 0–3 phases remain; catalog-bound messages wait for catalog-sync rather than lose generation identity. Ungoverned profiles retain C2–C6. |
| 27 | Bootstrap | Signed revision-1 seed, independently confirmed trust pin, explicit founding human decision record. |
| 28 | Offline | Signed artifact import validates pinned authority and predecessor; unsigned JSON is never official. |
| 29 | Change notices | Generated from actual snapshots; machine authority remains the signed catalog. |
| 30 | Historical catalog | Unchanged, 71 retained records; 60 active / five deleted / six rejected. |
| 31 | Modern count | 46 initial Active definitions, two core; living revision resource. |
| 32 | Dispositions | Separate original C5 proposal and C7 per-area review retained. |
| 33 | Charter | Original modern Charter 1.0. |
| 34 | Committee | Seven representatives, quorum five; timed bootstrap transition when independent membership is sufficient. |
| 35 | Administrator/ROOT | Human office and technical topology/publishing role explicitly distinct. |
| 36 | Secretary | Membership/Node IDs, proposals, votes, approval references, office terms, amendments and public changes; private applications not published. |
| 37 | Moderation | Committee appointments/replacements, bootstrap interim appointments, proportional conduct rules and review path; no signing power required. |
| 38 | Proposals | Discussion, human approval/rejection, reference/rationale, signed publication. No vote-counting software. |
| 39 | Emergency | Scoped temporary action, 72-hour notice, seven-day review and 14-day continuation limit with explicit reviewed decision. |
| 40 | Rules | Original modern Rules 1.0, concise caller/Sysop conduct, safety and operation. |
| 41 | Identity/privacy | Handles allowed; no automatic private-name disclosure; TLS distinct from conference/File Area visibility and E2EE. |
| 42 | Joining | Kit verification, application, approval/ID, separate TLS/pin enrollment, Test Link, sync, mapping, Dossiers and Events. |
| 43 | Node IDs | Requested then centrally approved unique 1–8 alphanumeric IDs; no static-IP requirement for END. |
| 44 | Registry | Minimal human Administrator/Secretary assignment and retirement record; distributed registry sync deferred. |
| 45 | Kit version | Network Kit 1.0; documentation/configuration only, independent of wire 1.4 and catalog revision. |
| 46 | Contents | Intro, Charter/Rules, generated catalogs/changes, application and field specification, role/joining/security/admin/protocol guides, schema, examples, licenses and text forms. |
| 47 | Machine artifact | Signed initial catalog, public authority pin, schema and per-area review; no local numbers. |
| 48 | Manifest | Deterministic names/sizes/SHA-256; manifest excludes itself. Sorted fixed-timestamp ZIP, two builds identical. |
| 49 | sfconfig | Capability-checked cold-board catalog pin/key/import/export/status/list/draft/publish/changes/create-map/map/ignore. Existing live Poll/Events. |
| 50 | sfmonitor | Catalog revision/publisher, mapping/core attention, pending sync and rejection/error counts alongside peer health. |
| 51 | Audit | Safe pin, verified/imported/published revision, local choices and rejection reasons; no message bodies/private caller data/keys. |
| 52 | Added tests | 13 Rust tests and two Python kit tests added; existing C5 seed test updated for the C7 joining schema. |
| 53 | Workspace tests | Private: 828 verified passes / seven existing ignored, eight doctest groups, across the successful daemon/config suites and corrected remaining-crate run. Public full workspace: 766 passed / zero failed / seven ignored, six doctest groups. |
| 54 | macOS | Apple Silicon arm64; current six-node C7 campaign passed in both private and public workspace runs. All six live CircuitNET campaigns passed (394.17s private / 371.30s public combined). |
| 55 | C2–C6 | C2 offline, C3 live, C4 directed/control, C5 Events, C6 Files, FTN and QWK daemon regressions pass. |
| 56 | Backup | Pins/revisions/choices/receipts retained; signed snapshot validation; known newer target cannot be overwritten by older catalog state. |
| 57 | Localization | en-US 1.31.0; 17 catalog strings added (1,421 total), existing usage updated; operator IPC 17. |
| 58 | Human docs | Charter/Rules, kit/joining/application/role/security/catalog-administration and CircuitNET/Events manuals. |
| 59 | Technical docs | Canonical schema/signing/chain/generation/local authority/recovery contract and 1.4 transport phase. |
| 60 | Static gates | Headers 172 private / 149 public; fmt, strict workspace all-target Clippy and diff checks pass. |
| 61 | Markdown/links | 225 private Markdown files / 1,561 local links; 153 public / 1,085; kit 17 / 26. Zero issues. |
| 62 | Provenance | Modern independently written artifacts only; raw historical corpus/private identity research excluded from public synchronization. |
| 63 | cargo-audit | Unavailable (`cargo audit` command is not installed); no audit success claimed. |
| 66 | Public delta | 63 paths: 21 added / 42 updated; 52 allowlisted paths byte-identical, including 24 crate paths. |
| 67 | CNP/CND | NONE. |
| 68 | Private mail/E2EE | NONE. |
| 69 | File request | NONE. |
| 70 | Governance automation | NONE beyond catalog technical publication/reference authority. |
| 71 | Production changes | NONE. No production access needed. |
| 72 | External CircuitNET | NONE; disposable loopback only. |
| 73 | Exact next action | Stop for C7 review of Charter/Rules, initial catalog, signing-key custody and joining process. Do not begin C8. |

## Verification and remaining boundaries

The private full workspace command passed every daemon/configuration and live networking
suite, then found one C5 test still parsing the C7 joining seed as a legacy native
profile. Only that test was corrected: the implementation-neutral seed now verifies
empty endpoints/credentials and the catalog capability, while retaining the C5 proposal
and privacy checks. The entire sf-core suite and all remaining private crates were
rerun successfully, followed by daemon/configuration doctests. This gives 828 distinct
passing private tests and seven existing ignored tests, not a claim that the first full
command was failure-free. The corrected sanitized public workspace then passed the
complete `cargo test --workspace` command: 766 / 0 / 7. Both strict all-target Clippy,
fmt, headers and diff checks pass. Two Python kit tests pass in both checkouts.

An earlier synthetic old-backup fixture retained C7's Event wakeup trigger after removing
the Event table; the fixture cleanup was corrected, and all backup tests passed in the
later private and public daemon runs. Production migration/restore semantics were not
weakened. The committed `circuitnet_live` test includes six independent disposable C7
daemons and proves Immediate/Scheduled propagation, local choices, invalid signed-object
rejection over authenticated TLS, lifecycle/generation preservation and restore protection.
Optional SPITFIRE_C7_EVIDENCE selects a new private evidence directory. Initial sandbox
port denial was resolved by authorized isolated-loopback execution. No production or
external BBS polling occurred; early native-bound fixture corrections are retained in
the work history rather than presented as implementation acceptance.

Build `catalog-artifact`, then run `python3 -m unittest discover -s tools/tests` and
`python3 tools/build-circuitnet-kit.py --check`. Build twice into new output directories
and compare ZIP SHA-256. Manifest inventory excludes itself and includes every other
file. Archives use fixed timestamps, permissions and sorted paths, and are not committed.
No actual software binary release or automatic CircuitNET Files distribution is created.
The final 40-file kit has SHA-256
`ad504bbec72e2c12845d0cc0492c34c0ae39c1a6b076ae6d5f168f31d4483096`;
two private builds and the public-source build are byte-identical. The signed initial
catalog revision-1 hash is
`84defdfbfb50b9eca68cf9bbf086e0a49ad6492caf8dc536bf3ff923f3ce349c`.

Key rotation, a distributed Node Registry, electronic voting/elections, bulletin
integration, file-area governance and file requests remain outside C7. The joining
profile has no live endpoints or secrets. A fresh replacement board cannot infer a
revision it never observed; recover retained signed history and sync before export.
No existing-board rollback protection is weakened to make a restore succeed.

Network catalog authority remains separate from local numbering. Technical publication
is separate from human governance. Retirement preserves history. Catalog sync rides
existing transport and Events. Modern Charter/Rules/catalog are rights-safe successors,
not republished proprietary documents. C1 remains historical authority and C2–C6 remain
accepted foundations. No production systems changed or external live traffic occurred.
