# M072 — CircuitNET public identity activation

C7.2 public identity metadata is complete and accepted. Schema 35 and protocol 1.4
remain unchanged. Applications are Closed; no DNS, web or mail deployment occurred.
This rights-safe report contains no private source history or acceptance artifacts.

## Interface decision before implementation

Extend the existing config/release.json authority with typed public identity:
network display name, official domain, role local-parts and fixed publication paths.
The builder validates these fields and derives HTTPS URLs/role addresses. Generated
sections in canonical Markdown, joining information, release provenance and the
planning worksheet consume those values. Applications remain closed; service status
is explicitly not verified. Recording a URL does not deploy or validate a service.

The existing signed catalog remains unchanged. A derived public signature sidecar
contains the wrapper signature in lowercase hexadecimal plus LF; it is not a new
signature of the JSON wrapper. The public-key sidecar retains accepted hexadecimal
encoding. Fingerprint derives from SHA-256 of the actual accepted raw public key.
No database, wire, governance, Events or Files behavior changes.

## Release scope and review

The existing release authority now derives URLs from one domain plus seven endpoint
paths and two role local-parts. PublicIdentity validates the field set, domain,
roles, path syntax and explicit application/service state. Generated source sections
are delimited and checked against metadata; the comments do not appear in delivered
Markdown or TXT. Full JOINING-INFO/PUBLIC-IDENTITY pages, the planning worksheet and
RELEASE.TXT derive from the same projection. Network Kit remains 1.0, revised build 3.
The kit has 70 files and 69 checksum entries; original catalog revisions and key are
unchanged. No schema, wire, Rust runtime or governance change occurred.

Applications are Closed. All web/mail/download locations are canonical intended
locations, with service deployment explicitly not verified. No DNS, HTTP or SMTP
probe was performed; there is no need to infer live infrastructure for this task.
The Founding Administrator is a public role address, not a personal address.

