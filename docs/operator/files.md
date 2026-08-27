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
created without overwrite, cataloged transactionally, and counted once.
Canceled, disconnected, invalid, duplicate, or failed uploads do not become
available files.

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

SQLite stores file-area policy, catalog metadata, SHA-256, attribution,
availability, and counters. File bytes remain under the configured external
storage root. Every download reauthorizes the caller and revalidates the byte
size/hash before transfer. Do not place a file into that directory manually;
uncataloged bytes are neither caller-visible nor native-backup content.

Archive inspection/import, `FILE_ID.DIZ`, ratios, batch-queue redesign,
delete/relocate maintenance, and file networking are not part of the current
operator surface. See [Native SPITFIRE NG File System](../sfng-file-system.md).
