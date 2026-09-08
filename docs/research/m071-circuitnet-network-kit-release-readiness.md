# M071 — Network Kit release readiness

C7.1 modern release-readiness report. C1 remains historical authority and C2-C7
remain accepted. This public summary contains no historical source artifacts,
private keys, credentials, private acceptance outputs or production configuration.
Schema 35, CIRCUITNET-NG 1.4 with optional catalog-access; stop before C8.

## Review gate

The actual 40-file ZIP was inspected. It has no root README.TXT/FILE_ID.DIZ;
text editions flatten Markdown, use UTF-8/LF, discard link destinations and expose
internal conference IDs. The joining notice is missing, END instructions name
CNTEST, and key replacement is deferred. Retained CNCONFS.TXT (June 1, 1991), read
without modification using explicit CP437 decoding, documents SUPPORT/SYSOP/SPITFIRE/
DOORS as Sysop/visiting-Sysop only. Preserve that classification: SUPPORT core,
other three optional. Visiting Sysops need explicit local verification/access grants;
network identity never grants a caller account privileges. No new testing area:
operators may agree a short operational test in SUPPORT.

## Interfaces defined before implementation

- Catalog schema 2 adds typed access (public or sysops); schema 1 remains verifiable
  byte-for-byte with omitted default public access. Retain revision 1 unchanged and
  publish a consecutive signed revision 2 for the access clarification.
- Wire remains 1.4. Optional eighth capability catalog-access gates schema-2 snapshots
  and Sysop-only traffic. Older peers can receive schema-1 history and supported
  public/message/file/control exchanges; no downgrade of restricted traffic.
- Native conference numbers/security remain local. Create+map chooses restrictive
  native security for Sysop areas; map-existing rejects an inadequately restricted
  area. The same check applies at delivery/export after a local policy edit. Local
  privileged security levels are for independently verified visiting Sysops, never
  populated by catalog import. No account identity or message body is audited.
- Normal administration takes codenames and human fields. Typed draft actions generate
  random immutable IDs internally; reactivation retains identity, explicit reuse
  generates a new one. Advanced detail/export retains IDs for diagnosis/history.
- Schema 35 retains signing-key epochs. A typed transition binds old/new keys, exact
  current revision/hash, mode, time and decision rationale/reference. Planned rotation
  carries old and new signatures; emergency replacement carries new-key proof plus
  explicit local sensitive-configuration approval and independently confirmed new-key
  fingerprint. Neither transition is accepted automatically from transport. Publisher,
  network and catalog identity do not change. No history is discarded or re-signed.
- Every node applies the transition at the same verified catalog checkpoint. Later
  revisions use the new key; retained revisions verify under their original epochs.
  Restore cannot erase known epochs or catalog knowledge. Conflicting history requires
  investigation, not a force/rollback escape hatch. Normal routing needs no signing key.
- One release metadata file supplies joining availability/contact and reproducible
  provenance. Unchosen contact fields explicitly keep applications closed. No secrets.
- Purpose-built ASCII/CRLF text editions derive from canonical Markdown and signed
  catalog data. Root entry point and DIZ, preserved filenames/URLs, readable lists,
  generated entry-oriented CONFS/CHANGES, deterministic manifest and external ZIP hash.

## Implementation and release review

The kit is now 67 explicitly allowlisted files, with literal root README.TXT and
FILE_ID.DIZ, 24 root member TXT editions, Markdown/config/technical subdirectories,
RELEASE.TXT, MANIFEST.json and MANIFEST.sha256. ZIP checksum is a sidecar. Canonical
Markdown remains the writing source; generated CONFS/CHANGES derive from signed
catalog data. BBS output is ASCII, CRLF and <=79 columns; DIZ is five lines <=45.
Links, filenames, literal commands and the topology diagram survive rendering.
A fresh extraction verified all 66 checksum-manifest entries and both catalog
signatures; focused tests reproduce identical archives and resolve member links.

Revision 1 remains byte-for-byte retained at config/catalog-history/000001.json,
hash `84defdfbfb50b9eca68cf9bbf086e0a49ad6492caf8dc536bf3ff923f3ce349c`.
Revision 2 hash is `c1056331f097761f1ca574092be66995d0dd014534877f7bce79a4f9fc6cc9d0`.
The 46 conference identities remain unchanged. Only the four operator access
classifications/effective revisions and signed publication metadata change.
The public key remains the founding authority; no real network key rotation was
performed. All rotation/recovery tests use disposable synthetic keys and boards.

The optional catalog-access capability extends the existing minor 1.4 without a
minor/major bump. Catalog schema 2 is an independently versioned signed-object
schema. Older catalog-sync peers receive only schema-1 predecessor history and
supported public/control/file traffic. Restricted work stays queued. Existing
Event empty-drain protection prevents unsupported work becoming a session storm.
A subsequent signed catalog cannot downgrade its object schema. Review also fences
public generations absent from the older peer's compatible snapshot, including
already-frozen retry artifacts, so unsupported work cannot reject/starve known areas.
A focused test freezes new-generation work, queues older compatible work, and proves
the latter still reaches the old-schema receiver. Another test proves repeated key
replacement at the same cutoff when a just-enrolled key is lost before first use;
monotonic epochs preserve both transitions without demanding a lost-key signature.

