# Classic SPITFIRE-Inspired Presentation Profile

## Status and authorization boundary

`classic-spitfire` 1.2.0 is an installed, independently redistributable
profile built from project-authored resources. Its design uses corroborated
historical visual grammar without copying original resources. Normal setup
installs Classic beside Modern and Minimal but keeps Modern selected by
default. Classic uses profile format/resource API 1, Modern as its base, and
`MIT OR Apache-2.0` project licensing and provenance metadata.

The post-0.1.0 source package adds independently authored Specific Caller
Messages and Text Search menu/help surfaces. The engine remains authoritative
for discovery and authorization; no original resource bytes or format/API
change is involved.

The target is:

```text
modern SPITFIRE NG engine
        +
historically evidenced SPITFIRE caller experience
        +
newly authored or explicitly redistributable presentation assets
        =
Classic SPITFIRE-inspired profile
```

Classic means a recognizable SPITFIRE personality, not a claim that SPITFIRE
NG is the original executable, an official Buffalo Creek release, or
SPITFIRE 3.7 itself. It also does not authorize Category-B behavior, RIP,
historical asset redistribution, SFDATE, SFREG, or website publication.

## Evidence authority and method

The evidence order is:

1. original SPITFIRE documentation and preserved resources;
2. observed original SPITFIRE 3.7 caller behavior;
3. accepted SPITFIRE NG architecture, decisions, and implemented behavior;
4. Synchronet only as a mature modern-BBS engineering comparison.

The profile review used a protected, read-only corpus without copying it into
the public repository. The principal evidence identities were:

`SPITFIRE.DOC` and `DISPLAY.ZIP` are in the `sf37-2` distribution directory.
The canonical loose `.MNU` and HLP copies are preserved in the companion
`sf37-1` research directory; `SFHELP.ZIP` in `sf37-2` independently inventories
the same 20,130-byte `SPITFIRE.HLP` format. This path distinction is recorded
rather than pretending every artifact was loose in one sample directory.

| Evidence | SHA-256 | Use in the review |
|---|---|---|
| `SPITFIRE.DOC` | `4a44f875ab6df1c7992aa7d1c85a075dae6eaffa088a09726adc4144dbcd7460` | Primary operating and resource specification |
| `DISPLAY.ZIP` | `79bbc54ebb924887f775c93a4f3b577ce478fadcc3303a9be4d0bd60f78ca612` | Exact 27-entry presentation archive and sample layout evidence |
| `SPITFIRE.HLP` | `758db950251a4c66f3d84ca6c8a6eedf586612478f0c8eec007d554aa99fb4bd` | Fixed-record help wording and identifier coverage |
| `SFMAIN.MNU` | `3e41e8571277e7c5a09e65c1ce1d23bb78e3f12f284ee63ff163748f45bbe786` | Main command/security authority evidence |
| `SFMSG.MNU` | `84339bfa9844a4cc73bc107cb726f3b264ee7e9e46db251b0de6c77d44c9eeb1` | Message command/security authority evidence |
| `SFFILE.MNU` | `93d6e311ec58c4dc2df76a9d025b1c6472ea5928d715c166bcb73c1a4538ec1e` | File command/security authority evidence |
| `SFSYSOP.MNU` | `66987d7ba2fb2c01410e654a8ea0ca38463d88db8e262f6c9263ff649a8cfd4e` | Sysop command/security authority evidence |

Private, redacted original-caller screenshots corroborate the ANSI choice,
name/password exchange, immediate message-queue summary, caller statistics,
new-file question, and compact two-column Main menu. Annotations describe the
observer's interpretation and are not treated as historical proof. The images
are evidence, not distributable profile assets.

Evidence labels in this specification mean:

- **Confirmed:** stated by the manual or directly present in preserved bytes.
- **Observed:** visible in a recorded original runtime session.
- **Inferred:** a bounded conclusion supported by more than one source but not
  directly specified.
- **Proposed:** a future SPITFIRE NG treatment requiring implementation and
  acceptance.

The existing Synchronet comparison supports keeping terminal capabilities,
paging state, and presentation resources separate. Its keys, menus, artwork,
prompts, and GPL/LGPL source do not define or supply Classic behavior.

## Non-negotiable architecture boundary

