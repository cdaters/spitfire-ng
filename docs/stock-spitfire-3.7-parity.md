# Stock SPITFIRE 3.7 Parity Checklist

This is the durable operational scope and implementation-status checklist for
the first major SPITFIRE NG development phase. It answers two questions:

1. What could a Sysop and caller do with a normal stock SPITFIRE 3.7 board?
2. How much of that operating model does SPITFIRE NG currently reproduce?

The primary operational source is the preserved Buffalo Creek manual at
`research/samples/shareware-software/sf37-2/spitfire.doc`. That input is
read-only and intentionally excluded from Git. Binary details and findings not
specified by the manual remain governed by dedicated public format
specifications and the private preservation record.

This is a parity checklist, not a promise to reproduce DOS itself. Apply the
project rule: preserve identity-defining behavior; modernize platform
limitations.

## Parity interpretation

The 46-row Category-A result is **functional stock-core parity**: the accepted
native operations and their stated tests are complete. It is not a claim that
every historical setup prompt, generated screen, optional DISPLAY override,
login cadence, companion utility, or advanced resource has been reproduced.

Runtime evidence exposed a real setup and presentation distinction without
reopening the functional ledger. SPITFIRE NG now
supports an engine-generated, security-filtered `.MNU` presentation as a
first-class menu mode and separately supports exact-security BBS/CLR display
overrides. Clean setup exposes Modern/Classic/Minimal, active/base selection,
menu mode, stock post-login journey, new-caller level, configured Sysop
threshold, and initial Sysop level. Modern remains the default.

Accordingly:

- **functional stock-core parity:** achieved for the Category-A contract;
- **historical setup/presentation parity:** improved and verified for the M037
  generated-menu/override/security/login scope, but not globally complete;
- **advanced stock resource breadth:** remains in existing Category-B rows;
- **RIP and companion-program behavior:** remains explicitly unimplemented or
  deferred; and
- **historical-original redistribution:** remains prohibited absent separate
  rights evidence.

The primary-source `SOP999` manual statement versus actual `SOP100` archive
members is unresolved and is not normalized into a historical Sysop constant.
The generated-menu behavior is documented in
[Presentation Profiles](presentation-profiles.md).

## Scope and Evidence

The target is a substantially usable native equivalent of a **stock SPITFIRE
3.7 installation**, not every Buffalo Creek companion product, third-party
door, network, or future SPITFIRE NG feature. Manual section references below
use `SF37 §x.y`.

Classification:

- **A — Stock core / near-term parity:** required for a modern board to
  reasonably represent normal stock SPITFIRE 3.7 operation.
- **B — Stock advanced / follow-on:** included in stock operation but not a
  prerequisite for the first genuinely usable end-to-end board.
- **C — Optional / companion / network / add-on:** companion executables,
  external ecosystems, network exchange, doors, or extension mechanisms.
- **D — Historical implementation detail:** a DOS, modem, serial, memory,
  filesystem, printer, or multitasking constraint to replace with a native
  facility.

Implementation status:

- **NOT STARTED:** no caller-usable implementation exists.
- **PARTIAL:** some behavior exists, but the listed acceptance is incomplete.
- **IMPLEMENTED:** the intended behavior exists with automated coverage.
- **VERIFIED:** exercised against its stated compatibility or end-to-end
  acceptance criteria.
- **DEFERRED:** intentionally scheduled after the current objective.
- **NOT APPLICABLE / MODERNIZED:** the historical mechanism will not be
  reproduced literally; the replacement is specified.

Design documentation or reverse engineering alone does not make a capability
implemented. As of 2026-08-22, Increments 0–6, the promoted binary-transfer
increment, the resource/menu, caller-policy, message-interaction, and file-
presentation closures provide the native runtime,
multinode connection/resource/menu path, persistent caller/authentication
lifecycle and terminal preferences, local message conferences, the first
usable native file library, and caller/Sysop page-chat plus an essential
operator surface. Exact remaining fidelity and transfer-protocol gaps retain
explicit statuses below.

## Increment 6 Category-A Audit

Before Increment 6 there were 46 Category-A rows: 16 VERIFIED, 8 IMPLEMENTED,
21 PARTIAL, and 1 NOT STARTED. The audit assigned page/chat, persistent caller
terminal preferences, shared MORE/abort behavior, operator essentials, and a
clean setup-created-board proof to Increment 6. Complete display precedence,
message scans/editor breadth, inactivity/time-policy breadth, and backup/
recovery remain narrow follow-ups rather than being silently removed.

After Increment 6 the same 46 rows are: **20 VERIFIED, 12 IMPLEMENTED, 14
PARTIAL, 0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category
A**. The D table separately records historical mechanisms that are explicitly
modernized. Stock-core parity is not declared complete while the 14 PARTIAL
rows below retain stated obligations.

## Category-A Resource/Menu Closure Audit

The approved baseline was independently recounted before this closure: 46
Category-A rows, comprising **20 VERIFIED, 12 IMPLEMENTED, 14 PARTIAL, 0 NOT
STARTED, and 0 BLOCKED/RESEARCH**. This table records every row that was not
already VERIFIED; known fidelity breadth was not removed to improve the count.

| ID | Capability | Initial | Evidence needed for VERIFIED | Implementation gap | Fidelity gap | Historical evidence | Closure action | Final |
|---|---|---:|---|---|---|---|---|---:|
| A-003 | Caller defaults/policy | PARTIAL | Complete configured policy journey | Inactivity/private-board policy | Remaining default-question breadth | SF37 §3.2–3.3 | Retained as next focused policy closure | PARTIAL |
| A-004 | Conference configuration | IMPLEMENTED | Clean-board admin acceptance | None in current native model | Retention/network breadth | SF37 §4.0 | Existing tests retained | IMPLEMENTED |
| A-005 | File-area configuration | IMPLEMENTED | Clean-board admin acceptance | None in current native model | Relocation/extended directories | SF37 §4.2 | Existing tests retained | IMPLEMENTED |
| A-006 | Start/stop/supervise | IMPLEMENTED | Packaged service/operator acceptance | Host-service packaging | Full local-control breadth | SF37 §3, §6 | Clean startup/shutdown repeated | IMPLEMENTED |
| A-014 | Persistent caller record | IMPLEMENTED | Historical import/field-policy acceptance | No native-flow gap | Legacy field import | SF37 §3, §8.2, §9.2 | Existing persistence retained | IMPLEMENTED |
| A-015 | Security/resource access | PARTIAL | Complete policy/resource matrix | Remaining policy gates | Private-board/special resource breadth | SF37 §3.2, §4, §5.5 | Exact-level menus added; row retained | PARTIAL |
| A-016 | Time/inactivity | PARTIAL | Idle and daily-limit proof | Idle enforcement/timezone | Remaining `DAILYLMT` semantics | SF37 §3.2, §8.2 | Retained without scope expansion | PARTIAL |
| A-020 | CP437 terminal behavior | IMPLEMENTED | Byte-exact tests and BBS-client rendering | None for current resource path | Non-ASCII caller identity is separate | SF37 §3.3, §5.4 | High-byte preservation plus SyncTERM accepted | VERIFIED |
| A-021 | BBS/CLR selection | IMPLEMENTED | ANSI/text reconnect plus fallback safety | Missing malformed/fallback proof | Complete flow evidence | SF37 §5.4, §5.7 | CLR→BBS→built-in policy and live runs | VERIFIED |
| A-022 | Display fallbacks/resources | PARTIAL | All stock event/security resources | Current-flow precedence | Advanced event/questionnaire resources | SF37 §5.4 | Added login, section, area, transfer, exact-security flows | PARTIAL |
| A-023 | Display controls/macros | PARTIAL | Confirmed table plus safe omissions | Remaining confirmed controls | Obscure non-core macros | SF37 §5.7 | Completed confirmed table and bounded tests | VERIFIED |
| A-024 | Editable MNU menus | IMPLEMENTED | Remapped original identifiers work end to end | Dispatch formerly key-coupled | Future unsupported commands | SF37 §5.5, §9–12 | Immutable-ID dispatch/remap test passed | VERIFIED |
| A-025 | Main/Message/File/Sysop navigation | PARTIAL | Complete stock command breadth | Immediate hot-key semantics | Full Sysop/advanced commands | SF37 §9–12 | Hot keys/current journeys closed | PARTIAL |
| A-026 | `SPITFIRE.HLP` | PARTIAL | Current mappings and corrupt/missing proof | Incomplete current-flow map | Unimplemented advanced records | SF37 §5.1, SFHELP.DOC | Historical map completed for implemented commands | VERIFIED |
| A-027 | Paging/pause/abort | IMPLEMENTED | Context-specific historical key evidence | No current-flow gap | Universal abort key unresolved | SF37 §5.7, §8.2 | Controls/isolation/transfer immunity tested | IMPLEMENTED |
| A-041 | Read new/available messages | PARTIAL | Current/all/queued scans | Scan-mode breadth | Daily scan policy | SF37 §10.2 | Resource/menu regression only | PARTIAL |
| A-045 | Reply/thread relationship | PARTIAL | Stock navigation/quote acceptance | Quote/edit traversal | Same-subject traversal | SF37 §10.2 | Resource/menu regression only | PARTIAL |
| A-047 | SPITFIRE line editor | PARTIAL | Stock edit-command acceptance | Edit/insert/delete | Quote fidelity | SF37 §10.2 | Composition intentionally unchanged | PARTIAL |
| A-048 | “Your Messages” statistics | PARTIAL | Full received/sent prompts/counts | Breakdown/direct-read prompts | Historical presentation | SF37 §10.2 | Help map closed; breadth retained | PARTIAL |
| A-051 | File listings/descriptions | PARTIAL | Historical dates/extended listing | Multi-line/import display | Extended-directory fidelity | SF37 §5.6, §11.2 | Paging/abort/resource regression passed | PARTIAL |
| A-052 | File search | IMPLEMENTED | Full stock wildcard acceptance | No current-flow gap | Historical corner cases | SF37 §11.2 | Security/UI regression retained | IMPLEMENTED |
| A-053 | New-file listing | PARTIAL | Dedicated checkpoint/date input | Last-files-checked state | Historical prompt/date fidelity | SF37 §11.2 | Retained without scope expansion | PARTIAL |
| A-054 | Download/accounting | IMPLEMENTED | Complete external protocol matrix | No principal-path gap | B-024 peer breadth | SF37 §11.2–11.4 | SyncTERM ZMODEM/stream immunity passed | IMPLEMENTED |
| A-055 | Upload/accounting | IMPLEMENTED | Approval/duplicate policy acceptance | No principal-path gap | Sysop approval/duplicate breadth | SF37 §11.2, §11.5 | Staged-transfer proof retained | IMPLEMENTED |
| A-060 | Atomic board state | PARTIAL | Backup/recovery and all stock state | Backup/restore workflow | Complete stock persistence | SF37 §3–5, §9–12 | No unrelated persistence scope added | PARTIAL |
| A-061 | Caller activity/Sysop notices | IMPLEMENTED | Full historical daily reporting | No live-flow gap | Daily log/maintenance views | SF37 §7, §8, §12 | Operator/logging proof retained | IMPLEMENTED |

