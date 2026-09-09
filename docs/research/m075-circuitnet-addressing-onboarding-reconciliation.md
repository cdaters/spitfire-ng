# M075 — CircuitNET addressing and operator onboarding reconciliation

C8.1 improves the Network Kit for an existing BBS Sysop joining a network. It does
not teach BBS installation, redesign accepted networking or begin C9. Starting
private `1561790693bba63ff35f042e3bddd964654c55f1`; public
`3755010d508d1f36b773ea4e363806495161aac0`. Schema remains **36**, wire **1.4**.
**COMPLETE / ACCEPTED.** Validation results are recorded below; publication
source provenance is recorded in Git and the generated kit release/checksum files.

## Evidence and decisions

Read the actual 70-file Network Kit build 3 ZIP, its source Markdown, renderer,
release metadata, signed revision 2 and authority, role/Charter/Rules/catalog guides,
and native Files/Events/Health documentation. Retained CNCONFS.TXT (1991-06-01)
explicitly groups SUPPORT, SYSOP, SPITFIRE and DOORS as Sysop/visiting-Sysop areas.
Retained CIRCUIT.DOC's installation prerequisites include assigned Node ID, HOST ID,
current available/required conferences, Charter/Rules and transfer/scheduling
instructions. These files were read with explicit CP437 decoding, never altered or
executed. No historical proprietary source is included in the public report or kit.
C1 remains historical authority; this is a rights-safe summary, not a reproduction.

The delivered conference list was alphabetic; application consent was free-form;
Files had no complete operator sequence; ordinary verification led with a developer
artifact utility; established topology wording could imply a migration procedure.
The new material preserves the historical practical installation spirit while using
current enrollment, catalog, Dossiers, native Files and Events commands.

### Catalog policy review

Signed revision 2 remains unchanged: SUPPORT and CHITCHAT are required; SYSOP,
SPITFIRE and DOORS are optional and Sysop-only. SUPPORT already supplies mandatory
network operational communication, so requiring SYSOP would duplicate that minimum
and impose another obligation on small boards. SYSOP remains useful broader operator
coordination; optional does not mean public. SPITFIRE-specific support and door-author
discussion remain optional because not every participating board uses those products.
No governance vote, signed revision, Charter/Rules rewrite or catalog propagation
change is warranted. The listing groups required areas first, optional operator
areas next, general callers last; SUPPORT's operator access is explicit in both the
required group and operator introduction. Immutable conference IDs remain hidden.

### Addressing decision and migration review

The [technical specification](../technical/circuitnet-addressing.md) inventories
Node ID use in profile/tree, certificate enrollment, messages, Files, directed
routing, controls, Dossiers, catalog signing authority, Events, database keys,
backup and audit. Existing retained profiles intentionally reject identity/tree
changes. The wire has no immutable node generation; a new local UUID alone cannot
preserve historical meaning when the old address is reused elsewhere.

Keep addressing an administrative convention and read-only assignment helper.
CNETROOT is reserved for ROOT in new assignments; CCRR000 primary HOST, 001–899 END,
900–999 additional HOST. Explicit role/topology still route traffic. Existing IDs
remain valid. The seed still names ROOT as publisher; no silent rename to CNETROOT
is possible or claimed. Safe established-tree migration, geographic relocation and
reuse need a separate reviewed migration. Retired IDs remain reserved meanwhile;
same-BBS recovery/reactivation requires retained state and reenrollment review.
This limit is visible in member and technical docs, not hidden in the report.

Version 1 [regions.json](../circuitnet-ng/config/regions.json) contains 13 initial
mappings across US/CA/AU/GB. Country authority is ISO 3166-1 alpha-2; RR is explicitly
a CircuitNET presentation code mapped to a full subdivision, not a purported ISO
standalone code. Only standard-authority web checks were used; no web archaeology.
Missing regions require a versioned administrative addition, not invented runtime
geography. The helper validates all reservations (including retired names), rejects
normalized collisions/range-role mismatch and suggests by country/region name. It
is not an atomic allocation service, registry sync or substitute for administrator
approval. One private membership record serializes actual assignments.

### Narrow local access operation