Classic uses the existing format-1 profile interface and Decisions D-028 and
D-029:

- the profile contains declarative data only;
- the board `.MNU` files and engine action registry own command letters,
  immutable identifiers, security, visibility, dispatch, and return context;
- the profile may own wording, arrangement, color, CP437 artwork, help text,
  and compatible BBS/CLR representations;
- terminal negotiation and caller preferences choose representation inside the
  selected profile; they do not choose a different board identity;
- paging, input, authentication, messages, files, transfers, node/session
  state, privacy, persistence, and backup remain engine-owned;
- non-menu resources retain board override -> Classic active -> Modern base ->
  built-in fallback, with CLR then BBS inside each layer for ANSI callers;
- exact-security menu art is a leaf override using board -> Classic active ->
  engine-generated `.MNU` presentation, never inherited from the base; and
- Modern remains the active/base setup default. Classic selection is an
  explicit future Sysop action.

## Implementation-ready caller-flow map

| Journey step | Historical evidence and resources | Current SPITFIRE NG equivalent | Owner | Proposed Classic treatment | Rights/provenance status |
|---|---|---|---|---|---|
| 1. Startup / connection | Observed Sysop, node, product/version, copyright, license/serial lines; manual identifies `SFPRELOG.BBS` as first display | Transport enters the common session; `SFPRELOG` and safe product identity are available | Engine transition; presentation wording | BBS-only prelogin header naming `SPITFIRE NG`, `@BOARD@`, `@SYSOP@`, and `@NODE@`; no historical serial/license claim | Newly authored historical-inspired text; allowed after project license record |
| 2. ANSI/color behavior | Manual asks `Do You Want ANSI Graphics? <y/N>` when ANSI is available and defines RIP -> CLR -> BBS fallback; runtime screenshot observes the ANSI answer | Negotiated capability plus saved caller graphics preference selects CLR/BBS | Engine/session capability | Do not add a fake profile-owned ANSI question. Supply BBS and CLR forms; retain the modern capability/preference path and record the prompt difference | New assets allowed; historical prompt bytes remain evidence only |
| 3. Pre-login resources | Manual orders `SFPRELOG` then `WELCOME1`; archive supplies CP437 BBS/ANSI CLR/RIP `WELCOME1` | Existing prelogin -> welcome resource path | Engine order; presentation content | Recreate board/Sysop identity and period composition in new BBS/CLR Welcome art; omit obsolete modem advertisement and license text | Historical-inspired, newly composed; original bytes blocked |
| 4. New caller path | `NEWUSER.[BCR]` normally carries rules; manual documents new-caller data and optional protocol choice | Policy-driven registration, modern password hashing, `NEWUSER`, profile collection, protocol preference | Engine workflow; presentation resources around it | Classic rules/welcome framing around the unchanged validated registration sequence; no second-password/birthday-as-password behavior | Newly authored text/ANSI; engine modernization retained |
| 5. Existing login | Observed first/last name lookup, wait text, password; `WELCOME2..9`, `ALL`, node, security, and caller-number displays may follow auth | Normalized caller name and password, returning `WELCOME2`, deterministic resource fallback | Engine authentication/order; presentation content | Stock-like courteous search/login wording only where existing semantic keys permit; never expose credential or imitate insecure lookup behavior | Newly authored; original prompts and display bytes not copied |
| 6. Post-login mail checks | Manual toggle and observed runtime show automatic all/queued message scan plus waiting/to/from statistics | Optional engine-owned `stock` journey reports live waiting/received/sent values; interactive Read/Your Messages retain full operations | Engine | Classic may frame the live result but cannot enable, reorder, or synthesize it | Newly authored framing allowed; state remains engine-owned |
| 7. Caller statistics | Observed greeting, caller number, last call, first call today, calls, daily/session time, active callers, and security; Main `Y` is implemented | Main Statistics uses persisted caller/message/file state and safe fields | Engine values; presentation framing | Use Classic terminology and compact framed output only through existing safe values; do not expose private fields or unsupported historical counters | New framing allowed; historical screen is observational evidence only |
| 8. New-files indication/check | Manual toggle and observed `List new files since last check?`; stock may continue into download before Main | The optional `stock` journey queries the live persisted checkpoint and offers the existing authorized new-file listing before Main | Engine | Retain the engine prompt/workflow and faithful File `N`; no static count | New labels/art allowed; state remains engine-owned |
| 9. Main menu | Manual, `SFMAIN.MNU`, `MAIN10` BBS/CLR/RIP, and observed two-column menu | Verified Main actions and exact-security generated/static menu path | `.MNU`/engine authority; profile layout | 80-column two-column BBS/CLR art for only authorized implemented actions; exact security 10/50 variants; generated fallback otherwise | Newly authored historical-inspired art; original `MAIN10` blocked |
| 10. Message menu | Manual, `SFMSG.MNU`, `MSG10` forms, `SF1STM`, `SFMSG<n>`, and help records | Verified conference, scan, post/reply, queue, Your Messages, Files/Main/Sysop/Help paths | Engine actions; presentation art/help | Two-column exact-security BBS/CLR menus and section/conference displays; omit LAKOTA, text search, and other unavailable commands | New assets allowed; original bytes and LAKOTA surface blocked/later |
| 11. File menu | Manual, `SFFILE.MNU`, `FILE10` forms, `SF1STF`, `SFIL<n>`, `SFDOWN`, `SFUP` | Verified list/search/new/upload/download/protocol paths | Engine actions; presentation art/help | Two-column exact-security BBS/CLR menus; show only implemented actions; do not imply tagging, archive view, erase/shuffle, ratios, or DIZ | New assets allowed; original and Category-B surfaces blocked/later |
| 12. Sysop menu | Manual, `SFSYSOP.MNU`, and `SOP100` forms show historical maintenance breadth | Security-controlled Sysop section currently supports Main, Xpert, and Goodbye only | Engine authority; presentation art | Packaged `SOP50` BBS/CLR serves the NG default exact level 50 only; all other exact levels use matching board/active art or generated output | New asset allowed; `SOP100` archive versus `SOP999` manual remains unresolved; advanced commands later |
| 13. Help | `SPITFIRE.HLP` is 55 fixed 366-byte records keyed to immutable actions | Validated fixed-record profile help and built-in fallback | Engine lookup/action identity; profile wording | Newly author all implemented Classic help records in the existing fixed format; unavailable historical records remain blank or bounded by current engine policy | Historical-inspired text allowed; original HLP blocked absent license |
| 14. About / credits | Historical Main System Info reports version/registration/Sysop/system counts; NG has a separate accurate About path | Paged About with final acknowledgment and project/original attribution | Engine values/paging; presentation framing | Classic-styled About must say SPITFIRE NG, credit original SPITFIRE separately, and deny official-release status; never claim `SF37` serial identity | Project-authored; existing attribution policy controls content |
| 15. Caller profile/settings | Historical `Your Statistics` also edits caller settings; help record 30 describes password/address/phone/keystroke changes | Separate Statistics, Caller Profile, and Terminal Preferences actions with modern policy | Engine validation/storage; presentation labels | Preserve separate safe NG actions while using recognizable terminology; no US-only phone layout, password display, or birthdate-as-secret | New labels/help allowed; modern privacy policy mandatory |
| 16. Goodbye/logoff | Manual and archive `GOODBYE` forms show a full-screen closing display; sample content advertises 1995 boards/phone numbers | Common Goodbye resource, accounting, disconnect, and node release | Engine close; presentation art | Newly composed closing art thanking the caller and naming the board; no copied phone list or obsolete advertising | Historical-inspired new art allowed; original Goodbye blocked |

