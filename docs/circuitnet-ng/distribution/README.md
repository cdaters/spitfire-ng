# Reproducible Network Kit source outline

Canonical human sources are the parent Markdown files and NODE-APPLICATION.txt.
Canonical machine authority is config/catalog.json plus its independently enrolled
catalog-authority.json. The review/disposition artifact is explanatory metadata.
No duplicate hand-edited Charter or Rules lives in this directory.

From the repository root:

```sh
cargo build -p sf-net --example catalog-artifact --offline
python3 tools/build-circuitnet-kit.py --output dist --check
python3 tools/build-circuitnet-kit.py --output dist
```

The builder verifies catalog signatures with the native artifact utility, checks the
revision/hash chain, generates CONFERENCES.md and CONFERENCE-CHANGES.md, and copies
only its explicit rights-safe allowlist. `--check` verifies checked-in generated docs
without changing them. `--update-docs` regenerates those two canonical derived docs.

Output is `circuitnet-ng-network-kit-1.0.zip` and an expanded tree of the same name.
It contains introduction, Charter/Rules, conferences/changes, joining/application,
END/HOST/ROOT, catalog administration, security, protocol/specification, NOTICE and
licenses; machine catalog, authority pin, review and safe example profile; 80-column
plain-text human forms; and MANIFEST.json listing every other file's size/SHA-256.
The manifest excludes itself to avoid a recursive self-hash. Sorted paths, fixed ZIP
timestamps/permissions and stored entries make identical input produce identical
archive bytes. Generated archives remain outside version control.

No raw historical corpus, private report, keys, real server configuration, acceptance
artifact, application submission or software binary is allowed. Future CircuitNET
Files distribution requires deliberate mapping/subscription/governance configuration;
the builder starts no daemon, Event or network transfer.
