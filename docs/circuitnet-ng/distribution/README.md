# Reproducible Network Kit build

Canonical member documents are the parent Markdown files. NODE-APPLICATION.txt,
CONFERENCES.md, CONFERENCE-CHANGES.md and JOINING-INFO.md are derived editions.
The signed catalog and immutable predecessor history live in config/. Release
joining/contact fields come from config/release.json. No duplicate hand-edited
Charter or Rules is maintained.

```sh
cargo build -p sf-net --example catalog-artifact --offline
python3 tools/build-circuitnet-kit.py --update-docs --check
python3 -m unittest discover -s tools/tests -p 'test_circuitnet*.py'
python3 tools/build-circuitnet-kit.py --output dist/network-kit-1.0
```

The release builder verifies signatures and consecutive hashes before generating
anything. `--check` checks generated sources; `--update-docs` refreshes them. An
ordinary build rejects stale generated files and an existing output directory.
Set `--source-commit` to the exact PUBLIC source commit for the final release build.
Without it, RELEASE.TXT explicitly says candidate and identifies source contents
by SHA-256. A private checkpoint is never guessed from the current checkout.

The ZIP has literal root README.TXT and FILE_ID.DIZ. Root BBS editions are ASCII,
CRLF and at most 79 columns. The purpose-built renderer preserves paragraphs,
lists, literal commands/topology and link destinations; tables become labeled
records. CONFS is generated as conference entries, never flattened table text.
Rich Markdown lives under markdown/, protocol material under technical/, public
catalog/config artifacts under config/. The compatibility source application uses
repository LF; delivered BBS text always uses CRLF.

MANIFEST.json lists payload names, sizes and SHA-256, excluding the two manifests.
MANIFEST.sha256 additionally covers MANIFEST.json, excluding only itself. A ZIP
checksum sidecar is generated outside the archive. Sorted entries, fixed ZIP
metadata and unchanged source inputs reproduce identical bytes. No wall-clock
build timestamp enters the package. The revised build supersedes the earlier
pre-release candidate; do not offer two different artifacts as the current 1.0.

The allowlist excludes historical proprietary material, private research/acceptance
files, applications, credentials and private keys. A public authority is not a secret.
The manifest is an integrity inventory, not a substitute for independently verified
publication provenance and catalog authority. See the included verification guide.

Current history uses the founding authority. A future release after key replacement
must add retained public transitions and epoch-aware build validation; this builder
fails signature verification rather than presenting old history as signed by a new
key. Existing nodes already have the usable explicit replacement/recovery procedure
in KEY-CUSTODY.md. Automatic trust replacement from a kit is never permitted.

The builder starts no daemon, Event or network transfer. Future distribution through
CircuitNET Files requires deliberate area/subscription configuration and governance.

## Public identity authority

config/release.json is the only hand-edited public network identity. The validated
PublicIdentity model combines its domain, role local-parts and endpoint paths into
HTTPS URLs and role addresses. Generated sections in member Markdown, joining
information, PUBLIC-IDENTITY.md, the planning worksheet and release provenance use
that projection. Edit the metadata then run --update-docs --check; ordinary builds
reject stale derived content. Marker comments delimit generated source sections and
are omitted from delivered editions. No personal contact is inferred from Git.

Applications remain closed independently of domain ownership. Service status is
not-verified until a separate deployment operation is explicitly performed; the
builder rejects opening applications without an explicit verified-open status.
This is an operator release declaration, not a reachability probe. Building starts
no DNS, web, mail or network enrollment work. Network Kit 1.0 revised build 4
supersedes build 3 as the pre-opening candidate.

The kit includes a derived catalog.sig sidecar copied from the accepted signed
wrapper. Signature verification still uses the unchanged catalog and pinned key;
no new signature/key is created. Domain-change fixture tests rebuild the actual
archive and verify that all generated URLs and role addresses move together.


Addressing metadata is separately versioned in config/regions.json. The read-only
assignment helper is allowlisted into technical/; no membership reservation list
is packaged. ADDRESSING, DOSSIERS and SYSOPACC root editions derive from their
canonical Markdown. Required and operator-only conference groups derive from the
unchanged signed catalog, not a second hand-maintained policy list.