### Login fidelity decision for the first Classic package

The recognizable sequence is connection identity -> capability treatment ->
welcome -> authentication/registration -> post-login state -> Main. Classic
resources reproduce the identity, welcome, transition, terminology, and
visual treatment. They do not initiate message scans, caller-statistics
queries, or new-file queries.

The approved implementation resolved the historical engine gap without changing
profile format/API 1. `caller.post_login_journey = "stock"` selects one fixed,
board-level session sequence. The engine queries live received/waiting/sent
message statistics, live caller/message/file totals, and the authorized
new-file checkpoint/count, then offers the existing new-file workflow before
Main. `none` remains the backward-compatible default. Profiles cannot select,
reorder, or extend the sequence, so no arbitrary command hook or profile-owned
state machine exists. Decision D-030 records this durable boundary.

## Menu contract

### Layout

Classic menu artwork targets 80 columns and a normal 24/25-line screen. The
historical archive supplies fixed 77-column stored BBS lines inside an
80-column composition and compact two-column command rows. M034 finds no
evidence for a universal adaptive-column algorithm.

The authored menus must:

- use a compact header and CP437 border vocabulary in BBS form;
- use restrained ANSI color and cursor positioning in CLR form;
- normally arrange commands in two columns when the authorized set fits;
- keep the caller key visible and faithful to the active `.MNU` record;
- fit ordinary 80x24/80x25 terminals without accidental paging;
- tolerate scrolling on a constrained terminal without becoming an input
  trap; and
