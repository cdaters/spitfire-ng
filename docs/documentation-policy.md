# SPITFIRE NG Documentation Architecture and Source-Header Policy

**Status:** Architecture and initial human-documentation foundation accepted;
source-header migration complete

**Applies to:** Project-authored documentation and source

## Purpose

SPITFIRE NG has a strong body of specifications, historical research,
operator notes, and verification records. This policy preserves that work and
adds a durable human-facing documentation system. A Sysop should be able to
install, configure, run, secure, maintain, back up, and troubleshoot a board
without first learning the Rust workspace, database schema, or internal domain
model. Callers should have a concise guide of their own. Developers must still
have the exact technical and historical evidence needed to maintain the
system.

This is an architecture gate, not a wholesale rewrite. Existing documents
remain authoritative until a replacement is complete, reviewed, linked from
the documentation index, and—when a path changes—given a useful redirect or
index pointer.

## Historical Inspiration

A bounded review of the SPITFIRE 3.7 `SPITFIRE.DOC` found a practical pattern
worth carrying forward:

- orient the Sysop before presenting reference detail;
- offer a real getting-started path for readers who want a board running
  first;
- explain commands and features in the context where a Sysop uses them;
- describe what callers see and the operational consequences of a setting;
- follow procedural chapters with technical reference, troubleshooting, and
  an index.

SPITFIRE NG adopts that teaching philosophy, not the historical manual's
wording, DOS procedures, or chapter structure. Modern architecture and current
capability determine the new manual. Primary SPITFIRE evidence remains the
authority for historical compatibility claims.

## Audiences and Documentation Layers

SPITFIRE NG documentation has five layers:

```text
SPITFIRE NG Documentation
├── Sysop Reference Manual
├── Caller Guide
├── Technical Reference
├── Historical and Compatibility Research
└── Contributor, Development, and Continuity Guidance
```

### Sysop Reference Manual

The manual is for a person operating a SPITFIRE NG BBS. It answers:

1. What is this feature?
2. Why does a Sysop care?
3. How is it configured and used?
4. What does the caller experience?
5. What important security, recovery, or compatibility consequence follows?

It uses BBS and Sysop terminology, commands that actually exist, short
examples, and task-oriented troubleshooting. It does not lead with schemas,
transactions, traits, stable identifiers, or architecture vocabulary.

### Caller Guide

The Caller Guide is for people connecting to a board. It covers connecting,
accounts and handles, navigation, messages and conferences, file discovery,
tagging, uploads and downloads, transfer choices, preferences, clean logoff,
and common problems. It does not reveal Sysop-only controls, private board
configuration, host paths, security internals, or operational recovery data.

The guide should be suitable for repository and web publication, optional
download, and reuse by contextual help.

### Technical Reference

The Technical Reference preserves implementation-level truth: architecture,
crate and module boundaries, schema and migration history, stable identifiers,
state machines, transactions, concurrency, leases, version conflicts,
security boundaries, protocol engines, storage roots, backup internals,
localization and presentation architecture, compatibility adapters, audit,
operator APIs, and extension points.

Technical terminology belongs here when it improves precision. The reference
links to canonical research instead of reproducing evidence archaeology.

### Historical and Compatibility Research

Research documents preserve evidence gates, corpus indexes, format studies,
original-runtime findings, interoperability reports, provenance, and the
classification of confirmed, inferred, modernized, and unresolved behavior.
They support engineering truth; they are not manual chapters. Human-facing
documents summarize their accepted conclusions and link to them when deeper
evidence helps.

### Contributor, Development, and Continuity Guidance

This layer includes development rules, roadmap and milestones, decisions,
session continuity, release engineering, worktree policy, and contributor
instructions. It explains how the project is built and governed, not how a
Sysop operates a board. Private continuity remains private unless separately
sanitized for publication.

## Canonical Source Hierarchy

The following is the target hierarchy. Directories and chapters should be
created only when they contain reviewed, useful content.