The endpoint contract is in generated
[PUBLIC-IDENTITY](../circuitnet-ng/PUBLIC-IDENTITY.md) and the
[catalog specification](../technical/circuitnet-catalog.md#public-discovery-and-publication-endpoints).
The future authority page must derive current revision, body SHA-256, publisher,
publication date, previous revision and fingerprint from actual artifacts, with
protocol version from the published contract. Catalog signatures still cover the
canonical body with the accepted signing domain. The signature/public-key sidecars
use lowercase hexadecimal plus LF. HTTPS supplies discovery/transport, not a trust
replacement. Future independent technical standards stewardship is recorded only;
no alternate domain or organization is an active dependency.

Placeholder review found no member-facing placeholder domain/contact. The technical
transport's illustrative fingerprint placeholder remains intentionally replaceable
with an enrolled neighbor fingerprint. FILES' “not yet” statement refers to genuinely
unfinalized official file-area governance, not public identity. Other test/example
domains occur only in automated renderer/negative fixtures, outside the kit. Public
source-repository ownership and copyright attribution remain intentional provenance,
not personal application contacts. Member editions contain only the configured join
and founder email roles; no local path, localhost address or private contact leaked.

Two focused Python tests were added (six total). They validate all official paths,
role-address derivation, closed/unverified state, invalid path/role rejection and
opening-state guard. A copied source fixture changes only the domain, confirms stale
derived material is rejected, regenerates and builds the actual ZIP, and verifies
all derived locations, authority fingerprint, signature sidecar and role-only emails.
Existing tests cover reproducibility, exact manifest inventory, signatures, generated
catalogs, Charter, source links and ASCII/CRLF/79-column BBS output. The new fixture
initially needed canonical temporary-path resolution on macOS and whitespace-aware
assertion for wrapped prose; both fixture corrections passed before acceptance.

The fresh candidate ZIP review verified README/home, joining/application/role
addresses, all catalog paths, Closed status, actual key fingerprint, manifest and
both accepted catalog signatures. FILE_ID.DIZ remains the root five-line description.
No C7.1 renderer, protocol, conference, key or native message/file authority was
redesigned. Full workspace regression and final publication results follow below.

## Required final report

| # | Item | Result |
| --- | --- | --- |
| 3 | Schema | 35 -> 35. |
| 4 | Protocol | CIRCUITNET-NG 1.4 -> 1.4; no capability changes. |
| 5 | Identity authority | docs/circuitnet-ng/config/release.json, validated PublicIdentity in tools/build-circuitnet-kit.py. |
| 6 | Domain | circuitnetng.org. |
| 7 | Joining | join@circuitnetng.org. |
| 8 | Founder | founder@circuitnetng.org. |
| 9 | Application | https://circuitnetng.org/apply. |
| 10 | Kit | https://circuitnetng.org/downloads/infopack.zip. |
| 11 | Human catalog authority | https://circuitnetng.org/network/catalog. |
| 12 | Catalog JSON | https://circuitnetng.org/catalog/catalog.json. |
| 13 | Signature | https://circuitnetng.org/catalog/catalog.sig. |
| 14 | Public key | https://circuitnetng.org/catalog/catalog-authority.pub. |
| 15 | Applications | Closed; all service deployment/reachability unverified. |
| 16 | README | Generated official home/kit locations with deployment distinction. |
| 17 | JOINING | Generated role contacts, apply location, closed-state instructions; no submission until opening. |
| 18 | Application | Generated intended submit paths above ordinary form; no secrets; separate field specification retained. |
| 19 | Roles | END joining locations; HOST/ROOT public administrator role; enrollment mechanics unchanged. |
| 20 | Verification | Generated human/machine locations and fingerprint; HTTPS never silently establishes/replaces trust. |
| 21 | Fingerprint | SHA-256 of actual accepted raw public-key bytes, derived during every build. |
| 22 | Placeholders | No member identity placeholders; intentional technical fingerprint fixture and future file-area status retained. |
| 23 | Contact leakage | Member emails exactly the two configured role addresses; source/legal attribution retained as provenance. |
| 24 | Single source test | Domain fixture rebuilds actual ZIP; stale documents fail; every derived URL/role follows metadata. |
| 25 | BBS | Accepted renderer: ASCII, CRLF, <=79 columns; generated source markers omitted. |
| 26 | DIZ | Root FILE_ID.DIZ retained, five lines <=45 columns; no changing contacts. |
| 27 | Manifest | Exact 70-file kit / 69 checksum entries, sizes/SHA-256 and external archive checksum. |
| 30 | Catalog signatures | Both accepted signed revisions verify; unchanged catalog/key, signature sidecar matches wrapper. |
| 31 | Tests | Two Python tests added; six kit tests PASS. No Rust behavior/test changes. |
| 32 | Workspace | Full private workspace: 836 passed / zero failed / seven existing ignored; eight doctest groups; all seven live CircuitNET journeys pass. No runtime source changes. |
| 33 | Quality | Private/public headers174/151, fmt, strict all-target Clippy and diff PASS. |
| 34 | Links | Private 238 Markdown files / 1,617 local links; public 166 / 1,142; zero issues. Generated/extracted kit references PASS. |
| 35 | Provenance | Explicit public source commit, no private keys/credentials/infrastructure details in kit. |
| 36 | cargo-audit | Unavailable; no audit success claimed. |
| 39 | Public delta | Release metadata, generated docs, builder/tests and safe report only. |
| 40 | DNS | NONE. |
| 41 | Hosting | NONE. |
| 42 | Email services | NONE. |
| 43 | C8 | NONE. |
| 44 | Production | NONE. |
| 45 | Next action | Generate final kit from this accepted public source revision, then stop for C7.2 review. Deployment/opening requires separate explicit work. |

The chosen domain is the accepted canonical public identity. Recorded URLs do not
claim deployed/reachable services. Applications remain Closed. C7.2 changes release
metadata/documentation plus narrowly supporting build validation/generation only;
C1-C7.1 remain accepted. No DNS, registrar, hosting or email production system was
changed. C8 did not begin.

## Public validation scope

All 19 allowlisted distribution/builder paths match accepted private source exactly.
Public six-test Python suite (including changed-domain actual ZIP), signature/build,
headers151/fmt/strict Clippy/diff and 166 Markdown files/1,142 local links pass.
No Rust/Cargo/runtime source differs from the accepted public checkpoint, so the
complete 836-test private workspace run provides this pass's full regression gate;
the unchanged public Rust workspace's 774/0/7 accepted run is retained as prior
validation, not misrepresented as a new full public run. No source binary release,
website deployment, email activation or application opening was performed.