After closure the same 46 rows are **25 VERIFIED, 9 IMPLEMENTED, 12 PARTIAL,
0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category A**.
Operational stock-board equivalence is demonstrated, but formal stock-core
parity is not declared while the 12 PARTIAL rows retain explicit obligations.

## Caller Policy/Profile/Product-Identity Closure Audit

The approved starting ledger was recounted as **46 total: 25 VERIFIED, 9
IMPLEMENTED, 12 PARTIAL, 0 NOT STARTED, and 0 BLOCKED/RESEARCH**. The closure
did not pull the Category-B questionnaire engine, dormant-caller packing, or
non-time file/chat/quick-logon fields into Category A.

| ID | Initial | Evidence and action | Final |
|---|---:|---|---:|
| A-003 | PARTIAL | Setup/config now controls IANA timezone, public/private access, private security, idle, all daily limits, and Disabled/Optional/Required address/phone/email/birthday policy; a clean-board registration/edit/reconnect run passed. Full questionnaires remain B-004. | VERIFIED |
| A-014 | IMPLEMENTED | SQLite migration 7 preserves existing records and persists structured private contact fields plus four-digit-year birth dates; caller and authorized operator edit paths passed. Legacy `SFUSERS.DAT` import is historical compatibility, not a native record gap. | VERIFIED |
| A-015 | PARTIAL | Private-board mode rejects new/unknown callers and authenticated callers below the configured threshold through `PRIVATE`; locked and limit resources fail closed. Existing menu/conference/file backend enforcement remains green. | VERIFIED |
| A-016 | PARTIAL | Board-local civil days, exact-security MPC/MPD, global per-call/per-day/call-count and first-day caps, local-midnight reset, DST transition, idle `SFASLEEP`, `TOOMANY`, and `SFTIMEUP` paths passed. Non-time DAILYLMT fields belong to their separately tracked capabilities. | VERIFIED |

After closure the ledger is **29 VERIFIED, 8 IMPLEMENTED, 9 PARTIAL, 0 NOT
STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category A**. Formal
stock-core parity is not declared because the nine PARTIAL rows remain A-022,
A-025, A-041, A-045, A-047, A-048, A-051, A-053, and A-060.

## Message-Interaction Fidelity Closure Audit

The approved starting ledger was **46 total: 29 VERIFIED, 8 IMPLEMENTED, and
9 PARTIAL**. The closure resumed preserved, incomplete crash-state work rather
than resetting it. Primary evidence was the SPITFIRE 3.7 manual §10.2 and
§13.1; no QWK/LAKOTA, network-message, full-screen-editor, or legacy-import
scope was added.

| ID | Initial | Evidence and action | Final |
|---|---:|---|---:|
| A-041 | PARTIAL | Implemented the documented This/All/Only Queued conference choices, caller queue editing, mandatory Conference 1, accessible-conference ordering, recipient queue enforcement, persistence, and clean-board Telnet/RAW traversal. | VERIFIED |
| A-045 | PARTIAL | Replies now prompt before changing the subject, preserve the exact subject by default, retain the parent ID, support CTRL+Q line-range quoting with sender initials, and traverse the visible same-subject thread forward/backward/from its start. | VERIFIED |
| A-047 | PARTIAL | The bounded line editor now implements Save, Edit, Abort, Continue, Begin Again, Replace, List, Insert, and Delete while preventing quoted-line edits and retaining CP437 bytes and terminal bounds. | VERIFIED |
| A-048 | PARTIAL | “Your Messages” reports new waiting, already received, sent, and total available; exposes private received/sent lists with status and direct-read prompts; persists idempotent receipts; and supports named-Sysop preview without acknowledging receipt. | VERIFIED |

The fresh Category-A count is **46 total: 33 VERIFIED, 8 IMPLEMENTED, 5
PARTIAL, 0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category
A**. The remaining PARTIAL rows are A-022, A-025, A-051, A-053, and A-060.
Formal stock-core parity is not declared while those rows remain incomplete.

Automatic logon/daily scanning and the board-level all-versus-queued default
remain follow-on breadth; they do not change the verified interactive
This/All/Only Queued choices accepted for A-041. Message deletion, carbon
copies, search, QWK/LAKOTA, network mail, and a full-screen editor remain in
their existing later categories.

## File-Presentation Fidelity Closure Audit

The approved starting ledger was **46 total: 33 VERIFIED, 8 IMPLEMENTED, and
5 PARTIAL**. Primary evidence was the SPITFIRE 3.7 manual §5.6 and §11.2. The
closure retained the native SQLite catalog and completed transfer engines;
archive handling, `FILE_ID.DIZ` import, ratios, batch redesign, and file
networking were not added.

| ID | Initial | Evidence and action | Final |
|---|---:|---|---:|
| A-051 | PARTIAL | Native rows now follow the documented filename/size/date/description columns, use comma-grouped sizes and board-local `MM-DD-YY` dates, preserve safe multiline descriptions at column 34, wrap to terminal width, and retain paging/abort behavior. Normal-setup Telnet and RAW/text listing passed. Legacy `SFFILES.BBS` import and extended-directory storage remain adapter/B work rather than a native presentation gap. | VERIFIED |
| A-053 | PARTIAL | New Files now selects one accessible area/current area or all accessible areas, accepts a dedicated last-check checkpoint or real board-local `MM-DD-YY`/`MM-DD-YYYY` input, reports authorized new/total/byte statistics, advances only completed scans, and persists a monotonic caller-private checkpoint introduced in schema 9 and retained in schema 10 across reconnect. | VERIFIED |

The fresh Category-A count is **46 total: 35 VERIFIED, 8 IMPLEMENTED, 3
PARTIAL, 0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category
A**. The remaining PARTIAL rows are A-022, A-025, and A-060. Formal stock-core
parity is not declared while those rows retain explicit obligations.

## Native Backup/Restore Closure Audit

The approved starting ledger was **46 total: 35 VERIFIED, 8 IMPLEMENTED, and
3 PARTIAL**. Original SPITFIRE 3.7 §5.8 documents automatic conference and
file-area backup copies, while §8/§12 exposes backup-file maintenance. The
native A-060 closure preserves the recovery outcome for all current authority
boundaries without claiming legacy `.$$$` / `.$??` format compatibility,
configuration history, packing/retention, live backup, or external providers.

| ID | Initial | Evidence and action | Final |
|---|---:|---|---:|
| A-060 | PARTIAL | Added cold `backup`/`restore` commands with one OS-backed board-operation lock, a versioned SHA-256 manifest, exact configuration bytes, a consistent and read-only-revalidated current schema-10 SQLite snapshot, complete SYSTEM/DISPLAY resources, and every available/disabled catalog row's independently verified bytes. Restore validates format/schema/migrations/integrity/foreign keys/identity/inventory/confinement/catalog/resources before mutation, stages on the target filesystem, requires explicit `--replace`, and retains rollback until publication succeeds. Normal-setup new-restore/authentication and replacement persistence passed; corruption, traversal, extra/missing content, live-board, and incompatible-schema paths fail closed. | VERIFIED |

