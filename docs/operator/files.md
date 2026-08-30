# Files

## Configure file areas

Stop the board and choose section 6 in `spitfire config`. File-area changes
are immediate. Creating an area creates its confined storage directory;
disabling an area preserves catalog metadata and file bytes.

Setup creates General Files and SPITFIRE Files with one small generated
starter file in each. These are SPITFIRE NG files, not historical assets.

## File menu

Enter Main `F`, or Message `F`:

| Command | Current behavior |
|---|---|
| `C` | List accessible areas and change the current area. |
| `L` | List files in the current area with size, board-local date, and description. |
| `R` | Read a bounded text file safely; binary and terminal-control content is rejected. |
| `V` | View bounded Stored/Deflated ZIP member metadata without extracting the archive; unsupported or inconsistent archives fail closed. |
| `D` | Download one file or a comma-separated batch through a selected protocol. |
| `U` | Upload through the selected protocol and catalog successful bytes. |
| `N` | Find new files by checkpoint or entered date. |
| `T` | Search descriptions for one to six words. |
| `F` | Find filenames using `*` and `?`. |
| `M` | Move directly to Messages. |
| `Q` | Return to Main. |
| `G`, `X`, `?` | Goodbye, expert mode, and contextual help. |

Listings use the stock-oriented filename/size/date/description columns and
show wrapped continuation descriptions without truncating longer safe modern
filenames.

## Add a downloadable file

There is no host-side import/catalog command. Use the same authenticated
upload path callers use:

1. Log in as an account authorized to upload to the area.
2. Enter Files and select the destination area.
3. Select `U`.
4. Supply the safe basename and caller-visible description.
5. Select a protocol and start the matching client send operation when
   prompted.
6. List the area and confirm filename, size, date, and description.

Successful uploads are staged per session, independently sized/hashed,
name-reserved without overwrite, operation-journaled, cataloged, and counted once.
Canceled, disconnected, invalid, duplicate, or failed uploads do not become
available files.

A description beginning with `/` invokes the historical Sysop-only signal.
Current source stores the upload as `PendingReview`; it is hidden from ordinary
listing, search, read/view, request, and download until an authorized operator
accepts it through the typed maintenance domain. Operator TUI/CLI presentation
for that domain is not part of this tranche. An active threshold Sysop may use
the bounded archive/DIZ inspection service during review without publishing the
item to callers.

## Search and new-file checkpoint

Filename matching is case-insensitive ASCII. If an extension is omitted,
SPITFIRE NG adds `.*`; the unhelpful `*.*` query is rejected. Description
search requires every supplied word.

New Files can scan all accessible areas, the current area, or one accessible
area. It accepts the caller's last successful checkpoint or a real
`MM-DD-YY`/`MM-DD-YYYY` board-local date. A completed result—even empty—moves
the checkpoint forward. Cancellation, invalid input, failure, or paging abort
does not.

## Storage boundary

SQLite schema 15 stores file-area policy, stable identities, lifecycle and
integrity, private requests/review, upload policy, versions, operation journal,
legacy publication state, audit, SHA-256, attribution, and counters. File bytes remain under the configured external
storage root. Every download reauthorizes the caller and revalidates the byte
size/hash before transfer. Do not place a file into that directory manually;
uncataloged bytes are neither caller-visible nor native-backup content.

The native maintenance domain now implements bounded FILE_ID.DIZ review,
private requests, move/tombstone/reconcile, denial policy, and legacy-listing
publication. A future sfconfig/sfmonitor/CLI must call these typed, versioned
commands; there is intentionally no direct online SQLite or managed-root edit.
Ratios, batch-queue redesign, file networking, persisted extended roots, and
the broader enhanced file UI remain outside this tranche. See the
[implementation report](../research/m039-tranche-5-safe-file-inspection-request-maintenance-implementation.md)
and [verification report](../research/m039-tranche-5-verification.md).
See [Native SPITFIRE NG File System](../sfng-file-system.md).