Review found no supported normal conference command for editing privileged visiting
Sysop levels, although native access rules already support them. Add
`sfconfig circuitnet BOARD catalog-access-levels NETWORK CODE LEVELS` (or `-` to
clear). Existing stopped-board and Read Configuration/Change Sensitive Configuration
checks gate the command. One transaction validates an active mapped Sysop catalog
area with intact restricted access, replaces at most five distinct nonzero native
levels and appends a bounded safe audit. Failure rolls back; no caller names,
message bodies, catalog signature, subscriptions or thresholds change. This is the
only runtime addition, with an updated en-US command list/package 1.34.0.

The procedure uses the existing local console SECURITY command to assign a reviewed
account level, with explicit board-wide consequences. It distinguishes OS operators,
local Sysop callers and verified visiting Sysops, requires independent verification,
keeps ordinary callers denied and documents revocation. Tests now exercise actual
grant/revoke authority instead of writing the grant SQL directly.

### Application and directory policy

Node ID, BBS name and role are the minimal public membership record, acknowledged
explicitly before approval. Country/region supports administrative assignment;
the geographic address itself reveals that assignment. Optional publication defaults
to unchecked: handle, general location, caller address, website, software/version,
separate public contact email and description. Review contact is never a fallback
public email. No secret fields; no static-IP or public caller endpoint requirement
for an outbound-polling END. Consent withdrawal suppresses later exports; downloaded
old copies cannot be recalled. The planned /nodes path derives from canonical release
identity metadata. No directory/form/site/mail service is implemented.

## Documentation and stale-reference reconciliation

Canonical member documents live in docs/circuitnet-ng; BBS editions are generated,
not hand-maintained. README leads ABOUT → ADDRESSING → JOINING → END guide.
DOSSIERS follows USAZ017/USAZ000 with actual catalog SUPPORT/CHITCHAT/RETRO,
local reverse Dossier, authenticated requests, approval and receipts. Events explains
900 seconds as 15 minutes, hybrid catch-up, scheduled/manual queues and bidirectional
outbound polls. EVENTS the conference is separate from scheduler Events. FILES gives
both boards' area/map/subscription/import/scan/approval/Poll steps and prominently
states no remote file subscriptions, no official file-area catalog, unsupported
inspection quarantine and restart-from-zero transfer.

Normal catalog import/sync automatically verifies authority, signature and chain;
status reports accepted revision, hash, authority, errors and pending work. The docs
name actual fields, not an invented Signature Valid display. Offline artifact
compilation remains advanced recovery/build tooling only. Ordinary To names do not
route; optional directed destination selects a BBS, not private mail or a global
person address. HOST/ROOT can also operate ordinary caller BBS services.

The stale scan classified all current matches:

| Match class | Disposition |
| --- | --- |
| Protocol 1.2 directed-controls introduction | Labeled introduction under current 1.4. |
| 1.3 file phase and 1.4 catalog phase | Historical extension headings retained; current opening remains minor 0–4 support. |
| TLS 1.3 | TLS version, not CircuitNET version; retained. |
| Schema 29/30/31 historical migration sections | Retained as explicit migration history. Catalog opening now states current board schema 36. |
| Conference Health future in foundation spec | Corrected to implemented native Health and linked current specification. |
| Files/governance outside implementation in current manual | Corrected for Files/signed catalog; old C2 scope remains labeled history. |
| CNTEST/CNTECH in general manual | Confined to explicitly isolated development examples; absent from member kit. RETRO used for ordinary setup. |
| Draft JSON in catalog admin/key custody | Actual unpublished working drafts, not adopted catalog status; retained. |
| Old conferences.proposed.json | Historical proposal source, excluded from kit, not machine authority. |
| Public services, future migration/registry/standards | Accurate unimplemented boundaries, retained. |
| Kit build 3 in distribution build guide | Updated to revised build 4; old research checkpoints remain historical. |
| Certificate .invalid name and endpoint placeholders | Deliberately replaceable enrollment values, not official web identity. |

## Acceptance and publication

Private full workspace: **853 passed / 0 failed / 7 existing ignored**, with all
eight doctest groups successful. All seven live CircuitNET campaigns pass, including
the revised three-board access/key-recovery journey, six-node catalog lifecycle,
Files, Events, directed routing and replay/reconnect. Offline CircuitNET, FTN,
QWK, native Files and Conference Health regressions pass. The new Node ID test
checks geographic/legacy names and explicit-tree parent choice without claiming
that established persisted topology can migrate.