```text
docs/
├── manual/
│   ├── README.md
│   ├── quick-start.md
│   ├── installation-and-setup.md
│   ├── running-your-board.md
│   ├── callers-and-security.md
│   ├── messages-and-conferences.md
│   ├── files-and-transfers.md
│   ├── menus-and-presentation.md
│   ├── nodes-and-transports.md
│   ├── operations-backup-and-security.md
│   ├── troubleshooting.md
│   ├── reference-tables.md
│   └── glossary.md
├── caller-guide/
│   ├── README.md
│   ├── connecting-and-accounts.md
│   ├── messages.md
│   ├── files-and-transfers.md
│   └── preferences-help-and-logoff.md
├── technical/
│   ├── README.md
│   ├── architecture.md
│   ├── persistence-and-migrations.md
│   ├── sessions-and-transports.md
│   ├── messages.md
│   ├── files-transfers-and-storage.md
│   ├── presentation-and-localization.md
│   ├── operations-backup-and-security.md
│   └── compatibility-adapters.md
├── research/
└── operator/
```

`docs/README.md` remains the overall router. During migration,
`docs/operator/` remains the source of current task guides; completed manual
chapters may replace or summarize them one subject at a time. Existing
top-level technical specifications remain canonical until deliberately mapped
into `docs/technical/`. Research documents stay under `docs/research/` and are
not moved merely for visual tidiness.

The repository Markdown is the canonical editable source. The website and any
downloadable manual are generated or synchronized from reviewed public-safe
Markdown; they are not independently edited copies. Website publication is a
separate, authorized operation.

### Initial publication handoff

The first canonical human-documentation entry points are:

- `docs/manual/README.md` — **Sysop Reference Manual**;
- `docs/manual/quick-start.md` — **Quick Start**;
- `docs/caller-guide/README.md` — **Caller Guide**; and
- `docs/technical/README.md` — **Technical Reference**.

A future website synchronization should place those three reader layers under
**SPITFIRE NG > Manuals & Documentation**, with Quick Start immediately
reachable from the Sysop Manual. The page titles, source-current applicability
notice, normal Markdown anchors, and `help-topic` identifiers must survive the
rendering step. Website navigation may add summaries and breadcrumbs, but the
body is synchronized from these reviewed Markdown sources rather than edited
as a second manual.

Release-manual builds take an immutable snapshot of the same files at the
release tag. They replace the source-current notice with the release identity
without rewriting operational content. Publishing either website pages or a
release manual remains a separate authorized task; this handoff changes no
website or DDEV state.

## Sysop Manual Structure

The manual should read as a small number of coherent parts rather than a long
list of empty placeholders:

1. **Welcome and Quick Start** — product identity, source/release note, the
   shortest safe first-board path, and where to continue.
2. **Installing and Setting Up** — supported environments, files and
   directories, first-time setup, board identity, Sysop account, timezone,
   nodes, listeners, conferences, and file areas.
3. **Running the Board** — startup and shutdown, caller accounts, security,
   messages, conferences, files, transfer protocols, tagging and batches,
   limits and credits, and extended/read-only storage.
4. **Menus and Presentation** — commands, display resources, profiles,
   localization, terminal compatibility, and fallbacks.
5. **Connectivity and Multinode Operation** — Telnet, RAW, RLogin, SSH, node
   ownership, and safe deployment boundaries.
6. **Sysop Operations** — routine inspection and maintenance, backup,
   recovery, upgrades, security, and privacy.
7. **Compatibility, Troubleshooting, and Reference** — legacy SPITFIRE
   boundaries, symptoms and recovery, reference tables, and glossary.

Only implemented or explicitly documented available behavior belongs in
procedural chapters. Planned capabilities may appear in a clearly marked
future note, never as instructions.

## Quick Start Contract

The Sysop Manual must contain a genuine Quick Start. It should let an
impatient but careful reader take the shortest supported path through:

- installation and first-time setup;
- board identity, Sysop account, and board timezone;
- at least one node and caller transport;
- the initial message conference and file area;
- board startup and a compatible terminal connection;
- a test call, message, file listing, test download, and clean logoff; and
- the first cold backup.