The fresh Category-A count is **46 total: 36 VERIFIED, 8 IMPLEMENTED, 2
PARTIAL, 0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category
A**. A-022 and A-025 remain PARTIAL for their explicitly documented resource
fallback and complete historical navigation breadth. Formal stock-core parity
is not declared while those obligations remain.

## Final Resource/Navigation Boundary Closure Audit

The approved starting ledger was **46 total: 36 VERIFIED, 8 IMPLEMENTED, and
2 PARTIAL**. The original manual establishes two distinct layers that the
earlier rows had combined: §5.4 makes caller DISPLAY files optional and says
SPITFIRE normally supplies default output, while §§9–12 define the four
security-filtered Main, Message, File, and Sysop menu boundaries. Bulletin,
questionnaire, Event A–L, door, subscription, full caller-maintenance, and full
Sysop-maintenance behavior remains explicitly classified in B/C/D rather than
being imported into this final Category-A boundary.

| ID | Initial | Historical requirement and pre-change gap | Acceptance evidence | Final |
|---|---:|---|---|---:|
| A-022 | PARTIAL | The implemented core journey already used bounded CLR→BBS→built-in/current-operation fallback, but had no exact-security `SOP<n>` lookup. Section 5.4 documents `SOP<n>.[BCR]`; comprehensive event/questionnaire/bulletin/state resources remain B-001/B-003/B-004/B-005/B-006/B-020. | Added `SOP<n>` selection through the common resource loader. Valid ANSI selects CLR, text and malformed CLR select BBS, and a missing pair renders the security-filtered `SFSYSOP.MNU` menu. Missing/malformed optional display/help content stays bounded; required current operations retain deterministic built-in text. | VERIFIED |
| A-025 | PARTIAL | Main/Message/File transitions were complete, but their historical `@` identifiers ended in the generic unavailable path; `SFSYSOP.MNU`, Sysop entry, and Sysop `Q`→Main were absent. Maintenance commands behind that boundary remain B-016–B-021/C-004 or are modernized by D-006/D-010. | The fourth stock menu now loads by immutable identifiers and command security. Authorized callers enter it from Main, Message, or File; `Q` returns to Main, `G` logs off, and `X` remains session-local. Unsupported supplied historical commands report one bounded unavailable result and remain in context. Ordinary callers cannot see or invoke `@`. Normal-setup Telnet ANSI, RAW/text, reconnect, missing-resource, help, and session-isolation tests passed. | VERIFIED |

The fresh Category-A count is **46 total: 38 VERIFIED, 8 IMPLEMENTED, 0
PARTIAL, 0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category
A**. No Category-A row is now PARTIAL, but formal stock-core parity is **not
yet declared** because the project status definition and release progression
require every Category-A row to be VERIFIED. A-004, A-005, A-006, A-027,
A-052, A-054, A-055, and A-061 remain IMPLEMENTED; this closure added no new
row-specific compatibility/end-to-end evidence that would justify silently
promoting them.

The exact next stock-core action is a verification-only audit of those eight
IMPLEMENTED rows. It must either exercise each row's stated acceptance target
and advance it to VERIFIED or preserve a precise evidence blocker. Development
preview/publication work does not begin until that audit determines the formal
parity checkpoint.

## Category-A Implemented-Row Verification Audit

The approved starting ledger was **46 total: 38 VERIFIED, 8 IMPLEMENTED, and
0 PARTIAL**. This audit added no capability and did not reclassify Category-B
or deployment work as stock core. It compared each remaining row with the
manual, its canonical acceptance target, implementation, automated coverage,
prior external-client evidence, and a new board created through the public
`spitfire setup` command.

| ID | Historical requirement and boundary | Validation executed | Final status / exact blocker |
|---|---|---|---|
| A-004 | Section 4.1 establishes editable numbered conference records with description, threshold/exact read access, entry security, public-only policy, line limit, and privileged levels. Network, routing, purge, physical insert/delete/renumber, and packing behavior remains B/C work. | A clean setup-created board used public `spitfire config` to add an exact-access, public-only conference with distinct read/post levels and a 75-line limit, then reopened the command and reproduced it. The focused admin test additionally creates and reopens a 99-line conference with a privileged level, and proves edit/disable preserves stable identity and existing messages. | VERIFIED |
| A-005 | Section 4.2 establishes numbered areas with description, access mode/security, upload security, preview, free/no-charge, upload bounds, and privileged levels. CD-ROM/extended directories, duplicate policy, move/delete, ratios, and physical relocation remain B rows. | Public `spitfire config` added an exact-access preview/no-charge area with separate read/upload levels, a bounded size, a confined storage key, and two privileged levels; reopening reproduced the area and directory. Existing focused tests prove create/edit/disable preserves stable identity, catalog rows, and storage while relocation/delete fail closed. | VERIFIED |
| A-006 | Sections 3 and 6 establish boot to a ready board, local supervision, orderly caller logoff/reset, explicit termination, and restart support. DOS batch files, AUTOEXEC, COM handling, and host service packaging are deployment/platform mechanisms. | The public executable validated and migrated the clean board, published Telnet/RAW/RLogin listeners and two waiting nodes, reported live status, authenticated a real RAW Sysop session, processed normal Goodbye, stopped at the configured session limit, logged the lifecycle, removed live status, and subsequently reported both nodes offline. Existing operator-console and restart/error tests remain green. | VERIFIED |
| A-027 | Section 5.7 confirms MORE suppression/forcing and display abort enable/disable controls; §8.2 confirms caller More preference. The reviewed primary text says display pauses for a keystroke and may be interrupted, but does not identify one universal abort key or a complete context-specific key table. | Paging/resource/file tests prove negotiated or caller page length, Enter continuation, Q/Escape current-unit abort, no stale input, `PROMPTOFF`/`PROMPT`/`NOABORT`/`ABORTON`, and binary-transfer isolation. | IMPLEMENTED — exact primary evidence or a controlled original-SPITFIRE observation is still required to establish abort keys by context. Q/Escape remains a documented SPITFIRE NG convention and cannot be promoted as historical fact. |
| A-052 | Section 11.2 permits filename `*`/`?` searches except `*.*`, adds `.*` when an extension is omitted, searches descriptions by up to six words, and limits results to available areas/files. | A new focused matrix proves case-insensitive `*`/`?`, extension omission, `*.*` rejection, specific/all-area backend selection, six-word matching, seven-word rejection, and restricted-area non-disclosure. Existing caller-flow tests prove filename and description presentation over the common File menu. | VERIFIED |
| A-054 | Sections 11.2–11.4 require authorized downloads, pre-transfer file/byte presentation, completed-transfer accounting, and protocol selection. Batch queue UI, daily ratios/limits, and complete external-peer protocol breadth remain B-011/B-014/B-024. | Existing tests reran successful ASCII/binary authorization, size/hash preflight, preview denial, changed-byte rejection, interrupted-transfer no-accounting, concurrent-node accounting, and exact protocol engines. Preserved real SyncTERM evidence proves exact ZMODEM and X/YMODEM downloads, SHA-256 integrity, one-time statistics, and clean File-menu return. | VERIFIED |
| A-055 | Sections 11.2 and 11.5 require authorized upload, filename/description collection, completed-transfer catalog/accounting, and safe batch defaults. Sysop-only review, comprehensive duplicate heuristics, ratios/time credit, and full batch UI remain B-011/B-012/B-014/B-024. | Existing tests reran ASCII/binary staging, cancel/disconnect cleanup, bounds, independent SHA-256, duplicate-race exclusivity, authorization recheck, catalog commit, success-only counters, and reconnect. Preserved real SyncTERM evidence proves exact ZMODEM and YMODEM upload paths with clean session return. | VERIFIED |
| A-061 | Sections 7, 8, and 12 establish caller activity visibility, Comment-to-Sysop persistence, page notification, live caller/node inspection, and operator actions. Historical daily log files, date-oriented log views, statistics reports, and maintenance reporting remain B-017/B-021. | The clean public run emitted structured start/login/logoff/end/shutdown records without credentials. Existing focused tests reran durable private Sysop comments, privacy-safe supplied-credential logging, independent/stale-safe page requests, page/chat node states, caller/node/operator inspection, and clean restoration. | VERIFIED |

The resulting Category-A ledger is **46 total: 45 VERIFIED, 1 IMPLEMENTED, 0
PARTIAL, 0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 MODERNIZED within Category
A**. Formal stock-core parity is **not declared** because A-027 still lacks
the historical acceptance evidence required by the VERIFIED definition. No
runtime defect was exposed and no feature implementation changed; the audit
added only focused verification coverage for conference persistence and the
stock file-search matrix.

The exact next stock-core action is to obtain primary, context-specific
historical evidence for A-027 paging continuation/abort keys—through further
preserved documentation/resource analysis or a controlled original SPITFIRE
3.7 runtime observation—then compare it with the existing isolated paging
behavior. Do not change the current Q/Escape convention speculatively, and do
not begin Development Preview or publication work while A-027 remains
IMPLEMENTED.

## A-027 Historical Evidence and Modernization Review

This time-boxed review began from the **46 total: 45 VERIFIED, 1 IMPLEMENTED,
0 PARTIAL** audit checkpoint. It kept page length, waiting for input, output
abort, menu cancellation, and hot-key interruption distinct rather than
searching for an unsupported universal key table.