Twelve Python tests pass: five addressing helper cases and seven kit cases. Actual
ZIP builds are deterministic, signatures/history and exact manifest inventory verify,
root BBS text is ASCII/CRLF/79 columns, links/config paths resolve, consent and
conference groupings are checked, and a domain-change fixture regenerates contacts
without drift. A fresh extracted kit was read as an outsider and its packaged
assignment helper exercised on Apple Silicon using an empty disposable snapshot.
It proposes USAZ001 with assigned=false; no membership record or live node is created.

Private headers **181**, fmt, strict workspace/all-target Clippy and diff checks
pass. Markdown/local links pass (247 files, 1,683 links before the final log entry).
Cargo-audit is unavailable, not passed. Catalog/history/authority and Charter/Rules
remain byte-identical to the accepted checkpoint. The signed revision-2 body hash
is c1056331f097761f1ca574092be66995d0dd014534877f7bce79a4f9fc6cc9d0.

Network Kit 1.0 revised build 4 contains **79 files**, nine more than build 3:
three new member guides with text editions, region metadata, technical specification
and read-only helper. Final build path is dist/c81-network-kit-1.0-final/ with the
ZIP checksum sidecar; it is generated after the public source commit is known.
No generated archive is committed or offered as a second simultaneous current 1.0.
Full private evidence stays outside tracked source. Public synchronization uses an
explicit reviewed allowlist; historical inputs and acceptance data are excluded.

Exact next action: publish the validated C8.1 checkpoint and sanitized public source,
record final kit provenance, then stop for review. Do not
begin C9. No DNS, website, mail or application opening is authorized.

## Requested completion report