The kit explicitly marks applications not yet open: no administrator/contact or
application endpoint has been chosen in this session. One release metadata source
supplies all joining fields and points to the public repository's stable joining
information path. This is an actionable closed-enrollment state, not a missing
notice or invented contact. The final release must not represent enrollment as open
until real public details are entered and regenerated.

The Charter retains nine independent boards/60 days, seven committee seats and
five-member quorum. It now names an interim Secretary/first-election procedure,
ends bootstrap at the first quorate appointed-office handoff, prevents return of
founding powers after later quorum loss, and explicitly waives nonexistent committee
recommendation during bootstrap amendments while retaining member ratification.
No election software or other governance engine was added.

## Reproduction and meaningful test corrections

Build the catalog-artifact utility, run the kit unittest module and build into a
fresh output directory as documented in the distribution README. The actual ZIP,
not only source Markdown, is the object of manifest/link/ASCII/width/root-entry
review. The native tests exercise restricted mapping, caller listing/read/QWK
queue denial, deliberate verified-visitor access, old-peer withholding, generated
conference identity through retirement/reactivation/reuse, key transition signatures,
restart, retained epochs, migration atomicity and backup/restore high-water protection.
The isolated real daemon journey passed on Apple Silicon in 78.26 seconds; the
final full-workspace run additionally covers the settled implementation. It exercises
CLI creation/mapping/listing, schema-2 propagation,
exactly-once restricted traffic and planned/emergency local trust transitions
across ROOT/HOST/END, with graceful shutdown.
The existing six-node catalog/Files/Events/routing journeys remain regression gates.

During development, a new identity test initially collided with an unrelated fixture
mapping; the fixture now removes that mapping before selecting a different codename.
The live access test initially reused a Sysop account as an ordinary account, then
used the older public-conference helper's intentionally non-Sysop threshold for a
restricted post. It now creates a real low-security caller and uses the board Sysop
identity correctly and the native All Callers public-conference recipient token.
These failures were test setup errors, not claimed acceptance. A superseded full
run was stopped to avoid competing suites; the following full run still contained
the earlier recipient fixture and failed that one new case while all six existing
CircuitNET live journeys passed. Final acceptance uses the later complete run from
settled source, not a sum that hides those unsuccessful attempts.
Review also found catalog-draft reset schema to 1; it now retains the accepted schema,
and the live test edits schema-2 metadata through the normal CLI to cover that path.

## Required final report