- fall back to the engine-generated authorized menu whenever the declared
  artwork command set differs from the caller's actual authority.

### Exact command surface

The initial artwork follows the current setup-created `.MNU` authority, not
the larger historical menus:

| Menu | Ordinary implemented keys | Additional key at/above configured threshold | Classic static resources for NG defaults |
|---|---|---|---|
| Main | `M C F P Y R U A X G ?` | `@` | `MAIN10.BBS/.CLR`, `MAIN50.BBS/.CLR` |
| Messages | `C R B E Y A F Q X G ?` | `@` | `MSG10.BBS/.CLR`, `MSG50.BBS/.CLR` |
| Files | `C L D U N T F M Q X G ?` | `@` | `FILE10.BBS/.CLR`, `FILE50.BBS/.CLR` |
| Sysop | none | `Q X G` | `SOP50.BBS/.CLR` |

Historical Door, Bulletin, caller-directory, Who's On/node chat, questionnaire,
newsletter, other-board, LAKOTA, message-search, archive, file-maintenance,
event, packing, log, and other maintenance entries remain evidence or later
scope. Classic art must not display them as live choices.

Descriptor format 1 has no machine-readable list of action identifiers inside
static artwork. M035 therefore may target only the setup-created default menu
authority and must compare every visible key/label against the parsed `.MNU`
sets in automated acceptance. A Sysop who customizes `.MNU` must also supply
matching board/active-profile exact artwork or select the generated menu.
M037 makes that generated path explicit, but does not pretend to parse labels
or identifiers out of static ANSI art.

Security values are operator-defined from 0 through 9999. These are separate:
the caller's assigned level, the configured Sysop threshold, each `.MNU`
record's minimum, and the exact display suffix. Generated menus work at every
valid level. The packaged `10`/`50` art reflects NG setup defaults only. The
manual's `SOP999` distribution statement and the archive's actual `SOP100`
members remain an unresolved evidence discrepancy.

## ANSI, CP437, and paging requirements

- `.BBS` means the non-ANSI byte-preserving presentation class. It may use IBM
  CP437 box/line characters and must not be silently converted to Unicode.
- `.CLR` may use ANSI color and cursor positioning plus CP437 glyph bytes. Its
  design target is SyncTERM/Qodem at 80x25, with a complete BBS fallback.
- No `.RIP` resource is included in Classic format 1. RIP capability,
  rendering, control files, and rights require a separate interface and
  milestone.
- Decorative Welcome, menu, and Goodbye artwork owns its documented display
  controls. Deliberate full-screen or longer art may scroll and may suppress
  automatic MORE for its output unit. The engine must not globally impose MORE
  merely because cursor-addressed art has many stored line endings.
- About, Help, caller statistics, messages, file listings, and other
  interactive information retain the established session-local page length,
  S=Stop/N=Nonstop/Enter contract, bounded modern aliases, and required return
  acknowledgments.
- Binary transfer remains wholly outside the presentation/paging path.

At 48x10, Classic static 80-column art is allowed to scroll or wrap according
to the terminal. Acceptance is about safe progress, fallback, and input
isolation; it is not evidence that stock SPITFIRE dynamically reflowed art.
Minimal remains the purpose-built constrained-terminal profile.

## Historical DISPLAY.ZIP inventory and disposition

Every archive member below remains an unmodified research input. “Future name”
is a semantic mapping, not permission to copy the member.