Each command and path must be verified against the current source or the
specific release named at the top of the page. If setup creates a starter
conference or file area, the guide says so rather than inventing a separate
creation step. If a requested path is not implemented, the page identifies a
product/documentation gap instead of fabricating a command.

The Quick Start ends with links to the chapters for configuration, security,
files and transfers, backup, and troubleshooting.

## Human-Facing Writing Standard

Human-facing pages use ordinary, direct language. Lead with the outcome and
the Sysop's task. Define unfamiliar terms on first use. State prerequisites,
show a realistic example, explain what the caller sees, warn about important
consequences, and provide a recovery path when an operation can fail.

Avoid implementation-first explanations and unnecessary abstractions. Terms
such as *semantic authority*, *projection*, *dispatch-time reauthorization*,
and *idempotent settlement* usually belong in the Technical Reference. In a
manual, translate them into their operational effect. Do not reduce accuracy:
link to the Technical Reference when the distinction matters.

Human-facing manuals read as product documentation. They do not mention
prompts, assistants, work sessions, chat history, or the mechanics by which a
milestone was developed. Verification history belongs in research or
continuity records.

### Representative Sysop style

> **Quick Start**
>
> If you want to see your board running before you read the whole manual,
> start here. You will create a board in its own directory, review the setup
> summary, start one caller listener, make a test call, and create a backup.
> The setup program installs starter message and file areas, so you can test
> the basic caller experience before designing the final board.
>
> Keep the board private while you work through these steps. Before accepting
> Internet callers, read Security and Privacy and choose which encrypted and
> legacy transports you are prepared to offer.

> **Batch downloads**
>
> A caller can tag several files and review the total before starting a batch
> transfer. SPITFIRE NG checks the entire batch against the caller's remaining
> download allowance. Each file is counted only after it transfers
> successfully, so a failed or cancelled file does not use the caller's
> allowance. If a file changes after it was tagged, the caller must refresh or
> remove that item before continuing.

### Corresponding Technical Reference style

> **Batch queue and accounting boundary**
>
> A batch queue is bounded, ordered, and session-ephemeral. Each item retains a
> stable `FileId` and expected file version. Dispatch reauthorizes every item,
> atomically reserves the batch's chargeable quota, and settles each completed
> member once using its settlement identity. Failed and unstarted members
> release their reservation and remain available to the caller's queue
> recovery flow. Queues are never reconstructed after disconnect or daemon
> restart.

The two treatments describe the same behavior for different readers. Neither
is a substitute for the other.

## Documentation Inventory and Migration Map

| Existing area | Primary classification | Disposition |
| --- | --- | --- |
| `README.md`, `docs/README.md` | Contributor/front door; mixed index | Keep concise and route readers to the three primary layers. |
| `docs/operator/README.md`, getting-started, installation, first-board, configuration, messages, files, transfers, terminals, backup, upgrades, troubleshooting | Sysop Manual source material | Reuse substantially, remove release-test narration, consolidate overlaps, and add version labels during incremental manual authoring. |
| Caller-facing portions of the existing message, file, transfer, and terminal-client guides | Caller Guide source material; mixed | Extract only caller-safe instructions; keep Sysop controls out of the Caller Guide. |
| `docs/operator/operator-gap-analysis.md`, publication plans, development-preview package records, and board-specific test guides | Mixed project/release/continuity | Summarize applicable facts; do not turn checkpoint evidence or a specific test board into manual prose. |
| `docs/01-*.md` through `docs/15-*.md`, `docs/sfng-*.md`, presentation/localization/backup/security specifications | Technical Reference; some mixed operator explanation | Preserve as current canonical specifications, then add a technical index and migrate by subject without deleting detail. |
| `docs/research/` and `docs/reverse-engineering/` | Historical/Compatibility Research | Preserve in place. Manuals cite accepted conclusions, not private samples or research paths. |
| `AGENTS.md`, `CONTRIBUTING.md`, `ROADMAP.md`, `MILESTONES.md`, `docs/DECISIONS.md`, `docs/SESSION-LOG.md`, `CURRENT-STATE.md` | Contributor/Development or Continuity/Project Management | Keep outside the manuals. Publish only documents already public-safe or separately sanitized. |
| `release/` | Release and provenance records | Keep immutable per release; link from version notes rather than copying into current-source instructions. |

