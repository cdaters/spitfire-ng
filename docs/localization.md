# SPITFIRE NG Localization Contract

This is the canonical format/API 1 specification for engine localization. It
was implemented by M037.2 before Development Preview packaging. Its purpose is
to keep three independent authorities explicit:

```text
engine behavior + presentation profile + language package = caller/Sysop UI
```

Profiles choose visual resources. Language packages supply engine-owned prose.
Neither can authenticate a caller, grant security, dispatch a command, alter
storage, or reorder a workflow.

## Package layout and identity

Boards store language packages below `SYSTEM/language-packs/<locale>/`:

```text
en-US/
├── language.toml
├── messages/
│   ├── shared.ftl
│   ├── caller.ftl
│   └── operator.ftl
├── README.md
└── LICENSES/
    └── ASSET-LICENSE.txt
```

`<locale>` is a canonical BCP 47 identifier. Underscores and noncanonical
directory spellings are rejected. Manifest format 1 records `format_version`,
`locale`, semantic `package_version`, `catalog_api_version`, a half-open engine
version range, `fallback_locale`, supported terminal encodings, provenance,
and an exact file inventory with byte lengths and SHA-256 hashes. Unknown TOML
fields, undeclared or duplicate files, missing required catalogs, invalid
hashes, unsafe paths, symlinks, special files, excessive sizes/counts,
incompatible engines, malformed UTF-8/Fluent, incomplete provenance, or
non-redistributable records fail validation.

The canonical `en-US` 1.5.0 baseline is project-authored and embedded for
recovery. Normal setup also installs an independently validated board-local
copy. It contains 484 semantic messages, including the complete schema-14
public-information catalog, across shared, caller, and operator messages. The
embedded recovery copy does not make English a profile
requirement and does not authorize a package to omit the en-US baseline.
Post-0.1.0 catalogs add localized message discovery and mutation, caller
lifecycle, named-Sysop protection, base/effective security, subscription,
JOKER denial, and local-operator outcomes. The independently authored Modern
and Minimal profiles are version 1.2.0 and Classic is version 1.3.0 with
LOCKOUT, SUBWARN, and SFSUBCHG presentation boundaries.
M042.5 advances only en-US to 1.4.0 for SSH listener/status and durable caller
identity administration text; presentation-profile packages are unchanged.
M043 advances en-US to 1.5.0 for directory privacy, locate, Other BBS,
bulletin/newsletter/system-information/thought, validation, denial, and
conflict states. Modern and Minimal advance to 1.3.0 and Classic to 1.4.0 for
their project-authored Main-menu resource mappings; behavior and visibility
remain engine-owned.
Original project-authored catalog/package
bytes remain `MIT OR Apache-2.0`. External language packages retain their own
compatible package-level license and provenance.

## Why Fluent

Fluent provides UTF-8 source catalogs, stable semantic identifiers, named
arguments, plural/select expressions, and syntax validation without embedding
behavior in translations. SPITFIRE NG uses concurrent read-only Fluent bundles
and disables isolation marks because output targets terminal byte streams.
Catalog API 1 permits messages with values; Fluent terms and attributes are
reserved until a later API explicitly defines them.

Keys describe meaning (`caller-auth-password-prompt`), not current wording.
Command letters, `.MNU` identifiers, security values, protocol/profile/locale
IDs, configuration tokens, paths, database/log keys, hashes, and transport IDs
remain invariant. Named runtime arguments are typed as text, signed/unsigned
numbers, or timestamps. The catalog controls grammatical selection; the
baseline call-count message proves one/other plural behavior. Translations are
data and cannot execute code.

Generated-menu labels are selected by stable `.MNU` semantic identifier. The
catalog supplies only the localized title/label meaning; the renderer owns the
invariant command token, calculated leaders, cell width, gutter, and action
distribution. At 80 columns, a label must fit one 30-character encoded cell
with its token and at least one leader. If any label cannot fit safely, the
whole menu uses the bounded single-column form rather than truncating,
wrapping, or corrupting output. Encoding changes glyph representation;
truthful terminal dimensions determine geometry and caller security determines
only the visible authorized action set.

## Selection and fallback

New and fixture boards write:

```toml
[language]
default_locale = "en-US"
```

Older configuration without this table reads as `en-US`; a subsequent normal
save makes the selection explicit. M037.2 implements board default plus an
internal per-session resolver. A future caller preference can select a locale
independently of profile, ANSI/text preference, graphics, and encoding without
changing this format or requiring profile/language combinations.