| Historical member | Evidence role | M034 disposition | Future Classic name/treatment |
|---|---|---|---|
| `FILE10.BBS` | Non-ANSI exact-security File menu | Required core evidence | Newly author `FILE10.BBS` |
| `FILE10.CLR` | ANSI exact-security File menu | Required core evidence | Newly author `FILE10.CLR` |
| `FILE10.RIP` | RIP File menu | Unresolved/later RIP | No version-1 asset |
| `FILECTRL.RIP` | RIP File control panel | Unresolved/later RIP | No version-1 asset |
| `GOODBYE.BBS` | Non-ANSI closing art | Required core evidence; old phone list obsolete | Newly author `GOODBYE.BBS` |
| `GOODBYE.CLR` | ANSI closing art | Required core evidence; old phone list obsolete | Newly author `GOODBYE.CLR` |
| `GOODBYE.RIP` | RIP closing art | Unresolved/later RIP | No version-1 asset |
| `MAIN10.BBS` | Non-ANSI exact-security Main menu | Required core evidence | Newly author `MAIN10.BBS`; also compose current-authority `MAIN50.BBS` |
| `MAIN10.CLR` | ANSI exact-security Main menu | Required core evidence | Newly author `MAIN10.CLR`; also compose current-authority `MAIN50.CLR` |
| `MAIN10.RIP` | RIP Main menu | Unresolved/later RIP | No version-1 asset |
| `MAINCTRL.RIP` | RIP Main control panel | Unresolved/later RIP | No version-1 asset |
| `MSG10.BBS` | Non-ANSI exact-security Message menu | Required core evidence | Newly author `MSG10.BBS`; also compose current-authority `MSG50.BBS` |
| `MSG10.CLR` | ANSI exact-security Message menu | Required core evidence | Newly author `MSG10.CLR`; also compose current-authority `MSG50.CLR` |
| `MSG10.RIP` | RIP Message menu | Unresolved/later RIP | No version-1 asset |
| `MSGCTRL.RIP` | RIP Message control panel | Unresolved/later RIP | No version-1 asset |
| `SFBATCHD.BBS` | Batch-download menu | Category-B/later | Reserve semantic identity; no Classic 1.0 asset |
| `SFBATCHD.CLR` | ANSI batch-download menu | Category-B/later | Reserve semantic identity; no Classic 1.0 asset |
| `SFBATCHD.RIP` | RIP batch-download menu | Category-B plus RIP unresolved | No version-1 asset |
| `SFBATCHU.BBS` | Batch-upload menu | Category-B/later | Reserve semantic identity; no Classic 1.0 asset |
| `SFBATCHU.CLR` | ANSI batch-upload menu | Category-B/later | Reserve semantic identity; no Classic 1.0 asset |
| `SFBATCHU.RIP` | RIP batch-upload menu | Category-B plus RIP unresolved | No version-1 asset |
| `SOP100.BBS` | Historical security-100 Sysop menu | Core layout evidence; commands mostly later | Newly compose current-authority `SOP50.BBS`; do not rename/copy bytes |
| `SOP100.CLR` | ANSI security-100 Sysop menu | Core layout evidence; commands mostly later | Newly compose current-authority `SOP50.CLR` |
| `SOP100.RIP` | RIP security-100 Sysop menu | Unresolved/later RIP | No version-1 asset |
| `WELCOME1.BBS` | CP437 welcome template | Required core evidence; version/modem/legal boilerplate obsolete | Newly author board-neutral `WELCOME1.BBS` |
| `WELCOME1.CLR` | ANSI/CP437 welcome template | Required core evidence; content obsolete | Newly author `WELCOME1.CLR` |
| `WELCOME1.RIP` | RIP welcome | Unresolved/later RIP | No version-1 asset |

The archive's `SOP100` and manual's stated distribution example `SOP999` are
not normalized into one supposed historical fact. The future `SOP50` name is
derived solely from the current setup board's exact security authority.

### Four historical menu authorities and help

