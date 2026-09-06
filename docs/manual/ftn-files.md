# FileEcho, hatching and FTN file requests

N7 adds FTN file distribution around the ordinary SPITFIRE file catalog. A file
hub stores one native file per mapped area and tracks delivery separately for
each subscribed peer. BinkP remains the shared transport. The
[M051 report](../research/m051-networking-n7-ftn-files.md) records isolated
acceptance; this guide does not enroll a board in a public network.

## Configure FileEcho

1. Create the ordinary native file areas using [file administration](../operator/files.md).
   Choose caller visibility deliberately. FileEcho import follows that native
   area's visibility; incomplete arrivals are never caller-visible.
2. Configure the existing FTN links and [BinkP transport](binkp.md). Do not create
   separate file-only endpoint records. Set the ordinary BinkP authentication.
3. Attach sfconfig to the daemon. In **Networks → FTN file networking**, open
   **FileEcho and FREQ policy**, enable file networking and review finite limits.
   FREQ may stay disabled while FileEcho operates.
4. Use **Add FileEcho area**. Enter domain, tag and native area ID, description
   and permitted directions. A tag maps to one native area; the association is
   retained and cannot be silently remapped. Disable it to stop use.
5. Use **Add file subscription** for each existing link/tag. Inbound admits
   authenticated content from that link; Subscribed selects outbound fanout.
   Missing permission denies. Held keeps future outbound work queued.
6. Select each link's **TIC password (write-only)** entry. Input is masked and
   never shown again. This is separate from its BinkP password. C clears it after
   confirmation. Rotation discards the trust of incomplete TIC controls, so the
   peer may need to resend those controls.

Forms show named fields, validate through typed authority and review changes
before saving. A concurrent edit rejects a stale draft; refresh and review again.
Configuration edits during active BinkP sessions also conflict. Read-only operator
bootstrap has no mutation authority. Offline file-network editing is not offered.

## Hatch an existing native file

Choose **Hatch native file / area / filename / tag** in the same menu. The preview
shows native and transfer names, native area, FileEcho tag, size, CRC, SHA-256,
description and recipient link IDs. Enter queues the preview; Esc cancels.
Publication rechecks file/mapping versions and recipients. A changed preview
must be refreshed. Hatching uses the existing native file and description; it
creates no second stored copy. Import new local files through ordinary native
file maintenance first. There is no network filesystem browser or archive hook.

Safe 8.3 names are used on the wire. Longer native names retain explicit Lfile
metadata and a deterministic safe transfer alias. Reserved TIC/REQ/PKT payload
names cannot be hatched. Re-hatching identical content into the same mapping
rejects its retained duplicate identity; it is not a hidden rescan operation.

Poll existing BinkP links explicitly. Successful payload and TIC acknowledgements
complete only that link's delivery. Another failed link stays pending. Hold a
subscription to stop its exchange while preserving new hatch intent. Release it
and poll when ready. Unsubscribe holds older queued work; inspect/release each
retained pending delivery deliberately if membership is restored.

## Inspect failures and queues

Open **sfmonitor → Networks → 9 Files**. It lists staging counts, mappings,
subscriptions, deliveries and bounded recent file/FREQ activity. Select a delivery
and press Enter for safe detail, including origin, provenance, integrity identity,
acknowledgements, attempts, hold and last semantic error. **6 Quarantine** includes
file rejection categories. No secret or private storage path appears.

Wrong password, unknown/denied area, malformed TIC, unsafe name, wrong CRC or size
never import a native file. Correct the sender or policy and arrange resubmission.
A conflicting occupied pair keeps its original staged bytes; it does not overwrite
them. Incomplete/conflicting pairs expire on a later arrival under the configured
retention policy. There is no reprocess/discard button. If storage is unavailable,
use ordinary native maintenance/recovery, then explicitly release/retry held work.
Do not edit network database rows or turn staging into a native file area.

The bounded history contains safe results, not raw quarantined TICs. Delivery
means the peer acknowledged artifact custody; its own later application policy
can still reject a TIC. Coordinate privately with that operator as appropriate.

## Serve and request FREQ

Enable FREQ in file policy, then **Approve native file for FREQ** for one link
and one native file ID. The exact request name must equal its safe transfer name.
The native area must be public: active, at-least security, read level zero. Only
primary managed storage is eligible. Each link needs its own grant. Disabling a
grant revokes future service and holds unsent responses. No area-wide automatic
exposure, magic alias, wildcard, nodelist publication or command execution exists.

Review maximum files, individual payload bytes and total FREQ bytes. They apply
per request/session; a rejected multi-name request queues no partial response.
Only exact filenames resolve against grants. Paths, private files, noncatalog
files and unapproved names are denied with generic results.

To request a file from a configured peer, choose **Request FTN files**, select its
link, enter the exact transfer filename and choose the destination native area ID.
The typed API also supports a bounded list. Poll to deliver the request, then poll
again for a queued response if necessary. Received bytes import only against the
outstanding link/name receipt. The requester's choice does not grant FileEcho
redistribution. Inspect recent FREQ activity for outcomes; lack of a returned file
is not proof of success. The implemented request convention and independent
coverage limits are described in the [Technical Reference](../technical/ftn-files.md).

## Restart and restore

Graceful shutdown ends live sessions. Interrupted partial wire bytes are discarded;
fully received incomplete pairs, subscriptions and policy survive. Retry begins
at zero for the incomplete artifact, preserving accepted artifacts and recipients.

Use the normal [cold backup/restore workflow](../operator/backup-restore.md).
Backups include native files, adapter database rows and private TIC credentials.
After restore inspect Files, native storage health and recovery holds before
polling. Accepted A remains complete while pending B remains pending. Surviving
later receipt evidence can reconcile an older snapshot. Unprovable pending work
stays held for review; restoration never creates an active transfer session.
Incomplete staged pairs remain incomplete until their matching valid artifact
arrives. Restored hatches reference the same native file, and FREQ sessions are
never resurrected as live connections.

A delivery reaching twelve unsuccessful attempts is held. After correcting the
cause, release its retained delivery in sfconfig to renew the attempt budget and
poll again. The Attempts counter restarts on explicit release; accepted artifacts
remain accepted and are not retransmitted. The release is audited.