Resolution is deterministic: requested locale, meaningful BCP 47 parent,
board default and its parent, each loaded package's declared fallback, embedded
en-US, then tiny emergency ASCII. A missing or invalid candidate is bounded in
status diagnostics; no raw key is shown to a caller. `es-MX -> es` uses parsed
language identity, not manual string chopping. Cycles and duplicates collapse
through the candidate set.

Before a board exists, `spitfire --locale <tag> ...` has first priority,
followed by normalized `LC_ALL`, `LC_MESSAGES`, or `LANG` when supported, then
embedded en-US. M037.2 ships only en-US, so an explicitly unsupported bootstrap
locale fails with a recovery diagnostic instead of silently changing language.

## Formatting and terminal output

Authoritative timestamps stay full-year Unix/SQLite values in the configured
IANA board timezone. The localization edge formats them; en-US API 1 uses
`MM/DD/YYYY h:mm AM/PM zone`. Fluent performs locale plural/number selection.
Classic resource-authored date styling remains presentation content and does
not force an engine locale.

The output pipeline is localized Unicode -> negotiated/requested terminal
encoding -> strict conversion. UTF-8 is emitted unchanged. CP437 uses an exact
Unicode-to-code-page table. ASCII accepts ASCII only. If a translation cannot
be represented, the same semantic key falls back to representable embedded
en-US, then emergency ASCII; bytes are never reinterpreted and mojibake is not
accepted. Raw `.BBS`, `.CLR`, fixed-record HLP, future `.RIP`, and custom art
remain presentation bytes and are never silently translated.

## Installation, diagnostics, backup, and security

Use the public cold-board workflow:

```sh
spitfire language-validate /path/to/packs/es-ES
spitfire language-install /path/to/board/spitfire.toml /path/to/packs/es-ES
spitfire config /path/to/board/spitfire.toml
spitfire status /path/to/board/spitfire.toml
```

Installation validates the source, copies only declared regular files through
a board-local staging directory, validates the staged copy, and atomically
installs a locale without replacing an existing package. `status` reports
default/effective locale, package version, READY/DEGRADED, and bounded issues.
Native cold backup already preserves the exact configuration and recursive
SYSTEM authority; restore therefore preserves and revalidates package bytes,
licenses/provenance, and locale selection.

Replacing the `spitfire` executable does not silently replace a board's
installed language package. The current public installer also refuses to
replace an existing locale. A future language-package update must therefore be
an explicit, separately validated operation rather than a side effect of an
executable upgrade.

Language packages are untrusted. They receive named display values only and
cannot supply SQL, shell input, file paths, commands, authorization, or mutable
state. Session-local resolver scopes are thread-local and restored on exit, so
one node's locale/fallback cannot affect another.

## Translation workflow

1. Copy the `en-US` package outside a live board and rename its directory and
   manifest locale to the canonical target BCP 47 identifier.
2. Translate Fluent values without changing semantic keys or argument names.
3. Record creator, rightsholder, evidence/source, classification, license,
   redistribution permission, and modifications for every declared file.
4. Regenerate exact byte lengths and SHA-256 hashes in `language.toml`.
5. Run `language-validate`, then tests with expanded/Unicode strings and all
   intended UTF-8, CP437, and ASCII clients.
6. Install only on a stopped board, select through `spitfire config`, and check
   `spitfire status` before starting listeners.

Non-English packs may be partial only through documented fallback. The en-US
baseline must exactly match the embedded API key set. The test-only `en-XA`
pseudo-localizer expands vowels, adds Unicode delimiters, and proves layout,
fallback, and per-thread isolation; it is not installed or shipped as a normal
package. A bounded source guard rejects new direct caller-facing byte/string
writes in the shared session/message/file engines. Structured logs and
support-oriented human logs remain stable English; directly interactive
caller/Sysop text belongs in catalogs. The same guard covers direct interactive
setup, configuration, status, and operator-console writes; structured support
errors remain on the documented diagnostic side of the boundary.

## Deferred work

Production translations, a caller locale selector/persistence field,
translator community workflow and language-specific client QA remain future
work. Localization does not implement RIP or a separate engine/profile per
language.

Current source implements caller-access keys and resources. Semantic catalog
entries cover bounded caller find/add, lifecycle confirmation,
named-Sysop protection, stale conflict, base-security/subscription changes,
renewal, and privacy-safe denial. Stock-style presentation additionally uses
`LOCKOUT`, `SUBWARN`, and `SFSUBCHG` BBS/CLR resources through the existing
bounded resolver. Lifecycle values, audit codes, stored ISO dates, command
tokens, and `JOKER.DAT` remain invariant and unlocalized. See
[Caller Access Lifecycle and Security](sfng-caller-access.md).