| # | Item | Result |
| --- | --- | --- |
| 1 | Starting private | `1561790693bba63ff35f042e3bddd964654c55f1` |
| 2 | Starting public | `3755010d508d1f36b773ea4e363806495161aac0` |
| 3 | Schema | 36 → 36; no migration. |
| 4 | Protocol | 1.4 → 1.4; no wire change. |
| 5 | Kit review | Actual build-3 ZIP and sources reviewed; findings and corrections above. |
| 6 | Audience | Existing BBS Sysop, possibly new to networking; no BBS installation tutorial. |
| 7 | Conference structure | Required, optional operator, general caller groups generated from signed entries. |
| 8 | Required | SUPPORT and CHITCHAT on every participating board, with distinct access. |
| 9 | Sysop-only | SUPPORT, SYSOP, SPITFIRE, DOORS; no automatic account grant. |
| 10 | SYSOP | Optional retained after explicit rationale review. |
| 11 | SPITFIRE | Optional specialized support; access remains Sysop-only. |
| 12 | DOORS | Optional specialized author/operator discussion; access remains Sysop-only. |
| 13 | Access setup | SYSOP-ACCESS documents read/post 9999, local grants and denial checks. |
| 14 | Visiting Sysop | Independently verified by local operator; dedicated level, local grant and revocation. |
| 15 | Application | Checkbox choices replace free-form publication consent. |
| 16 | Directory privacy | Minimal acknowledged ID/BBS/role, separately consented optional fields; private review contact excluded. |
| 17 | Dossier | Complete USAZ017/USAZ000, map/local-subscribe/request/approve/query/test journey. |
| 18 | Events | 15-minute example, Hybrid/Scheduled/Manual/Immediate, bidirectional outbound Poll, EVENTS distinction. |
| 19 | Files | Both boards' map/subscription/import/safety/review/exchange; limits prominent. |
| 20 | Catalog verification | Automatic import/sync checks; actual status fields; developer utility advanced only. |
| 21 | Stale docs | Classified and reconciled above; historical scopes remain labeled. |
| 22 | Address architecture | Assignment policy and read-only helper; native routing/identity untouched. |
| 23 | CNETROOT | Reserved new ROOT name; current signed ROOT publisher retained, transition not fabricated. |
| 24 | Country authority | ISO 3166-1 alpha-2, named selection from approved initial metadata. |
| 25 | Region authority | Versioned network mapping to named formal subdivisions, 13 bootstrap rows. |
| 26 | CCRRNNN | Seven alphanumeric characters; existing 1–8 parser unchanged. |
| 27 | Primary HOST | 000, helper requires explicit HOST role. |
| 28 | Ordinary END | 001–899. |
| 29 | Additional HOST | 900–999. |
| 30 | Range validation | Helper rejects disagreement; wire parser does not infer role. |
| 31 | Topology | Explicit tree separate from ID; established-tree migration unavailable. |
| 32 | Relocation | Future new geographic assignment preserving old provenance; no unsafe in-place move implemented. |
| 33 | Immutable node identity | No generation authority today; deferred after full dependency review, not papered over with a UUID. |
| 34 | History | Existing historical addresses unchanged; administrative retirement record required. |
| 35 | Reactivation/reuse | Same-BBS retained-state recovery reviewed; different-BBS reuse blocked pending safe generation migration. |
| 36 | TLS | Existing certificate-to-Node-ID binding unchanged; future address change requires coordinated reenrollment. |
| 37 | Assignment UX | Country/region names plus role/request; helper suggests, Administrator alone confirms/reserves. |
| 38 | Directory | Search model and consent fields documented; /nodes planned, no service or sync implemented. |
| 39 | Normal post | RETRO example uses shared codename and Dossiers; human To does not route. |
| 40 | Directed post | Explicit destination selects a BBS; still conference traffic, not private/E2EE/person addressing. |
| 41 | Address docs | ADDRESSING.md/TXT and technical specification included. |
| 42 | Navigation | README → ABOUT/ADDRESSING/JOINING/END, role guides and technical directory. |
| 43 | Tests added | One Rust Node ID/topology test; existing native access and real operator journey extended; six new Python tests. |
| 44 | Workspace totals | 853 passed / 0 failed / 7 existing ignored; eight doctest groups. |
| 45 | macOS | Apple Silicon; native helper/ZIP review and existing isolated live campaigns with new access commands. |
| 46 | Regressions | Full workspace and final targeted/public scope recorded below. |
| 47 | Kit/hash | Network Kit 1.0 revised build 4; final path/checksum in publication closure. |
| 48 | Localization | en-US 1.34.0, existing command-list string updated; no new display vocabulary required. |
| 49 | Headers/fmt/Clippy/diff | All pass; 181 private source headers. |
| 50 | Links | Repository and freshly extracted kit checked, including addressing/config links. |
| 51 | Privacy/provenance | No original historical documents, membership records, keys, private contacts or acceptance artifacts packaged. |
| 52 | cargo-audit | Executable unavailable; not claimed passed. |
| 53 | Private source | Accepted implementation `801ddc00103d373ee7f6cc8d18da680ecc82cbf7`; private documentation closure recorded separately. |
| 54 | Final public | This sanitized release commit, identified by Git HEAD. |
| 55 | Public delta | 42 paths: 8 added, 34 updated; 38 reviewed copies plus four public navigation/status files. |
| 56 | C9 | NONE. |
| 57 | DNS/web/mail deployment | NONE; applications remain Closed. |
| 58 | Production changes | NONE. |
| 59 | External live traffic | NONE; only isolated loopback BBS acceptance, plus read-only standards/publication tooling. |
| 60 | Exact next action | Stop for C8.1 review. Do not begin C9 or deploy public services without separate authorization. |

Confirmed: onboarding targets Sysops new to networking, not people new to BBS
operation. CircuitNET retains simple 1–8 character Node IDs; international assignment
is a convention, not an FTN hierarchy. Explicit role/topology remain separate from
the human address. Normal conference posting does not need a destination Node ID.
C1–C8 remain accepted. No C9 or production/public-service changes occur in this pass.


## Sanitized public validation

The 38 allowlisted implementation/document/test paths matched accepted private
source `801ddc00103d373ee7f6cc8d18da680ecc82cbf7` before this public validation note.
Four public navigation/status files were updated without copying private project
state/history. No historical corpus, keys, personal membership/readership data or
private acceptance artifacts are included.

Public core/protocol tests: 504 passed, zero failed, one existing ignored, with
two doctest groups. The real macOS catalog/access/key-recovery campaign passes with
the new local grant/revoke commands. Twelve Python tests, 158 source headers,
formatting, local links (175 Markdown files / 1,215 links), signatures and manifest
checks pass. This supplements the complete private workspace run; it is not claimed
as a second full public workspace run. Strict public workspace/all-target Clippy passes.

The sanitized release commit identifies public source. Generate the final kit with
that commit supplied as --source-commit; RELEASE.TXT, MANIFEST.sha256 and the ZIP's
adjacent checksum record its provenance. Generated archives remain outside Git.
Exact next action after release generation: stop for C8.1 review. No C9 or public
service deployment; applications remain Closed.
