# Verify the Network Kit

<!-- public-identity:start -->
Catalog authority and verification information: https://circuitnetng.org/network/catalog

Catalog JSON: https://circuitnetng.org/catalog/catalog.json

Catalog signature: https://circuitnetng.org/catalog/catalog.sig

Catalog public key: https://circuitnetng.org/catalog/catalog-authority.pub

Current accepted signing-key fingerprint (SHA-256 of raw public key):
32062995958587344e9f8f0d76351169a7c9d4e5163bf3fef3972eba64e0a2c2

HTTPS provides delivery and discovery, not independent signing authority.
Confirm the fingerprint through approved enrollment or an already trusted
authority. Verify the signed catalog and retained revision chain.
These publication locations are recorded; endpoint service is not verified.
<!-- public-identity:end -->

This is a documentation/configuration kit, not a software binary release.
RELEASE.TXT identifies its version, build revision, source repository, source
reference where supplied, source-content digest and catalog authority fingerprint.
Fixed archive timestamps make identical source inputs reproducible; they are not
claims about when a file was originally written.

## Check the downloaded bytes

Obtain the ZIP and its neighboring .sha256 checksum from the published release.
Compare the archive checksum before extraction, for example with `shasum -a 256`
on macOS or `sha256sum` on systems providing that command. The checksum is outside
the ZIP because a ZIP cannot meaningfully contain its own final hash.

In the extracted directory, verify all manifest entries:

```
shasum -a 256 -c MANIFEST.sha256
```

On systems using sha256sum, use `sha256sum -c MANIFEST.sha256` instead.
MANIFEST.json additionally records payload sizes and the exact expected inventory.
It excludes the two manifest files to avoid self-reference; MANIFEST.sha256 also
covers MANIFEST.json and excludes only itself. Unexpected extra files are not
part of the kit. A manifest detects changed bytes but does not prove who supplied
it; obtain the release reference and authority fingerprint independently.

## Verify the catalog authority

Compare the catalog authority fingerprint with the value supplied through your
approved enrollment contact, not just another file in this ZIP. The fingerprint
is SHA-256 of the raw public key. config/catalog-authority.pub contains that public
key in hexadecimal; config/catalog-authority.json binds it to the network/publisher.
No private signing key belongs in this package.

The offline catalog-artifact utility described in [KEY-CUSTODY](KEY-CUSTODY.md)
verifies the signature without a board or network call:

```
catalog-artifact validate config/catalog-history/000001.json \
  config/catalog-authority.json
catalog-artifact validate config/catalog.json \
  config/catalog-authority.json
```

Signature checks alone do not prove a consecutive history. Board catalog import
checks revision order and previous hashes; follow [END-NODE](END-NODE.md).
Existing nodes also retain their accepted revision and reject rollback or forks.
Do not bypass a failed check by editing a pin, removing history or trusting a key
included in the failed object.

## Example configuration

config/network-profile.example.json is a planning worksheet, not a directly
importable SPITFIRE NG board configuration. Paths inside it are relative to its
own config/ directory. Its joining_document resolves to markdown/JOINING.md.
Node, parent, endpoint and listener fields require approved enrollment values.
The kit prescribes no public connection endpoint or official port.