Detailed routing within those groups is:

- `docs/01-project-charter.md`, `02-compatibility-principles.md`,
  `05-compatibility-matrix.md`, and `15-development-roadmap.md` are project
  definition or planning references. Human manuals may summarize their stable
  product promises.
- `docs/03-security-philosophy.md`, `04-system-architecture.md`,
  `06-legacy-file-formats.md` through `14-nodes-events.md`, and the
  `docs/sfng-*.md` specifications are primarily Technical Reference material;
  their historical claims continue to point into research.
- `docs/HISTORICAL-SPITFIRE.md`, `stock-spitfire-3.7-parity.md`, and the
  publication documentation index are historical/reference bridges, not
  procedural manuals.
- `classic-presentation-profile.md`, `presentation-profiles.md`, and
  `localization.md` are technical specifications with corresponding practical
  source material already under `docs/operator/`.
- `cross-project-reference-policy.md` and `development-worktrees.md` are
  contributor/development guidance.
- `sfdraw.md`, `sfreg-005-registration-manager.md`, and
  `future/modern-bbs-terminal-client.md` are future component specifications;
  they must not be surfaced as current manual capability.
- Every document under `docs/research/` or `docs/reverse-engineering/` remains
  historical, compatibility, interoperability, or preservation research.
- Within `docs/operator/`, installation, configuration, caller management,
  messages, files, transfers, terminals, localization, presentation, backup,
  upgrades, support, and troubleshooting are manual source material. The
  board-specific test guide, gap analysis, and publication plan remain
  continuity/release records and are summarized rather than copied.

### Duplication and exposure to resolve incrementally

- Getting Started, First Board, Installation, and package-specific setup repeat
  the same path with different checkpoint language.
- Files and transfers are explained in both operator guides and internal
  specifications without a clear audience boundary.
- Status, roadmap, milestones, and research reports repeat capability summaries
  that can drift.
- Some operator documents contain test-board names, local-development detail,
  and acceptance narration that a Sysop should not need.
- There is no unified Caller Guide, glossary, reference-table set, or stable
  help-topic namespace.
- Existing task guides do not consistently distinguish current source from the
  older downloadable Development Preview.

No useful detail should be deleted to solve these problems. Establish the new
canonical page, redirect the old index entry, then retire duplication only
after link and content review.

## Version and Release Labels

The current manual tracks current source. Every procedural page carries a
short reader-facing applicability block when source and downloadable release
differ:

```text
Applies to: Current source (schema 17)
Downloadable release: Not yet included
Latest downloadable release: SPITFIRE NG 0.1.0 Development Preview
```

Use product versions in human-facing pages; mention schema only when it helps
an upgrade, recovery, or compatibility task. A tagged release should produce
an immutable documentation snapshot from the same canonical Markdown. Older
release manuals are not edited to track `main`. The website may later offer a
version selector, but each rendered page must remain understandable on its
own.

## Documentation Is Part of Feature Completion

A capability is not documentation-complete until the affected audiences have
the documentation they need. Every feature gate or milestone records its
documentation impact:

- update the Sysop Manual when installation, configuration, operation,
  maintenance, security, backup, or troubleshooting changes;
- update the Caller Guide when a caller-visible workflow or result changes;
- update the Technical Reference when architecture, data, protocol,
  concurrency, recovery, security, or extension boundaries change;
- update contextual help when an interactive choice changes;
- update Quick Start only when the shortest first-board path changes; and
- update historical research only when new evidence changes a compatibility
  conclusion.

Not every capability touches every layer. A completion report must state which
layers were updated and why the others were unaffected.

## Contextual Help

Manual topics intended for reuse receive stable, product-owned identifiers,
for example `sysop.quick-start`, `files.batch-downloads`, and
`transport.ssh`. IDs describe a concept rather than a file path or UI widget.
Future setup, CLI, `sfconfig`, and `sfmonitor` surfaces may show a concise
localized explanation and point to the matching topic. They should not embed
large copies of manual chapters.