| # | Item | Result |
| --- | --- | --- |
| 3 | Schema | 34 -> 35; immutable signing-key epochs, no speculative subsystem tables. |
| 4 | Runtime/protocol | Automatic identity/human catalog CLI; Sysop access enforcement; deliberate planned/emergency key recovery. Wire remains 1.4; optional catalog-access; object schema 2. |
| 5 | Kit structure | Old 40-file wrapper/Markdown/text layout -> 67 files with human TXT at ZIP root and Markdown/config/technical below. |
| 6 | README.TXT | Obvious entry point and reading/setup paths, no protocol dump. |
| 7 | FILE_ID.DIZ | Five ASCII/CRLF lines, <=45 columns, docs/config scope. |
| 8 | ABOUT | Network, roles, terms, ASCII tree, exchange and visibility. |
| 9 | GOALS | BBS conversation, preservation, experimentation, interoperability and conduct. |
| 10 | CONFS | Entry-oriented codename/name/description/access/core/status; no opaque IDs. |
| 11 | CHANGES | Generated signed-history changes, no IDs; supports add/update/deprecate/retire/reactivate/explicit reuse. |
| 12 | FILES | Shared native custody, separate mappings/subscriptions, local safety and access, no official file-area governance yet. |
| 13 | Joining | Explicit current information plus application/approval/enrollment sequence. |
| 14 | Contact strategy | config/release.json; public contact unchosen, applications closed, stable public joining-information path. |
| 15 | Application | Text form and web field specification; minimal data, no authentication-secret fields, no static-IP requirement for END. |
| 16 | END | Complete command sequence from approval through certificate enrollment, pin/seed, Test Link, sync, map, Dossier, Event, arranged SUPPORT exchange and diagnosis. |
| 17 | HOST | Parent/children, certificates/listener, local/remote Dossiers, approvals, route and Events, catalog/files/health. |
| 18 | ROOT | Technical service, separate governance, protected publication, child service, backup and recovery. |
| 19 | CNTEST | Absent from member setup; no new governed area. Arrange short Sysop SUPPORT test. |
| 20 | Example seed | Explicit non-importable worksheet; own-directory-relative ../markdown/JOINING.md; no prescribed endpoint/port. |
| 21 | Version contract | Opening supports minors 0-4; later 1.4/catalog-sync/catalog-access agrees. |
| 22 | Signing custody | Designated protected publication custodian, distinct from TLS identity. |
| 23 | Offline signing | catalog-artifact signs away from ROOT; signed import preserves normal authority/chain checks. |
| 24 | Key backup | Explicit risk/availability decision, encrypted isolated recovery copy, separate unlock custody, restore test; retain public history. |
| 25 | Loss | Verified protected recovery then replacement, or explicit emergency enrollment if old key unavailable. |
| 26 | Compromise | Announced incident/checkpoint, stop publication, independent fingerprint confirmation; preserve evidence and reconcile divergent history. |
| 27 | Replacement | Dual-signed planned / new-key-proof emergency transition; local privileged confirmation; safe audit; wrong signatures/fingerprint/checkpoint fail. |
| 28 | Node trust | Every node explicitly enrolls at exact cutoff; old epochs retained, no automatic packet pin change. |
| 29 | Bootstrap | Transitional authority and exact handoff clarified without changing governance model. |
| 30 | Secretary | Founder appoints interim; normal committee appoints successor; role need not occupy a committee seat. |
| 31 | First transition | Interim Secretary administers ballot with independent checking; quorate meeting appoints offices and publishes handoff. |
| 32 | Quorum loss | No founding-power revival; caretaker service/security only, vacancy elections and published reviews. |
| 33 | Amendments | Committee recommendation waived only during bootstrap; member ratification retained. |
| 34 | Renderer | Purpose-built paragraphs/headings/lists/records/literals/links, deliberate ASCII transliteration and orphan-word handling. |
| 35 | Width | <=79 columns; literal overwidth rejects; DIZ <=45. |
| 36 | Encoding | ASCII BBS edition; UTF-8 Markdown separately retained. |
| 37 | Line endings | Delivered TXT/DIZ CRLF; repository derived compatibility source uses LF. |
| 38 | Links/filenames | Visible TXT mappings, preserved external URLs; no silently removed member link destination. |
| 39 | Historical draft language | Adopted rationales replace residual review-before-adoption wording; source signed revision remains unchanged. |
| 40 | History | Rights-safe HISTORY explains 71/60/5/6 historical reconciliation vs modern 46, four operator areas and explicit identity-policy change. |
| 41 | Limits | 256 retained identities, 4,096 revisions, 64 key epochs; coordinated topology migration; Files bounds referenced in included technical custody spec. |
| 42 | Security | Encrypted/authenticated transport != E2EE/public-message secrecy; directed != private; files obey destination access. |
| 43 | Provenance | Version/build revision, public source reference, content digest, public key fingerprint, fixed timestamps, external ZIP checksum. |
| 44 | Manifest | Exact allowlist with names/sizes/SHA-256 and external checksum; no recursive self-hash. |
| 45 | Catalog verification | Original revision 1 and new revision 2 signatures/hash chain verified. |
| 46 | Outsider ZIP review | Fresh extraction: entry path, roles, conference/file status, joining closure, application, guides, links, keys, version, manifests and BBS bytes checked. |
| 47 | Tests added | Seven new core/wire tests, one daemon journey; kit suite expanded from two to four tests with delivered-byte assertions. en-US advances to 1.32.0 / 1,432 strings. |
| 48 | Workspace totals | Private: 836 passed / zero failed / seven existing ignored, eight doctest groups. Public: 774 passed / zero failed / seven existing ignored, six doctest groups. Both complete settled-source workspace runs. |
| 49 | Regressions | Private and public C2-C7, FTN/BinkP, QWK, six-node Files/catalog/routing, Events, restore and new C7.1 live journey PASS. |
| 50 | Headers/fmt/Clippy/diff | Private/public source headers 174/151, fmt, strict workspace/all-target Clippy and diff PASS. |
| 51 | Markdown/local links | Private: 236 Markdown files / 1,600 local links; public: 164 / 1,128; zero issues. Delivered member references also resolve. |
| 52 | Privacy/provenance | No private keys, credentials, raw historical artifacts or acceptance payloads in allowlist; public catalog key only. |
| 53 | cargo-audit | Unavailable locally (command not installed); no audit success claimed. |
| 57 | Public delta | Sanitized modern code/docs/tests only; this source revision records accepted public validation. |
| 58 | C8 | NONE. |
| 59 | Legacy CNP/CND | NONE. |
| 60 | Private mail/E2EE | NONE. |
| 61 | Production changes | NONE. |
| 62 | External live traffic | NONE; synthetic loopback boards only. |
| 63 | Exact next action | Generate the revised kit with this public source commit; review release readiness and choose public joining/contact details before opening applications. Stop before C8. |

C7 architecture remains accepted. C7.1 is bounded release readiness/humanization,
not a redesign. Network identities remain independent of local numbering; native
messages/files remain canonical. Member-facing documents contain no milestone or
developer shorthand and do not announce accessibility. Signing-key recovery has
an implemented, documented, explicit local path; no secret stays on ROOT merely
for routing. README.TXT starts the kit journey. C1 remains historical authority;
C2-C7 remain the accepted foundations. No production systems changed and no C8
work began.