| Historical authority | Classic package disposition | Reason |
|---|---|---|
| `SFMAIN.MNU` | Never packaged as profile art; current board file remains authoritative | Profile cannot add historical Main commands or change security |
| `SFMSG.MNU` | Never packaged as profile art; current board file remains authoritative | LAKOTA/search/etc. cannot be implied by presentation |
| `SFFILE.MNU` | Never packaged as profile art; current board file remains authoritative | Archive/erase/shuffle/tagging breadth remains outside Classic |
| `SFSYSOP.MNU` | Never packaged as profile art; current board file remains authoritative | Advanced maintenance cannot be simulated |
| `SPITFIRE.HLP` | Newly author a fixed-record Classic help file for current identifiers | Exact historical wording/bytes are rights-unresolved and describe unavailable behavior |

## Accepted package

| Field | Accepted value |
|---|---|
| Stable profile ID | `classic-spitfire` |
| Display name | `Classic SPITFIRE-Inspired` |
| Initial version | `1.0.0` |
| Descriptor | Strict format 1, resource API 1, exact hashes/inventory, engine range `>=0.1.0,<0.2.0` |
| Supported formats | `bbs`, `clr`, `spitfire-help` |
| Active/base configuration | Explicitly selected Classic active with `modern-ng` base |
| Default behavior | Modern remains active/base for existing and new boards |
| Fallback | Board override -> Classic CLR/BBS -> Modern compatible CLR/BBS -> built-in |
| Installation | Normal setup installs Classic beside Modern/Minimal without selecting it automatically |

The accepted core inventory is:

- 32 newly authored BBS resources: 25 current general display keys plus the
  seven exact-security menu-art keys;
- 31 newly authored CLR counterparts. `SFPRELOG` remains BBS-only to preserve
  its evidenced pre-graphics role; any later CLR form requires an explicit
  rationale;
- one newly authored 55-record fixed-size `SPITFIRE.HLP`, with content only for
  current action identifiers;
- `README.md` describing the target and non-official identity;
- `LICENSES/` evidence and project asset-license records; and
- strict provenance entries and hashes for every rendered asset.

The 25 general display keys are `SFPRELOG`, `WELCOME1`, `WELCOME2`, `NEWUSER`,
`SFONFAIL`, `GOODBYE`, `SFPGOFF`, `SFUNANS`, `SFPAGED`, `USERINIT`, `CHATDONE`,
`SF1STM`, `SF1STF`, `SFMSG1`, `SFMSG2`, `SFIL1`, `SFIL2`, `SFDOWN`, `SFUP`,
`ABOUT`, `PRIVATE`, `LOCKOUT`, `TOOMANY`, `SFTIMEUP`, and `SFASLEEP`.

The seven menu-art keys are `MAIN10`, `MAIN50`, `MSG10`, `MSG50`, `FILE10`,
`FILE50`, and `SOP50`. Conference/file-number examples are the setup package's
current bounded starter keys, not a new rule limiting board overrides.

## Rights and provenance gate

### Distributable Classic 1.0 categories

| Asset group | Intended provenance kind | Creator/rightsholder | Evidence | License | Redistribution | Modifications record |
|---|---|---|---|---|---|---|
| New BBS/CLR composition | `historical-inspired` | Named SPITFIRE NG contributor(s); project-controlled rights confirmed at contribution | This specification, experience map, manual/resource citations | `MIT OR Apache-2.0` for current project-authored assets | `allowed` only after review | Record “new composition informed by cited behavior; no historical bytes copied,” author, and date |
| New help wording | `historical-inspired` | Named SPITFIRE NG contributor(s) | Current immutable action map plus historical HLP functional evidence | Project asset license | `allowed` after review | Record independent wording and mapped action IDs |
| Descriptor and generated manifest | `generated` | SPITFIRE NG project | Deterministic generator/source commit | Project asset license | `allowed` | Record generator version/commit and inputs |
| Package README/license notices | `project-authored` | Named contributor(s)/project | Repository history | Project documentation/asset license | `allowed` | Normal revision history |
| Separately licensed third-party art, if ever proposed | `third-party` | Exact creator/rightsholder required | Written grant or published license | Exact SPDX or reviewed `LicenseRef-*` | `allowed` only when the grant covers redistribution and modification | Record all changes |

Every production record must populate creator, rightsholder, source/evidence,
license, redistribution, and modifications. “Inspired by SPITFIRE” is not a
license and cannot replace those fields.

### Rights-blocked or unresolved inputs

