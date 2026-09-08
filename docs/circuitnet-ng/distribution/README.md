# Reproducible Network Kit build

Canonical member documents are the parent Markdown files. NODE-APPLICATION.txt,
CONFERENCES.md, CONFERENCE-CHANGES.md and JOINING-INFO.md are derived editions.
The signed catalog and immutable predecessor history live in config/. Release
joining/contact fields come from config/release.json. No duplicate hand-edited
Charter or Rules is maintained.

```sh
cargo build -p sf-net --example catalog-artifact --offline
python3 tools/build-circuitnet-kit.py --update-docs --check
python3 -m unittest discover -s tools/tests -p 'test_circuitnet_kit.py'
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