Moving a Markdown file must not silently break a topic ID. A later tooling
pass may validate unique IDs and build website/help mappings; this gate adds
no operator tool or help runtime.

## Public and Private Boundary

| Material | Publication class |
| --- | --- |
| Sysop Manual and Caller Guide | Public by default after normal review. |
| Technical Reference | Public by default; sanitize private evidence paths and operational identities. |
| Historical format conclusions and rights-safe interoperability summaries | Public after sanitization. |
| Corpus indexes, acquisition paths, screenshots/captures, private test identities, unpublished artifact hashes, and proprietary samples | Private. |
| Current state, session log, decision archaeology, and private milestone continuity | Private or separately summarized for public status. |
| Contributor rules and security policy | Public when already written for public use. |

Public manuals never require private research material to be useful.

## FireComm Reference Boundary

SPITFIRE NG and FireComm remain independent projects. A bounded read-only
review of FireComm's manual, developer guide, identity/header policy, and
source-header validator informed this gate.

- **Adopted:** clear separation of user, developer, and historical material;
  concise project headers; and automated validation as a future enforcement
  mechanism.
- **Adapted:** SPITFIRE NG needs separate Sysop and Caller audiences, plus a
  stronger historical-evidence boundary.
- **Rejected:** treating another project's exact hierarchy or wording as a
  template to copy.
- **Deferred:** any shared documentation tool, validator, crate, or library.
- **FireComm-specific:** its application workflows and platform presentation
  instructions.

FireComm may inform implementation technique and interoperability. It is not
historical SPITFIRE authority, a build dependency, or a source of code to copy.
Any reusable abstraction is documented first and extracted only under a
separate decision. The reciprocal learn-without-coupling rule remains defined
in [Cross-Project Reference Policy](cross-project-reference-policy.md).

## Terminal and Presentation Documentation

Documentation and support claims keep four layers separate:

1. character encoding or repertoire, such as CP437, PETSCII, or ATASCII;
2. terminal behavior or protocol, such as ANSI, VT100-family, or
   VT52-family behavior;
3. graphics or presentation protocol, such as ANSI art, RIPscrip, or Sixel;
4. visual or font profile, such as IBM PC/VGA, Commodore, Amiga Topaz, Atari,
   ZX Spectrum, or Amstrad CPC styling.

Rendering a font does not establish terminal-protocol or whole-platform
support. PETSCII, ATASCII, Atari modes, Amiga/Topaz, ZX Spectrum, and Amstrad
CPC remain research candidates unless separately implemented and verified.

Sixel has a future home in both manuals and technical reference, but is not
implemented. A future Sysop chapter would explain benefits, enablement,
compatible clients, and fallback. A future technical chapter would define
capability negotiation, resource selection, bounds, security, and fallback.
Sixel remains optional presentation only and cannot alter commands,
authorization, identity, message/file behavior, accounting, or required
workflows.

## Project-Authored Source Headers

Use the following short header on substantive project-authored source files
where the format safely permits comments:

```text
// SPITFIRE NG
// Preservation-driven modern cross-platform reimplementation of
// Buffalo Creek Software's SPITFIRE Bulletin Board System
//
// Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
// Licensed under MIT OR Apache-2.0
//
// This file is part of the SPITFIRE NG project.
// See the repository documentation for architecture, provenance,
// compatibility research, security, and contribution guidelines.
```

Adapt comment syntax to the language. A shebang, encoding declaration,
compiler directive, or format-required preamble remains first. The header does
not replace SPDX or third-party notices where they already apply.

Do not add the header to generated files, lockfiles, vendored or third-party
code, binary fixtures, protocol/test data whose bytes are significant, or
formats where comments are unsupported or alter meaning. Data and localized
resource files use separate manifest-level provenance rather than embedded
comments unless their format and loader explicitly support them.

When a module has a special compatibility boundary, a short note may follow:

```text
// Compatibility note:
// This module is independently authored from published specifications,
// historical documentation, and interoperability testing.
// No proprietary historical source code is incorporated here.
```