Primary evidence establishes the complete caller-visible MORE contract:

- manual §5.7 separately defines `PROMPTOFF`, `NOABORT`, `ABORTON`, and
  `PROMPT`, proving that automatic paging and permission to interrupt a display
  are independent controls;
- manual §8.2 says the caller controls a 10–24-line page length and More
  preference, and that output pauses at `< ENTER >` / “More, Continue” until a
  keystroke signals continuation;
- three adjacent Pascal strings in the preserved shareware `SPITFIRE.EXE`,
  beginning at file offset `0x211E0`, compose the exact prompt `MORE:
  <S>top, <N>onstop, < ENTER > to continue?`;
- shipped DISPLAY resources use `PROMPTOFF` and `NOABORT` on bounded menu/
  welcome displays and explicit `PROMPT` at Goodbye, but publish no Q/Escape
  MORE legend; and
- all 55 bounded Pascal records in the preserved `SPITFIRE.HLP` contain no
  universal paging-key table. Other manual contexts name their own keys, such
  as Q for message browsing and Escape for node-chat cancellation.

No original runtime experiment was needed: the primary executable prompt
answers the key question directly, while the manual and resources establish
its surrounding state model. A bounded Synchronet comparison was used only
afterward. Its mature `pause()` implementation accepts a configured Quit key
and Ctrl-C abort while otherwise treating a key as continuation. That supports
Q as a reasonable modern alias but is not evidence about SPITFIRE.

The review exposed one small fidelity defect rather than a system-design gap.
SPITFIRE NG now visibly preserves S=Stop, N=Nonstop for the remainder of the
current output unit, and Enter=continue page by page. Q and Escape remain
undisplayed modern aliases for Stop; both abort only the current unit, never
consume the next command, never cross sessions, and never intercept binary
transfer bytes. `NOABORT` still prevents Stop, `ABORTON` restores it, and the
next output unit restores the caller's More preference.

The executable directly proves the N choice and label, but not whether it
mutated the saved caller preference. Current-unit scope is an intentional
bounded interpretation: §8.2 exposes persistent More preference through caller
settings, while a transient prompt response should not silently rewrite that
record. That ambiguity and the Q/Escape aliases are the documented
modernization portion of the disposition.

**Final disposition: A-027 VERIFIED WITH DOCUMENTED MODERNIZATION.** Historical
intent, exact caller prompt, display controls, implementation, and focused
tests now agree. The aliases improve familiar BBS/terminal cancellation
without replacing or misattributing the stock keys.

The resulting Category-A ledger is **46 total: 46 VERIFIED, 0 IMPLEMENTED, 0
PARTIAL, 0 NOT STARTED, 0 BLOCKED/RESEARCH, and 0 status-level MODERNIZED
rows**. Every Category-A row satisfies its evidence target. **SPITFIRE NG Stock
SPITFIRE 3.7 Core Parity is achieved.** Category-B and later historical/
ecosystem breadth remains explicitly open and is not part of this declaration.

### Current stock resource-selection model

For each logical display stem, the loader reads bounded optional resources
case-insensitively. An ANSI-capable transport may load a valid `.CLR` over the
same-stem `.BBS`; text-only capability loads only `.BBS`. At session time the
persisted caller graphics choice is intersected with transport capability, so
ANSI preference never invents ANSI support. Missing, oversized, or malformed
preferred resources fall to a valid same-stem `.BBS`, then to bounded built-in
or current-operation text for required core displays. Main, Message, and File
menus remain required, bounds-checked configuration inputs. `SFSYSOP.MNU` uses
the same parser when present and a security-filtered `Q`/`X`/`G` built-in only
when missing or malformed so pre-closure native boards remain startable.
Missing/corrupt help uses contextual fallback text.

The implemented stock journey is `SFPRELOG` → `WELCOME1` → authentication →
`NEWUSER` on a first session or `WELCOME2`…`WELCOME9` when present → `ALL` →
`SFNOD<n>` → `<security>SEC` → caller-number display → exact-security
`MAIN<security>`, `MSG<security>`, `FILE<security>`, or `SOP<security>`.
Section and operation resources include `SF1STM`, `SF1STF`, `SFMSG<n>`,
`SFIL<n>`, `SFDOWN`, `SFUP`, page/chat resources, and `GOODBYE`. No proprietary
resource is required by a new board.

## Stock Operating Model

A stock board combines six layers:

1. a Sysop configures board identity, logical paths, caller policy, message
   conferences, file areas, resources, and optional services;
2. a node accepts a caller and establishes terminal/session properties;
3. the caller registers or authenticates and receives a security- and
   time-limited persistent session;
4. editable displays, menus, macros, and help drive a recognizable SPITFIRE
   interface;
5. message, file, Sysop-contact, and account workflows share one caller model;
6. logout persists caller, message-read, usage, and board state.

The initial native implementation must preserve this composition. Implementing
one subsystem in isolation is not a substitute for a board that runs.

## Stock File Capability Classification

Increment 5 refined the file scope directly from SF37 §3.2–3.3, §4.2,
§5.6, §11, §16, and the documented companion-utility sections.

| Classification | Documented file capabilities | Increment 5 treatment |
|---|---|---|
| **Required for this increment** | Multiple numbered areas; names/descriptions; threshold/exact/privileged and preview access; list; filename wildcard and one-to-six-word description search; new-file foundation; authorized download/upload; persistent catalog and successful-transfer statistics | IMPLEMENTED with SQLite metadata, confined host storage, stock ASCII text download/upload, staging, and end-to-end Telnet/raw tests. The later A-051/A-053 closure added stock row/date/multiline presentation and the dedicated last-files-checked/date-input workflow. |
| **Stock-core follow-up** | Tag/batch queues; XMODEM checksum/CRC, 1K-XMODEM, YMODEM, ZMODEM, `-g` variants, and Telink; ratios/daily limits/time credit; comprehensive duplicate checks; Sysop-only/approval workflows; archive/text view; CD-ROM/extended directories | Explicit B work. ASCII is one documented usable path; full stock protocol breadth is not claimed. |
| **Optional / ecosystem** | External transfer-protocol drivers and companion tools such as `SFCHKUP`/`SFFILES` maintenance | Adapter/utility work after the native stock-core journey. No external program is executed by the core. |
| **Historical mechanism to modernize** | DOS drive/current-directory assumptions, separate upload/download directories, copying CD-ROM files through WORK, serial transfer-time estimates, DOS disk-free handling, direct listing-file mutation | Logical paths and confined storage, per-session staging, streaming, transaction-safe accounting, modern quota/disk error handling, and stable metadata replace the mechanism while preserving caller-visible area/transfer behavior. |

The stock internal protocol list is: ASCII, XMODEM checksum, XMODEM CRC,
1K-XMODEM, YMODEM batch, ZMODEM batch, 1K-XMODEM-g, YMODEM-g batch, and
Telink. These are now implemented behind one binary-session contract; actual
SyncTERM 1.9rc4 ZMODEM upload/download and current SyncTERM 1.10a XMODEM
checksum/CRC/1K, YMODEM single/batch, and YMODEM-g download are verified while
the remaining real-client variants stay explicit. See the
[transfer specification](sfng-file-transfers.md) and
[native file specification](sfng-file-system.md).

## A — Stock Core / Near-Term Parity

### Board, configuration, and runtime

| ID | Documented capability | Manual evidence | Status | SPITFIRE NG acceptance target |
|---|---|---|---|---|
| A-001 | Create/configure a board and designate the Sysop | SF37 §3.1–3.4 | VERIFIED | The non-proprietary `setup` workflow creates validated configuration, logical paths, schema, Sysop credential, starter resources, conferences, and file areas. Increment 6 repeated the integrated journey from a clean setup-created board. See `sfng-setup-configuration.md`. |
| A-002 | Logical SYSTEM, WORK, MESSAGE, DISPLAY, menu/home, and file paths | SF37 §3.4, §5.0–5.4 | VERIFIED | Increment 0 resolves and creates the five stock logical paths behind a host-independent API with traversal validation. |
| A-003 | Configure caller defaults and policy | SF37 §3.2–3.3 | VERIFIED | Setup/config exposes security, per-call/per-day/first-day/daily-call and idle limits, IANA timezone, public/private mode, private security, and Disabled/Optional/Required address/phone/email/birthday groups through one validator. The clean-board policy journey passed; `SFNEWU.QUE` remains B-004. |
| A-004 | Configure message conferences | SF37 §4.0 | VERIFIED | Public clean-board configuration added and reopened an exact-access/public-only conference with independent read/post levels and line limit. Focused service tests additionally prove privileged levels, stable identity, message preservation, and safe enable/disable. Network/retention/packing breadth remains follow-on. |
| A-005 | Configure file areas | SF37 §4.2 | VERIFIED | Public clean-board configuration added and reopened a confined exact-access preview/no-charge area with independent upload security, size bound, and privileged levels. Focused tests prove stable identity/catalog/storage preservation and fail-closed delete/relocation. |
| A-006 | Start, stop, and supervise the board | SF37 §3, §6 | VERIFIED | The public native executable validated/migrated a setup-created board, published named listeners and two waiting nodes, reported live status, authenticated and logged off a RAW caller, shut down cleanly, removed live status, and reported both nodes offline. Operator-console/restart/error tests remain green; host service packaging is deployment work. |
| A-007 | Node/session identity and basic status | SF37 §3.2, §20 | VERIFIED | A race-safe configured pool assigns the lowest free enabled node independently of transport, publishes waiting/connecting/login/online/disconnecting state, rejects a fifth caller when four nodes are occupied, and reuses a released node. Historical 1–255 numbering is preserved without becoming the native limit. See `sfng-multinode-runtime.md`. |