| Input | Classification | Distribution result |
|---|---|---|
| Exact `DISPLAY.ZIP` members or close byte/artwork derivatives | `historical-original`; individual artist/rightsholder and redistribution grant unresolved | `unknown`; blocked from repository/distributed profile |
| Original `SPITFIRE.HLP` records | `historical-original`; Buffalo Creek-era wording, redistribution unresolved | `unknown`; blocked |
| Original `.MNU` files | `historical-original` and engine authority evidence | Not profile content; blocked from redistribution absent separate review |
| Caller screenshots | Observational evidence; capture/annotation rights and private context not a profile license | Evidence only; never packaged |
| RIP members/control panels | Historical originals plus unsupported runtime format | Rights-blocked and technically unresolved |
| Original Welcome/Goodbye version, license, phone, modem, and advertising content | Historical original and obsolete board-specific material | Do not reproduce in new assets |
| Synchronet artwork/text/source | Third-party GPL/LGPL comparative material, not SPITFIRE authority | Do not use as Classic asset source |

New art may reproduce the function, documented command terminology, compact
two-column arrangement, CP437/ANSI medium, and general period character while
using a fresh border/logo composition, spacing, prose, color palette, and
board-neutral content. Authors must not trace screenshots, transcribe full
historical screens, or mechanically recolor preserved bytes.

A Sysop-supplied local-only package made from lawfully possessed originals is
a separate future compatibility path. It would require `local-only`
provenance, remain outside this repository, and make board backups sensitive.

## Product identity requirements

All Classic resources must:

- identify the running product as `SPITFIRE NG`;
- label the presentation `Classic SPITFIRE-Inspired` where profile identity is
  useful;
- retain the separate credit to original SPITFIRE, Mike Woltz, and Buffalo
  Creek Software under the established About/product policy;
- state that SPITFIRE NG is not an official Buffalo Creek release or
  endorsement; and
- avoid original registration serials, “Version 3.7” as the running product,
  or “Licensed to” language associated with an original executable.

## Modernization boundary

| Historical behavior/constraint | Required Classic treatment |
|---|---|
| Two-digit/pivoted dates | Keep full modern dates and board-local time policy; style may be Classic, semantics may not regress |
| Birthdate as second password | Never reproduce; keep Argon2id credentials and full-year private birth dates |
| US phone formatting and fixed legacy address assumptions | Keep international, policy-driven address/phone/contact fields |
| Public caller names/addresses and broad caller lists | Keep modern privacy and backend authorization; art cannot expose private profiles |
| Modem/COM/UART/FOSSIL and fixed-baud assumptions | Omit from core identity; transports remain native and capability-neutral |
| DOS shell/drop, direct video memory, ANSI.SYS, drive letters | Treat as obsolete implementation constraints, not Classic requirements |
| Fragile line input and stale escape sequences | Retain bounded, drained, transport-neutral input behavior |
| Historical storage/backup mechanics | Retain SQLite authority, confined storage, exact cold backup/restore, and validation |
| Per-node DOS directories/processes | Retain the scalable session/node manager and session-local presentation state |
| Historical transfer UI | Retain safe binary ownership, staging, integrity, and success-only accounting |
| Historical registration/license display | Use accurate SPITFIRE NG identity and separate historical credit only |

Classic presentation may evoke the era; it may not reverse security, privacy,
data-integrity, portability, or reliability improvements.

## Localization compatibility

Descriptor format 1 does not include localization fields or language packs.
Unknown descriptor fields currently fail closed, so an invented `locale` field
would be incompatible.

The Classic plan preserves a future separation by:

- keeping profile ID and visual identity independent of language;
- keying menu labels/help to stable semantic action IDs rather than English
  text or visual position;
- keeping prompts engine-owned by stable semantic keys;
- putting translatable prose in discrete display/help resources rather than
  Rust conditionals or ANSI cursor logic;
- avoiding word lengths as authorization or dispatch inputs;
- recording English as package documentation, not as an engine assumption;
  and
- reserving locale selection/fallback for a separately versioned language
  interface that can apply across Modern, Minimal, and Classic.

The separate interface is defined in the
[Localization Contract](localization.md). Classic remains profile format/API 1
and version 1.1.0. Its BBS/CLR/HLP bytes are not rewritten by locale selection;
engine prompts, generated menus, paging, and live status use the selected
language package. The model is “Classic visual profile + selected language
resources,” not a separate hard-coded engine or an unbounded family of IDs.

