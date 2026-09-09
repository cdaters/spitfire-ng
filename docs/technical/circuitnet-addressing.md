# CircuitNET node assignment convention

CIRCUITNET-NG remains 1.4; schema remains 36. This is administrative assignment
policy, not a wire identity migration. The existing NodeId parser accepts one to
eight ASCII alphanumeric characters, normalized uppercase. Existing assignments
and protocol fixtures remain valid. Topology has explicit role and parent fields;
no routing code infers them from geography or numeric ranges.

## Versioned inputs and helper interface

[regions.json](../circuitnet-ng/config/regions.json) version 1 is the initial
network-maintained mapping. CC follows
[ISO country-code authority](https://www.iso.org/iso-3166-country-codes.html).
RR is explicitly CircuitNET metadata, mapped to a full named subdivision;
[ISO's glossary](https://www.iso.org/glossary-for-iso-3166.html) distinguishes
country and subdivision codes. The small table is an original selection of factual
identifiers, not a copy of the standards text or a comprehensive licensed database.
AU abbreviations follow named subdivisions (NSW/VIC/QLD), with network NS/VI/QL;
UK mappings use the [government's published identifiers](https://www.gov.uk/government/publications/open-standards-for-government/country-codes).

The Network Administrator/Secretary approves additions with a decision reference,
unique CC+RR, full name and formal subdivision reference; increments table version;
and distributes the changed table. Never silently change an assigned RR's meaning.
Countries/subdivisions not in the initial table are unsupported until added, not
invalid geographic locations. The helper does not download or guess missing codes.

`tools/circuitnet-addressing.py` is also packaged in technical/. Python 3 is needed;
no Rust compilation is involved. It reads region metadata and a private reservation
snapshot, validates every row, and prints a suggestion. It never writes the ledger,
changes a board, contacts a service or grants uniqueness across stale snapshots.
The Administrator must serialize decisions in one authoritative membership record,
recheck before approval and record the reservation there. Concurrent suggestions
alone are not safe assignments. OS access to those records remains administrative.

Reservation snapshot JSON is an array with exactly these fields per row:

```json
[
  {"node":"USAZ000","role":"HOST","status":"active","legacy":false},
  {"node":"USAZ001","role":"END","status":"retired","legacy":false},
  {"node":"ROOT","role":"ROOT","status":"active","legacy":true}
]
```

Status is reserved, active or retired. All statuses occupy the name; normalized
duplicates reject, even for different statuses. `legacy` must be explicit Boolean
and preserves a previously approved non-geographic assignment. It cannot make
CNETROOT an END or HOST. This snapshot contains no contacts, credentials or private
keys and is not itself a distributed registry. Keep the actual membership record,
approval references and backups separately; the kit ships no real reservations.

From the kit root, with your protected snapshot filename substituted:

```
python3 technical/circuitnet-addressing.py --regions config/regions.json \
  --reservations assignments.json --country "United States" \
  --region Arizona --kind END
```

The result suggests USAZ002 for the example above and explicitly says assigned=false.
Kinds are END, PRIMARY-HOST, ADDITIONAL-HOST and ROOT. ROOT suggests CNETROOT without
country/region, but rejects a second active ROOT, including an existing legacy
ROOT assignment. Optional `--requested USAZ017` validates location, kind and vacancy.
Each input is bounded to 2 MiB, tables to 4,096 rows and reservations to 10,000 rows.
Invalid names, malformed rows, duplicates, mismatched role/range and exhausted
ranges fail closed. Existing wire parser tests establish legacy compatibility.

## Migration review and limits

Node ID currently participates in all of the following authorities:

| Component | Existing meaning and consequence |
| --- | --- |
| Profile/topology | Local ID, parents, routes and neighbor admission; retained history freezes identity/tree. |
| TLS | A unique enrolled peer certificate binds to a configured Node ID; no geographic inference. |
| Messages | Origin-scoped message ID, origin, path, ingress and destination retain addresses. |
| Dossiers/controls | Network/neighbor/codename keys, requester/target IDs, request receipts and hashes. |
| Files | Publication origin/path, ingress, per-neighbor queues/receipts retain original Node IDs. |
| Catalog | Authority publisher must be a ROOT in the configured tree; signed history pins publisher. |
| Events | CircuitNET action has an optional explicit Node ID target; no address-derived schedule. |
| Backup/audit | Native snapshot restores these keys and histories; safe audits retain operational IDs. |

Native database foreign keys join profile network, publication identity and queue
IDs; neighbor/origin text is not a foreign key to a generation-aware node registry.
Adding only a UUID column would not repair the old signed/wire provenance. There
is no hidden immutable node identity today and none is introduced here. Doing so
requires a separately reviewed migration covering all rows above and old peers.

A future move should generate an administrative continuity identity automatically,
retain previous/current addresses and effective dates, reenroll certificates,
and preserve historical wire addresses. Reactivation means the same BBS; deliberate
reuse means a distinct generation with privileged approval, rationale and audit.
Neither can be simulated by changing an old database Node ID. Current retired IDs
remain reserved until that migration exists. Same-BBS recovery from its retained
state remains possible after operator review; do not reset replay history.

Even a parent change is currently rejected once catalog/traffic history exists.
The conceptual address need not change with parent, but this release provides no
live migration procedure. The kit's signed authority remains publisher ROOT.
CNETROOT is reserved in new assignment validation only; changing the seed publisher
or an established ROOT is outside this pass and must not be implied by examples.