### Connection, login, and caller lifecycle

| ID | Documented capability | Manual evidence | Status | SPITFIRE NG acceptance target |
|---|---|---|---|---|
| A-010 | Remote caller connection | SF37 §2–3, §6 | VERIFIED | Telnet, raw TCP, and RLogin loopback clients enter the same session engine; Unix stdio uses that engine locally without granting BBS identity. |
| A-011 | Recognizable startup and pre-logon sequence | SF37 §5.4, §8.2 | VERIFIED | Synthetic Welcome resources now lead into the same explicit new/existing caller flow over every adapter before Main-menu access; Telnet acceptance verified the ordering. Full historical display-resource breadth remains B-006. |
| A-012 | Existing-caller login | SF37 §3, §8, §9 | VERIFIED | Case-insensitive normalized names and Argon2id credentials authenticate one persisted caller through the common engine; wrong, unknown, disabled, EOF, and bounded retry paths are covered. |
| A-013 | New-caller registration | SF37 §3, §7, §8.2 | VERIFIED | A caller creates a unique name and modern credential, supplies only Sysop-enabled profile groups, satisfies required validation, receives configured security/time defaults, reaches Main, and reconnects. Full questionnaires remain B-004. |
| A-014 | Persistent caller record | SF37 §3, §8.2, §9.2 | VERIFIED | Stable identity, state, statistics, preferences, structured address, phone, email, and unambiguous birth date persist through SQLite schema 7. Native migration and edit/reconnect passed; historical-record import remains compatibility work. |
| A-015 | Security levels and per-command/resource access | SF37 §3.2, §4, §5.5, §7, §9–12 | VERIFIED | Validated 0–9999 levels centrally gate private-board admission, `.MNU`, conference, and file operations, including exact/preview/privileged exceptions. Rejected private/locked/limited callers receive bounded stock resources and no Main access. |
| A-016 | Time limits and inactivity handling | SF37 §3.2, §8.2, §9.2 | VERIFIED | Exact-security MPC/MPD, global per-call/per-day/call-count limits, first-day caps, monotonic elapsed time, and transport idle timeout use a configured IANA board-local day. Local-midnight/DST reset and `SFASLEEP`/`TOOMANY`/`SFTIMEUP` paths passed. Dormant-caller packing and non-time DAILYLMT fields remain separate B/C work. |
| A-017 | Normal disconnect, logoff, and reconnect | SF37 §3, §9–12 | VERIFIED | Clean/failed authentication, EOF, common Goodbye, runtime errors, and simulated carrier loss release the acquired node; the same caller reloads with incremented persistent call state. |

### Terminal, displays, menus, and help

| ID | Documented capability | Manual evidence | Status | SPITFIRE NG acceptance target |
|---|---|---|---|---|
| A-020 | CP437-oriented text terminal behavior | SF37 §3.3, §5.4, §9–12 | VERIFIED | Legacy resource bytes remain byte-exact instead of being silently converted to UTF-8. Synthetic high-byte tests and current SyncTERM rendering verify the boundary; modern database strings remain Unicode. |
| A-021 | `.BBS` text and `.CLR` ANSI displays | SF37 §5.4, §5.7 | VERIFIED | Effective graphics mode is caller preference intersected with terminal capability. Valid `.CLR` wins only for ANSI mode; `.BBS` serves text mode and preferred-resource failure. Missing/malformed cases, reconnect persistence, raw text, and SyncTERM ANSI passed. |
| A-022 | Stock-core display fallbacks and security-specific resources | SF37 §5.4 | VERIFIED | Startup/login, message/file selection, transfer, page/chat, and logoff retain compatible resource fallback. M037 makes exact-security Main/Message/File/Sysop art an optional board/active leaf override over a first-class engine-generated `.MNU` menu; missing/malformed/unsupported art, arbitrary security levels, ANSI/text, and reconnect pass. Comprehensive event/questionnaire/bulletin/caller-state resource breadth remains B-006 and its owning advanced rows. |
| A-023 | SPITFIRE display controls and macros | SF37 §5.7 | VERIFIED | Confirmed controls are bounded. City/region, phone, and birth macros expand only from the active caller's private self-context; password remains unavailable, unrelated caller/status/message contexts have no contact values, and unknown macros stay byte-exact. |
| A-024 | Editable `.MNU` menu resources | SF37 §5.5, §9–12 | VERIFIED | Bounds-checked `.MNU` parsing drives displayed keys, descriptions, security, and immutable historical command identifiers. A remapped command-letter test proves action/help dispatch follows the identifier rather than a hard-coded key. |
| A-025 | Main, Message, File, and Sysop navigation | SF37 §9–12 | VERIFIED | Main, Message, File, and security-controlled Sysop menus share one engine. Historical `@` identifiers enter Sysop from all three caller sections; Sysop `Q` returns to Main, `G` logs off, and `X` is session-local. Unavailable supplied commands stay bounded without stale input. Full maintenance and advanced command behavior remains B-002–B-021/C-003/C-004. |
| A-026 | `SPITFIRE.HLP` command help | SF37 §5.1, §9–12 | VERIFIED | The 55-by-366-byte format is bounds-checked. The SFHELP record map now covers every implemented stock command; command remapping, missing/corrupt files, bounded paging, and contextual fallback are tested. HOME-key/advanced-command help remains with those non-Category-A command surfaces. |
| A-027 | Screen paging, pause, and abort behavior | SF37 §5.7, §8.2; preserved EXE prompt | VERIFIED | The executable's Pascal strings at `0x211E0` establish `S`top, `N`onstop, and Enter. Negotiated/caller height drives MORE across resources/help/messages/files; Stop aborts only the current unit, Nonstop suppresses only its remaining prompts, and the next unit restores preference. `PROMPTOFF`, `PROMPT`, `NOABORT`, and `ABORTON` remain isolated; Q/Escape are documented modern Stop aliases; binary transfer bypasses paging/hot keys. |

### Caller-facing main section

| ID | Documented capability | Manual evidence | Status | SPITFIRE NG acceptance target |
|---|---|---|---|---|
| A-030 | Main-menu entry and command security | SF37 §9.0–9.2 | VERIFIED | Main is unavailable before authentication; after login, the resource-driven menu filters commands against the authoritative caller security level. |
| A-031 | Caller statistics/preferences | SF37 §9.2 | VERIFIED | `Your Statistics` shows persisted identity/use/message/file counts. Main `U` safely edits graphics/text, width, page length, MORE, scroll prompt, and hot-key preference; reconnect verifies persistence and capability precedence. Network/serial adapters accept immediate menu keys while line mode remains available and binary/password input is isolated. |
| A-032 | Private comment/message to Sysop | SF37 §9.2 | VERIFIED | Main-menu Comment to Sysop resolves the configured active Sysop caller and stores a non-public `SysopComment` in Conference 1; persistence and Sysop visibility passed the end-to-end fixture scenario. |
| A-033 | Page and text-chat with Sysop | SF37 §8.2, §9–11 | VERIFIED | Main `P` uses stock-style unavailable/unanswered/already-paged resources. Stable-session requests can be answered/declined in the operator console; bidirectional chat returns the caller to the prior BBS context. Concurrent and stale-session behavior is tested. |

### Messages and conferences

| ID | Documented capability | Manual evidence | Status | SPITFIRE NG acceptance target |
|---|---|---|---|---|
| A-040 | Multiple message conferences and access checks | SF37 §4.0, §10.2 | VERIFIED | The fixture exposes General and SPITFIRE; callers list/change conferences with unread counts. Backend threshold/exact/privileged checks and denied access are tested independently of menu visibility. |
| A-041 | Read new/available messages | SF37 §10.2 | VERIFIED | Read offers This/All/Only Queued accessible-conference scans in conference/message order, starts at the first unread visible message, and supports next/previous/direct-number movement. Caller queues persist, Conference 1 is mandatory, and normal-setup Telnet/RAW acceptance passed. Automatic logon/daily scan defaults remain follow-on breadth. |
| A-042 | Last-message-read persistence | SF37 §5.2, §10.2 | VERIFIED | SQLite high-water state persists per caller/conference across logoff/reconnect and can advance only after an authorized read by that caller. |
| A-043 | Browse and select messages | SF37 §10.2 | VERIFIED | Browse shows visible To/From/Subject headers and unread/private state; Read opens an authorized caller-facing message number. |
| A-044 | Post messages | SF37 §10.2 | VERIFIED | Authenticated callers address All Callers or an active local caller, choose public/private where permitted, enter bounded subject/body data, confirm save, or abort without partial storage. Author identity comes only from the authenticated actor. |
| A-045 | Reply and preserve subject-thread relationship | SF37 §10.2 | VERIFIED | Replies retain a parent identity, prompt before changing the subject, preserve exact subject/visibility defaults, support CTRL+Q line-range quotes with sender initials, and traverse visible exact-subject threads from start/forward/back before returning to the original message. |
| A-046 | Private messages and messages to the Sysop | SF37 §4.0, §9.2, §10.2 | VERIFIED | Backend list and direct-read authorization restrict non-public content to author, recipient, and Sysop. Conference 1 Sysop comments use the same durable model and passed cross-caller acceptance. |
| A-047 | SPITFIRE-style line editor | SF37 §10.2 | VERIFIED | The shared bounded composer implements Save/Edit/Abort/Continue/Begin Again/Replace/List/Insert/Delete, immutable quoted ranges, `/S`/`/A`, size/line limits, CP437 preservation, and disconnect-safe cancellation without a full-screen or external editor. |
| A-048 | Message statistics / “Your Messages” | SF37 §10.2 | VERIFIED | `Your Messages` reports new waiting/already received/sent/total available, lists private received and sent headers/status, directly opens a conference/message, persists idempotent receipts, and lets the configured named Sysop preview without receipt or last-read mutation. |

