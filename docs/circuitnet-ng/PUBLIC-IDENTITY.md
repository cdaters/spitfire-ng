# Official network identity and publication locations

CircuitNET NG uses circuitnetng.org as its official home.

Applications Closed.
Web, email and download services have not been verified.
These are the intended publication locations; no deployment is claimed.

## Network introduction

https://circuitnetng.org/

## Node application when opened

https://circuitnetng.org/apply

## Current documentation/configuration Network Kit

https://circuitnetng.org/downloads/infopack.zip

## Human catalog authority and verification page

https://circuitnetng.org/network/catalog

## Signed machine catalog

https://circuitnetng.org/catalog/catalog.json

## Signature sidecar

https://circuitnetng.org/catalog/catalog.sig

## Catalog signing public key

https://circuitnetng.org/catalog/catalog-authority.pub

## Public role contacts

Joining: join@circuitnetng.org

Founding Network Administrator: founder@circuitnetng.org

Do not submit applications until opening is announced. Credential enrollment
occurs after membership approval; no application includes authentication secrets.

## Catalog publication contract

The human authority page should show the current revision, catalog body hash,
signing-key fingerprint, publisher, protocol version, publication date and
previous revision, with artifact links and verification instructions.

catalog.json is the unchanged signed wrapper: body, hash and signature.
Its catalog SHA-256 covers the canonical body, not the entire JSON file.
catalog.sig contains the same 64-byte signature as 128 lowercase hexadecimal
characters followed by LF. It is a convenience copy, not a separate signature
of the JSON wrapper. Reject disagreement with the wrapper signature.
catalog-authority.pub contains the 32-byte public key as 64 lowercase
hexadecimal characters followed by LF. It contains no private signing material.

HTTPS discovery does not establish trust in a replacement signing key.
Use an independently accepted authority and verify the catalog signature and
revision chain as described in [VERIFICATION](VERIFICATION.md).
An online page must generate its current values from the actual published
catalog and authority, not copy the values from this older kit indefinitely.

Current kit authority fingerprint (SHA-256 of raw public key):
32062995958587344e9f8f0d76351169a7c9d4e5163bf3fef3972eba64e0a2c2

## Future site sections

Conference information, public node listings, Files, standards and downloads
may receive separate pages. Their publication requires separate work.
The planned public Node Directory is https://circuitnetng.org/nodes.
It will show Node ID, BBS name and role plus only explicitly consented
optional fields, with BBS name/Node ID search and consented location search.
The directory is not deployed; see the application publication choices.
A future independent CircuitNet Technical Standards identity may be considered;
it is not an active organization, site or dependency. This network home remains
the canonical location for both network and technical documentation.