Link to the canonical specification or research document. Do not paste an
evidence history into source.

### Copyright years

New project-authored files use their year of first publication. In a later
calendar year, use a range only when a file receives a material copyrightable
revision; do not churn every header annually. Preserve contributor credit and
all third-party notices. Licensing and contribution policy, not a file-header
shortcut, governs ownership.

### Current audit and migration plan

The gate audit found 68 project Rust files and two shell tools, plus
project-authored Ruby and assembly research tools, with no common SPITFIRE NG
header. Workspace licensing is consistently `MIT OR Apache-2.0`, and the MIT
license names Craig Daters and SPITFIRE NG contributors. No conflicting
project-source license header was found.

Header insertion should be a separate bounded implementation pass:

1. classify every candidate as project-authored, generated, third-party,
   data/fixture, or format-sensitive;
2. add the canonical header to substantive Rust and tool sources, preserving
   shebangs and directives;
3. add concise compatibility notes only to independently authored legacy and
   protocol modules that benefit from them;
4. add a repository validator with an explicit, reviewed exclusion list;
5. add it to normal quality gates; and
6. review the diff for line-sensitive scripts, source maps, generated output,
   package metadata, and license attribution.

Do not mechanically add comments to `Cargo.lock`, Fluent resources, binary or
fixed-byte fixtures, generated assets, or historical samples. The architecture
gate made no source-header edits; the reviewed implementation is recorded
below.

### Migration implementation

The 2026 source-header migration applies the canonical block without changing
the source that follows it:

| Reviewed class | Result |
| --- | --- |
| 49 Rust files under public workspace `src/` trees | Project-authored; all require and carry the canonical `//` header. |
| 2 shell release tools | Project-authored; both carry the `#` header after the preserved shebang. |
| 2 existing Ruby tools plus the validator | Project-authored; all carry the `#` header after the shebang and any Ruby magic/SPDX line. |
| Cargo manifests and `Cargo.lock` | Excluded: license-bearing package metadata and generated dependency state. |
| GitHub issue-form YAML | Excluded: repository configuration, not substantive source. |
| Fluent/localization, presentation resources, fixtures, protocol payloads, and other data | Excluded: embedded comments could change parsed or byte-exact content; provenance remains manifest-level. |
| Historical samples and compatibility evidence | Excluded: not project-authored source and never relabeled. |
| Vendored or third-party source | None is tracked in the reviewed source scopes; Cargo dependencies remain external and retain upstream licenses. |

[`tools/verify-source-headers.rb`](../tools/verify-source-headers.rb) validates
the reviewed source scopes, exact canonical block, shebang and Ruby preamble
ordering, and unclassified tracked source-shaped files. Its explicit
non-source and content exclusions prevent manifests, generated dependency
state, resources, release material, or historical samples from gaining a
project ownership notice by inference. A future generated, vendored,
third-party, or historical source file must be explicitly classified before
the validator will accept it.

The existing SPDX notice in `tools/inspect-display-resource.rb` remains ahead
of the project block. Existing module documentation and source attribution are
unchanged. No additional compatibility block was needed: canonical technical
and research documents already preserve protocol and clean-room provenance,
including the independently authored TeLink boundary, without placing private
peer details in source headers.

## Adoption Plan

1. Keep this policy and `docs/README.md` as the architecture and routing
   authority.
2. Author the source-current Quick Start and manual front page first, using
   existing operator guides and verified commands.
3. Establish Caller Guide navigation and extract the first complete caller
   journeys.
4. Add a Technical Reference index that maps, rather than immediately moves,
   existing specifications.
5. Add page applicability labels and stable help-topic IDs.
6. Consolidate one subject at a time; preserve redirects and validate links.
7. Perform the source-header migration as a separately reviewed mechanical
   pass.
8. Generate or synchronize website/manual outputs only under a separate
   publication authorization.

M039 Tranche 7 must apply this policy when it begins: B-017/B-021/B-022 work
will require appropriate Sysop guidance, Technical Reference updates, and
contextual-help planning. This policy does not begin that gate or implement
those rows.