### Files and file areas

| ID | Documented capability | Manual evidence | Status | SPITFIRE NG acceptance target |
|---|---|---|---|---|
| A-050 | Multiple file areas and access checks | SF37 §4.2, §11.2 | VERIFIED | General Files and SPITFIRE Files are configured data, not business-logic constants. Caller area selection, threshold/exact/privileged checks, preview-only visibility, disabled areas, and backend denial passed synthetic and end-to-end tests. |
| A-051 | File listings and descriptions | SF37 §5.6, §11.2 | VERIFIED | Terminal-height-aware native rows follow stock filename/size/date/description columns, use board-local `MM-DD-YY`, and render safe multiline descriptions from column 34 with wrapping and abort. Normal-setup Telnet and RAW/text acceptance passed. Legacy listing import and extended-directory storage remain separate adapter/B work. |
| A-052 | Find files and search descriptions | SF37 §11.2 | VERIFIED | Focused stock-search validation proves case-insensitive `*`/`?`, omitted-extension `.*`, `*.*` rejection, specific/all-area backend selection, one-to-six-word description matching, over-limit rejection, and restricted-area non-disclosure; existing caller-flow presentation remains green. |
| A-053 | New-file listing | SF37 §11.2 | VERIFIED | The File menu scans one accessible area/current area or all areas from a caller-entered operational date or dedicated last-files-checked checkpoint, reports authorized new/total/byte statistics, and persists completed scans monotonically from schema 9 onward across reconnect. |
| A-054 | Download authorization and accounting | SF37 §3.2, §4.2, §11.2–11.4 | VERIFIED | ASCII/binary success and failure tests prove authorization, path/hash/size preflight, preview denial, changed-byte/interruption no-accounting, concurrency, and success-only atomic counters. Preserved real SyncTERM X/Y/ZMODEM evidence proves exact bytes/SHA-256/statistics and clean return; remaining whole-family peer breadth stays B-024. |
| A-055 | Upload authorization, description, and accounting | SF37 §3.2–3.3, §4.2, §11.2, §11.5 | VERIFIED | ASCII/binary tests prove caller filename/description input, bounded per-session staging, cancel/disconnect cleanup, authorization recheck, SHA-256, duplicate-race exclusivity, catalog commit, and success-only accounting. Preserved real SyncTERM ZMODEM/YMODEM evidence passes; approval/heuristic/batch breadth remains B work. |
| A-056 | Normal return and logoff from file workflows | SF37 §11.2 | VERIFIED | Functional list/search/download/upload paths return to Message/Main or the shared Goodbye lifecycle and persist across reconnect. |

### Persistence and integrated acceptance

| ID | Documented capability | Manual evidence | Status | SPITFIRE NG acceptance target |
|---|---|---|---|---|
| A-060 | Atomic persistent board state | SF37 §3–5, §5.8, §9–12 | VERIFIED | Cold native backup/restore preserves exact validated TOML, a consistent integrity-checked current schema-10 SQLite snapshot, complete SYSTEM/DISPLAY resources, and all cataloged bytes behind one board-wide OS lock. A versioned SHA-256 manifest, full pre-mutation validation, same-filesystem staging, explicit replacement, rollback, new-board restore, caller reconnect, and corruption/confinement failure tests pass. Transient status/upload staging are deliberately excluded; legacy backup formats/history remain B-025. |
| A-061 | Caller activity and Sysop notifications | SF37 §7, §8, §12 | VERIFIED | A public clean-board run emitted privacy-safe start/login/logoff/end/shutdown records. Focused tests prove durable private Sysop comments, secret-free failure logging, live/stale-safe pages, page/chat node states, caller/node/operator inspection, and clean restoration. Full historical daily log/maintenance reporting remains B-017. |
| A-062 | End-to-end stock-core demonstration | SF37 §3–12 | VERIFIED | A clean non-fixture board created through the normal setup service completed Sysop initialization, caller creation/reconnect, preference persistence, messages/reply/Comment-to-Sysop, file browse/download/upload/statistics, live page/chat, node release, and clean shutdown through the real engine. |

## B — Stock Advanced / Follow-On

These remain stock-parity obligations, but they follow the first integrated
board rather than blocking its first useful run.

| ID | Capability | Manual evidence | Status | Notes / acceptance direction |
|---|---|---|---|---|
| B-001 | Public/private-board admission, lockout, subscription expiry/warnings | SF37 §3.2–3.3, §5.4, §8.2 | PARTIAL | Public/private admission and disabled-caller lockout now use the normal verifier plus bounded stock resources. Subscription-expiry dates, warning intervals, and operator renewal remain advanced caller-maintenance work using modern unambiguous dates. |
| B-002 | Full caller directory/partial-name locate and “Other BBS” directory | SF37 §9.2 | NOT STARTED | Privacy-aware configuration must govern personal fields. |
| B-003 | Bulletins, newsletter, system information, random `THOUGHTS.BBS` | SF37 §5.4, §5.9, §9.2 | NOT STARTED | Retain resource-driven presentation. |
| B-004 | New-user and up to 24 order questionnaires | SF37 §7 | NOT STARTED | Preserve question/branch semantics; do not preserve printer dependence or unsafe automatic privilege escalation without policy controls. |
| B-005 | `.RIP` display selection and RIP-oriented command resources | SF37 §3.3, §5.4 | NOT STARTED | Historical terminal capability; implement after text/ANSI path is stable. |
| B-006 | Comprehensive display-resource precedence | SF37 §5.4 | NOT STARTED | The Category-A current/core journey is verified. This row retains the full caller-, bulletin-, questionnaire-, door-, event-, subscription-, maintenance-, and other advanced state-specific resource inventory plus RIP precedence. |
| B-007 | Conference queue editing and mandatory conference 1 | SF37 §10.2 | NOT STARTED | Preserve queue semantics and new-message scan behavior. |
| B-008 | Message search by caller or up to six text terms | SF37 §10.2 | NOT STARTED | Enforce private/deleted visibility during search. |
| B-009 | Message threads, carbon copies, delete/undelete, move/copy/forward | SF37 §10.2, §13 | NOT STARTED | Preserve caller/Sysop permissions and revision/audit semantics. |
| B-010 | Sysop message preview without marking received | SF37 §10.2, §13 | NOT STARTED | Read-only preview must not advance recipient state. |
| B-011 | File tagging and batch queues | SF37 §11.2, §11.4–11.5 | NOT STARTED | Preserve queue workflow independently of specific serial protocols. |
| B-012 | File requests, duplicate upload detection, Sysop-only uploads | SF37 §5.6, §11.2 | PARTIAL | Exact case-insensitive same-area filename duplicates are race-safely rejected. File requests, digit-stripped/base-name heuristics, and Sysop validation/review remain. |
| B-013 | Read text files and inspect ZIP archives | SF37 §11.2 | NOT STARTED | Bounds-check archive metadata and content rendering. |
| B-014 | File ratios, daily limits, no-charge areas, preview areas, upload time credit | SF37 §3.2, §4.2, §11 | PARTIAL | Preview and no-charge area behavior are implemented. Ratios, daily transfer limits, and upload time credit remain; serial-speed estimates will not be authoritative. |
| B-015 | File move/delete/listing maintenance | SF37 §11.2 | NOT STARTED | Privileged operations require confirmation, audit, and path confinement. |
| B-016 | Caller record add/edit/lockout/purge protection | SF37 §8.2, §12 | NOT STARTED | Preserve history/audit rather than destructive silent edits. |
| B-017 | Sysop logs, daily statistics, notifications, maintenance views | SF37 §8.2, §12 | NOT STARTED | Native structured logs may back SPITFIRE-style views. |
| B-018 | Message/caller packing and purge policies | SF37 §4, §12 | NOT STARTED | Modern storage need not require physical packing, but must reproduce retention/deletion outcomes and provide legacy maintenance adapters. |
| B-019 | Multi-node active-caller list, Who’s On, node-to-node messages/chat | SF37 §9.2, §20 | NOT STARTED | Build on native concurrent sessions and node registry. |
| B-020 | Stock events A–L, enable/disable/list, caller exclusion windows | SF37 §14 | NOT STARTED | Implement in the modern scheduler after core session stability; no DOS batch/errorlevel dependence. |
| B-021 | Local/Sysop operator controls | SF37 §8, §12–13 | PARTIAL | `spitfire console` provides status, page availability/answer/decline/chat, targeted disconnect, caller list, enable/disable, security change, and clean exit. Time grants, richer maintenance, attachable control IPC, and complete stock command breadth remain. |
| B-022 | Screen/export/print-oriented operations | SF37 §10, §12 | NOT STARTED | Preserve export/report outcomes; printers are optional destinations, not core dependencies. |
| B-023 | CD-ROM/extended download-directory behavior | SF37 §4.2, §5.6 | NOT STARTED | Model read-only and extended areas without staging through DOS work-space constraints. |
| B-024 | Built-in ASCII, XMODEM variants, YMODEM variants, ZMODEM, and Telink transfer protocols | SF37 §11.3–11.5 | PARTIAL | All nine stock internal choices are implemented. Actual SyncTERM verifies ASCII, XMODEM checksum/CRC/1K, YMODEM single/batch, YMODEM-g download, and ZMODEM single-file upload/download with exact SHA-256; controlled peers cover XMODEM-g, bidirectional YMODEM-g batch, ZMODEM batch, and Telink. A real ZMODEM multi-file client path, second external Telink/1K-XMODEM-g peer, and external YMODEM-g upload/batch remain before the whole family is VERIFIED. See `sfng-file-transfers.md`. |
| B-025 | Configuration backup/recovery files and operator recovery workflow | SF37 §5.8, §12 | PARTIAL | The native cold full-board workflow now provides explicit validated backup/new restore/replacement and subsumes current configuration recovery. Automatic legacy `SFMCONF.$$$`, `SFFAREA.$$$`, and `SFUSERS.$??` compatibility plus transactional configuration history remain adapter/advanced work. |