## Acceptance matrix

Every result must identify profile ID/version, descriptor digest, SPITFIRE NG
commit, client/version, terminal geometry, representation selected, and exact
fallback path. Planned cells below are requirements, not current PASS claims.

### Clients and terminal sizes

| Client/path | 80x25 | 48x10 constrained | Wider modern terminal | Required result |
|---|---|---|---|---|
| SyncTERM Telnet ANSI/CP437 | Required | Required safe-progress check | Required smoke check | CLR selected where valid; BBS fallback works; no authority change |
| SyncTERM text mode | Required | Required safe-progress check | Optional | BBS selected; no raw ANSI/RIP bytes |
| Qodem ANSI | Required | Required safe-progress check | Required smoke check | CLR color/cursor behavior remains usable |
| Qodem text | Required | Required safe-progress check | Optional | BBS/CP437 or negotiated text handling remains usable |
| Plain RAW | Required text transcript | Required | Optional | BBS path, bounded input, no ANSI escape leakage |
| Automated RLogin/stdio/serial/Hayes | Shared-engine regression | Representative constrained coverage | Not required | No transport-specific Classic policy |

At 48x10, exact visual composition need not match 80x25. The caller must be
able to page/scroll, issue the next valid command, and exit without stale input
or disconnect. Minimal remains the visual baseline for deliberately narrow
use.

### Caller journeys

| Journey | Required evidence before Classic is Verified |
|---|---|
| Startup/prelogin | Accurate NG/Sysop/node identity; no original-version/serial claim; BBS-only prelogin behavior recorded |
| New caller | Registration completes under all active profile-field policies; Classic resources cannot change validation |
| Returning login/reconnect | CLR/BBS preference and caller state persist; failed auth remains bounded |
| Post-login summaries | Historical difference explicitly recorded; any automatic sequence requires separate engine authorization |
| Main | Security-10 and security-50 art matches exact authorized identifiers and keys |
| Messages | Read/post/reply/queue/Your Messages plus Main/File/Sysop/Help returns pass |
| Files | List/search/new/upload/download plus acknowledgments and Main/Message/Sysop returns pass |
| Sysop | Ordinary caller rejected; security-50 caller sees only Q/X/G and returns safely |
| Help | Every current action maps to bounded Classic help; missing/malformed help falls back in context |
| About | Paging at 80x25 and 48x10, final acknowledgment, accurate product attribution |
| Caller profile/settings/statistics | Modern privacy/policy retained; no private field or credential disclosure |
| Goodbye | Classic BBS/CLR closing art, clean accounting, disconnect, and node release |

### Fallback and regression

Acceptance must also prove:

- non-menu Classic CLR -> Classic BBS -> Modern base -> built-in resolution,
  plus exact-menu board -> Classic active -> generated resolution;
- missing, hash-mismatched, malformed, incompatible, and exact-security menu
  failures remain visible in operator status and usable to callers;
- acceptance fails if the visible static-art key/label matrix differs from the
  parsed setup-created `.MNU` authority; format 1's lack of automatic runtime
  artwork parsing remains explicit;
- Modern and Minimal descriptors, bytes, selection, caller journeys, and
  defaults remain unchanged;
- authentication, authorization, private messages, profile PII, statistics,
  messages, files, paging, hot keys, cancel paths, and node isolation match the
  presentation-neutral baseline;
- XMODEM/YMODEM/ZMODEM/Telink and ASCII transfers emit no presentation bytes
  in binary mode;
- cold backup/new-root restore preserves exact Classic descriptor, assets,
  licenses, provenance, configuration, and fallback status; and
- mixed Modern/Minimal/Classic failure tests do not leak state across nodes or
  sessions.


## Related canonical documents

- [Presentation Profiles](presentation-profiles.md)
- [Classic Fidelity and Provenance Review](research/m036-classic-fidelity-review.md)
- [Legacy Data and File Formats](06-legacy-file-formats.md)
- [Caller/Sysop Interaction and Terminal Fidelity](sfng-caller-sysop-interaction.md)
- [Stock SPITFIRE 3.7 Parity Checklist](stock-spitfire-3.7-parity.md)
