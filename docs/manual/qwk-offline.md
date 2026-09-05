# QWK offline mail

Current source offers QWK offline mail under **Messages → L**. Callers download
messages, read/reply in a QWK/QWKE CP437 reader, and upload replies. See the
[Caller Guide](../caller-guide/qwk-offline.md) for the complete procedure.
QWK networking, DOVE-Net and FidoNet/BinkP are not implemented. The downloadable
0.1.0 Development Preview is older than this source feature.

New setup creates a stable QWK board ID. For an existing board, stop the daemon,
back up the board and add `qwk_board_id = "MYBOARD"` under the existing `[caller]`
section in its configuration. Use a unique, stable one-to-eight-character uppercase
letter/digit ID; device names are rejected. Restart normally. Omission disables
packet creation/import. Do not change the ID casually: readers and pending replies
identify your board with it. No Networks page is added to sfconfig.

Conference access, posting security and private-message rules remain authoritative.
QWK does not bypass online permissions. Callers explicitly confirm pointer updates
after successful downloads. Failed transfers do not consume new mail. Repeated
ordinary reply uploads are safe; explicit New submission permits an intentional
repeated post. Malformed framing rejects a packet; valid packets can have both
imported and rejected replies. The caller receives counts, not debugging output.

Use existing Activity and Errors views for QWK outcomes. Logs do not include packet
contents, private recipients or message bodies. Retained packet evidence is private
board data under SYSTEM and participates in normal cold backup/restore. Protect
backups as carefully as the live board. Incomplete transfers are not public file
uploads and do not affect caller download ratios.

A capacity error holds intake without deleting history. Retained artifacts have a
1 GiB budget; import history is also bounded. Do not delete registry files or edit
SQLite to make space. A custody error requires restoring a consistent board backup
or developer-assisted recovery. Startup cleans only writes explicitly journaled as
incomplete, preserves unknown files, and verifies retained evidence. See the
[Technical Reference](../technical/qwk-offline.md) for the exact limits and recovery
contract.

For malformed replies, confirm that the reader uses ZIP QWK/QWKE CP437, the board
ID matches, and no attachments or unsupported pointer/routing extensions are in
the packet. Leading header-like body lines can also hold an export to prevent
reader misattribution; native content and pointers remain intact. MultiMail 0.52 is an exercised independent peer. Original LAKOTA LMR
pointer interoperability remains evidence-qualified. Windows live QWK acceptance
is deferred until a real Windows environment is available; no Windows interactive
compatibility claim is made by the macOS acceptance.