## C — Optional / Companion / Network / Add-On

These are valuable historical/ecosystem compatibility targets, not blockers for
stock-core operation.

| ID | Capability | Manual evidence | Status | Planned treatment |
|---|---|---|---|---|
| C-001 | LAKOTA QWK download/upload and pointer management | SF37 §10.2, §19.3, §24.4 | DEFERRED | Historical/ecosystem phase using the shared message abstraction. |
| C-002 | Net-mail, front-end mailers, and UTI drivers | SF37 §4.0, §10, §19 | DEFERRED | Network adapters; FidoNet and other targets remain separately specified. |
| C-003 | Doors and `SFDOORS.DAT`/industry drop files | SF37 §15, §24.2 | DEFERRED | Isolated legacy runtime; never execute DOS doors inside the core process. |
| C-004 | Main/Message/File/Sysop menu extensions and batch hooks | SF37 §9–12 | DEFERRED | Controlled extension interface after stock commands are complete. |
| C-005 | External transfer-protocol drivers | SF37 §16 | DEFERRED | Adapter boundary after safe native transfer capability. |
| C-006 | `DAILYLMT`, `SFSENDIT`, `SFPCKUSR`, `SFPCKMSG` companions | SF37 §24 | DEFERRED | Reproduce required outcomes natively or add format/runtime compatibility as justified. |
| C-007 | Fax-call handling | SF37 §22 | DEFERRED | Not a stock-core network transport; any future integration uses modern external services. |
| C-008 | CircuitNet | Separate preserved corpus | DEFERRED | Historical/ecosystem phase; see [CircuitNet](09-circuitnet.md). |
| C-009 | FidoNet, SMB, DOVE-Net, SSH, web terminal/admin | Project roadmap | DEFERRED | Later historical/ecosystem or modern-enhancement phases; not claims about stock 3.7. |
| C-010 | Optional GUI administration and registration-manager integration | Project roadmap | DEFERRED | Later clients over shared Rust APIs; not part of stock board parity. |

## D — Historical Implementation Details to Modernize

| ID | Historical mechanism or limitation | Manual evidence | Status | SPITFIRE NG replacement |
|---|---|---|---|---|
| D-001 | Modem initialization strings, result codes, rings, DTR, carrier, COM/IRQ/base addresses | SF37 §2–3 | NOT APPLICABLE / MODERNIZED | Native transport adapters are primary. An optional inbound Hayes controller above direct serial preserves `RING`/`ATA`/`CONNECT`/carrier outcomes; it is simulation-tested but hardware-unverified. |
| D-002 | Serial baud, UART buffering, hardware/software flow control | SF37 §2–3, §17 | NOT APPLICABLE / MODERNIZED | The core receives capability metadata rather than COM assumptions. A direct-serial adapter is synthetic-PTY tested and remains physical-hardware-unverified. |
| D-003 | DOS hardware/memory requirements, overlays, EMS | SF37 §2, §18.2 | NOT APPLICABLE / MODERNIZED | Native protected-memory Rust process and ordinary host virtual memory. |
| D-004 | DOS drive letters, 8.3 names, current-directory dependence | SF37 §3.4, §5, §6 | NOT APPLICABLE / MODERNIZED | Logical SPITFIRE path abstraction with canonicalized, confined cross-platform host paths; adapters recognize historical layouts. |
| D-005 | `SF.BAT` restart loop, shelling, exit/error levels, reboot behavior | SF37 §6, §14–16 | NOT APPLICABLE / MODERNIZED | Native lifecycle, structured errors, supervised service operation, and job APIs. |
| D-006 | Remote or local “Drop to DOS” host shell | SF37 §8, §12 | NOT APPLICABLE / MODERNIZED | No caller-accessible host shell; authenticated, least-privilege admin actions and isolated external runtimes only. |
| D-007 | Plaintext/weak password storage, birthdate as second password, password display macro | SF37 §3.3, §5.7 | NOT APPLICABLE / MODERNIZED | Modern password hashing, migration markers, rate limiting, and no credential display. Birthdate is optional profile data, never an authentication secret. |
| D-008 | Direct video memory, local color-monitor switch, split-screen assumptions | SF37 §3.3, §8 | NOT APPLICABLE / MODERNIZED | Terminal capability abstraction plus separate portable operator view. |
| D-009 | PC speaker/page bell, keyboard function-key dependence | SF37 §8 | NOT APPLICABLE / MODERNIZED | Event notifications and portable admin commands while retaining page/chat semantics. |
| D-010 | Printer readiness and hard-copy output | SF37 §7, §10, §12 | NOT APPLICABLE / MODERNIZED | Durable reports/export streams; optional printing outside the core. |
| D-011 | Process-per-node directories, RAM-drive chat files, DOS SHARE/record locking, multitasker tuning | SF37 §20–21 | NOT APPLICABLE / MODERNIZED | Concurrent session registry, transactional persistence, async synchronization, and isolated per-session working state. |
| D-012 | CD-ROM cache/copy-to-WORK constraints and free-space assumptions | SF37 §4.2, §11 | NOT APPLICABLE / MODERNIZED | Read-only area/storage capabilities and streamed transfers without DOS media staging. |
| D-013 | Modem Caller ID parsing | SF37 §18.3 | NOT APPLICABLE / MODERNIZED | Optional transport connection metadata with explicit privacy/retention policy. |
| D-014 | DOS screen saver | SF37 §18.1 | NOT APPLICABLE / MODERNIZED | Host display/power management; operator UI may independently redact idle screens. |
| D-015 | Two-digit DOS dates and obsolete pivot behavior | Technical dossier; stock records | NOT APPLICABLE / MODERNIZED | Four-digit internal dates and explicit, tested conversion policy at legacy boundaries. |
| D-016 | Serial transfer-time estimates as authoritative limits | SF37 §11 | NOT APPLICABLE / MODERNIZED | Policy checks based on quotas and safe transfer lifecycle; compatibility protocols may report estimates informationally. |
| D-017 | Fixed limits caused by static arrays, memory, or disk capacity | Throughout SF37 | NOT APPLICABLE / MODERNIZED | Preserve limits only where caller-visible semantics require them; otherwise use validated configurable limits and resource quotas. |
| D-018 | Local import/export by arbitrary drive/path | SF37 §10–11 | NOT APPLICABLE / MODERNIZED | Confined administrative import/export with explicit source/destination authorization. |

## First Major Acceptance Demonstration

The first major runnable checkpoint passes only when the following works as one
board, not as disconnected demonstrations:

1. start SPITFIRE NG with a configured board and at least two message
   conferences and two file areas;
2. connect from another terminal using Telnet;
3. receive recognizable SPITFIRE startup/logon resources;
4. create a new caller;
5. disconnect, reconnect, and authenticate as that caller;
6. navigate the stock Main, Message, and File sections through menu resources;
7. enter multiple permitted conferences, read seeded messages, post, reply,
   and send a private message/comment to the Sysop;
8. disconnect and confirm last-read state persists;
9. browse multiple permitted file areas and see names/descriptions;
10. download a file if a safe interoperable initial transfer path is ready;
11. log off normally, reconnect, and observe persistent caller, message, file,
    and board state.

