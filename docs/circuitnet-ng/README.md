# CircuitNET NG Network Kit 1.0

CircuitNET NG connects independent BBSs through configured ROOT/HOST/END tree
relationships. Its purpose is useful conference discussion, operator cooperation
and BBS preservation. SPITFIRE NG is a reference implementation, not a requirement
that participating software share a database, language or local conference numbers.

This original modern **documentation/configuration infopack** contains Charter 1.0,
Rules 1.0, a signed initial conference catalog, joining and node application materials,
role guides, security/privacy guidance and a safe profile example. It is not a BBS
software binary release, membership approval, live endpoint directory or credentials.
Network Kit version 1.0 is distinct from CIRCUITNET-NG wire protocol 1.4 and catalog
revision 1. Future kits may contain later signed catalogs without changing wire major.

[Charter](CHARTER.md), [Rules](RULES.md), [Conferences](CONFERENCES.md),
[Changes](CONFERENCE-CHANGES.md), [Joining](JOINING.md),
[Application fields](APPLICATION-FIELDS.md), [plain application](NODE-APPLICATION.txt),
[END](END-NODE.md), [HOST](HOST-NODE.md), [ROOT](ROOT-NODE.md),
[Security](SECURITY.md), [Protocol](PROTOCOL.md),
[Catalog administration](CATALOG-ADMIN.md), [Notice](NOTICE.md).

The machine authority is [the signed catalog](config/catalog.json), verified against
the independently confirmed [authority pin](config/catalog-authority.json). Human
conference and change documents are generated from it. The
[46-area review](config/catalog-review.json) records each C5 candidate's disposition.
Earlier C5 proposal JSON remains separately labeled proposal-only; neither it nor
historical records override the signed current catalog. Catalogs are living resources,
not a hard-coded fixed list, and presence does not create local mappings/subscriptions.

Build with `python3 tools/build-circuitnet-kit.py --output dist` from the repository
root after building the catalog artifact validator. See the distribution source
outline for validation commands. The generated ZIP includes Markdown and 80-column
plain-text forms plus a deterministic size/SHA-256 manifest. Generated archives are
not committed. No proprietary historical material or private acceptance data belongs
in this kit. A future explicitly configured CircuitNET Files area may distribute
approved kit revisions; this package enables no such traffic automatically.