The ASCII path satisfies the original minimum transfer requirement. Actual
SyncTERM X/Y/ZMODEM acceptance now adds verified practical binary transfer;
the remaining real-client and non-SyncTERM variant matrix stays PARTIAL under
B-024 rather than being implied by that result.

## Increment 0–6 Verification

The runtime foundation is implemented in
[`sf-core`](../crates/sf-core/src/lib.rs) and the
[`sf-bbs` application](../crates/sf-bbs/src/lib.rs). Synthetic tests cover
configuration, logical paths, migrations, persisted board identity, nodes,
session lifecycle, fixture creation, expected errors, and clean shutdown.
Increment 1 additionally covers transport negotiation and metadata, node
contention, legacy resource parsers/rendering, and menu traversal. Increment 2
adds SQLite caller migration, Argon2id credentials, explicit authentication
state, new/existing caller flows, security/time policy, statistics, and
reconnect persistence. See the
[caller/authentication specification](sfng-caller-authentication.md). The
Increment 3 message journey is documented in
[the native message specification](sfng-message-system.md). Increment 4 adds
real setup/configuration/status commands, safe conference administration, and
the shared multinode pool documented in
[the setup specification](sfng-setup-configuration.md) and
[multinode specification](sfng-multinode-runtime.md). The
[Increment 5 file journey](sfng-file-system.md) adds schema 4, file-area
administration, confined storage, protected search/listing, staged uploads,
verified ASCII transfers, successful-transfer statistics, and transfer-aware
node status. Increment 6 adds schema 5 caller preferences, shared MORE/abort,
stable-session page/chat, and essential operator commands documented in the
[caller/Sysop interaction specification](sfng-caller-sysop-interaction.md).
The promoted binary-transfer increment adds schema 6, binary Terminal stream
ownership, all documented internal protocol engines, controlled-peer coverage,
and actual SyncTERM X/Y/ZMODEM acceptance documented in
[the transfer specification](sfng-file-transfers.md).
The resource/menu closure adds the stock current-flow display precedence,
complete confirmed macro table, immutable `.MNU` identifier dispatch, current
`SPITFIRE.HLP` mapping, immediate network/serial hot keys, per-resource
prompt/abort controls, and malformed/missing-resource fallbacks. A clean
setup-created board selected `.CLR` in current SyncTERM 1.10a, persisted text
mode over raw TCP, and repeated a successful ZMODEM download before returning
cleanly through Files, Main, and Goodbye.
The
manual development commands use:

```text
cargo run -p sf-bbs -- init-fixture ./var/fixture-board
cargo run -p sf-bbs -- setup ./var/my-board
cargo run -p sf-bbs -- config ./var/my-board/spitfire.toml
cargo run -p sf-bbs -- status ./var/my-board/spitfire.toml
cargo run -p sf-bbs -- console ./var/my-board/spitfire.toml
cargo run -p sf-bbs -- init-sysop ./var/fixture-board/spitfire.toml
cargo run -p sf-bbs -- run ./var/fixture-board/spitfire.toml
cargo run -p sf-bbs -- shell ./var/fixture-board/spitfire.toml
cargo run -p sf-bbs -- demo ./var/fixture-board/spitfire.toml
```

`run` starts the configured service listeners. Loopback acceptance traversed
the same Welcome → Main → Message → File → Help → Goodbye path over Telnet,
raw TCP, and RLogin; `shell` traversed it over Unix stdio. The Telnet acceptance
also registered a caller and then authenticated that persisted caller, while
existing-caller flow passed raw TCP, ordinary RLogin, and stdio. Optional
SyncTERM-compatible RLogin auto-login passed an end-to-end loopback test and
remains disabled by default. Direct serial is PTY-tested, and the Hayes inbound
state machine plus common authenticated-session boundary are simulation-tested;
physical serial/modem hardware is unverified. SSH is deliberately deferred.

## Accelerated Width-First Implementation Sequence

The one-to-two focused-week horizon is aspirational. Each increment must leave
the board runnable and add a vertical part of the final caller journey.

| Increment | Focus | Runnable exit condition |
|---|---|---|
| 0 — Contract and skeleton | Freeze the first checklist subset; add core board/session/config types, SQLite migrations, logical paths, process lifecycle, and integration-test harness. | A board initializes from a small checked-in fixture and starts/stops without corrupting state. |
| 1 — Connect and traverse | COMPLETE: common terminal metadata/session boundary; Telnet, raw TCP, RLogin, Unix stdio, direct serial, simulated inbound Hayes; CP437/ANSI resources; `.MNU`/HLP adapters; Main/Message/File traversal. SSH is a fail-closed follow-up. | Loopback Telnet, raw TCP, and RLogin plus Unix stdio traverse the same recognizable session and log off; serial/modem tests remain hardware-independent. |
| 2 — Become and remain a caller | COMPLETE: new/existing caller flow, Argon2id PHC credentials, stable IDs, numerical security, time/daily-call checks, caller statistics, accounting, reconnect, explicit Sysop initialization, and optional SyncTERM RLogin auto-login. Persistent terminal preferences and full questionnaires remain separately tracked parity work. | Telnet registration/reconnect and existing login over raw TCP, RLogin, and stdio reach the same caller and permitted menu; serial/modem authentication boundary is synthetic-tested. |
| 3 — Use messages | COMPLETE: SQLite message backend, conferences, read/post/reply/private/Sysop messages, line editor essentials, and last-read state. | Two callers exchange messages across two conferences and retain independent read state. |
| 4 — Set up and operate multiple nodes | COMPLETE: first-run setup, shared validated configuration services, interactive implemented-settings administration, safe conference changes, named listeners, race-safe configurable node pool, busy/release behavior, and read-only status foundation. | A setup-created four-node board accepts simultaneous callers across Telnet/raw/RLogin, rejects a fifth while full, and reuses the released node. |
| 5 — Use files | COMPLETE: schema-4 file areas/catalog, setup/config administration, protected listing/search/new view, confined hash-verified storage, stock ASCII text download/upload, staging, statistics, and multinode transfer state. Binary protocol breadth remains B-024. | Telnet completes browse/search/download/upload and two raw-TCP nodes download concurrently; reconnect state persists. |
| 6 — Close caller/Sysop interaction gaps | COMPLETE: page availability/request/answer/chat, persistent terminal preferences, shared MORE/abort, essential live caller/node administration, and a clean-board integrated acceptance. | A-062 passes; all 46 Category-A rows are implemented/verified or retain a precise PARTIAL obligation. |
| Resource/menu fidelity closure | COMPLETE: current-flow display precedence, confirmed macros, immutable menu identifiers, implemented-command help mapping, immediate hot keys, and bounded malformed/missing fallbacks. | 25 of 46 Category-A rows are VERIFIED; 12 precise PARTIAL obligations remain, so formal stock-core parity is not yet declared. |
| Final Category-A verification | COMPLETE: focused fidelity closures, public clean-board acceptance, row-by-row verification, and the A-027 historical MORE review. | All 46 Category-A rows are VERIFIED; Stock SPITFIRE 3.7 Core Parity is achieved. |

### What plausibly fits in one to two focused weeks

With disciplined scope, existing architecture decisions, SQLite as the initial
native store, and proven crates where appropriate, the target can plausibly
produce a genuinely usable **first stock-core board**: one native server,
Telnet, text/ANSI terminal output, new/existing caller flow, security levels,
resource-driven menus, multi-conference messages with last-read state, file
area browsing, normal logoff, and persistent restart behavior.

The complete A checklist did extend beyond that initial horizon: resource
fidelity, display controls, message interaction, interoperable transfers,
limits, native recovery, file presentation, and operator interaction each
required focused closure and evidence. Those closures and the final A-027
review now leave all 46 rows VERIFIED. B capabilities explicitly follow and do
not retroactively broaden the Stock Core Parity declaration.

## Implementation Boundaries

- Prefer a small number of cohesive Rust crates until a real dependency
  boundary exists; do not instantiate the entire aspirational crate diagram.
- Use one session engine for local tests and Telnet.
- Put caller/message/file persistence behind narrow domain interfaces, with
  SQLite as the first native backend and historical adapters added at their
  confirmed boundaries.
- Treat original `.MNU`, `.BBS`, `.CLR`, and `SPITFIRE.HLP` as compatibility
  inputs, not as the modern database schema.
- Parse all network and legacy input with explicit bounds and limits.
- Keep board behavior testable without opening a real network port; add a small
  number of end-to-end Telnet tests for transport integration.
- Do not build speculative plugin, network, web, GUI, or multi-version
  frameworks during stock-core parity.

## Updating This Checklist

Change a row to **PARTIAL**, **IMPLEMENTED**, or **VERIFIED** only with links to
the relevant source, tests, or acceptance report. Add newly discovered manual
capabilities rather than silently folding them into vague rows. If later
evidence corrects the classification, record why in
the relevant architecture document or pull-request record as appropriate.

This document controls stock 3.7 parity scope. The
[compatibility matrix](05-compatibility-matrix.md) controls the form of legacy
compatibility, and [ROADMAP.md](../ROADMAP.md) controls broader project order.
