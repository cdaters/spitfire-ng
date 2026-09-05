# Reading and replying to mail offline

QWK lets you download a group of messages, read and compose replies in an offline
mail reader, then upload your replies on your next call. Use a QWK/QWKE CP437
reader; MultiMail 0.52 has been exercised with current SPITFIRE NG source.

From Messages, choose **L — QWK Mail**. Choose **D** to download, then **New** or
**To You**, and all, one selected, or your queued conferences. Choose a binary
transfer protocol supported by your terminal. Open the downloaded `.QWK` packet
in your offline reader. Only messages you may read online are included.

After successful download, the board asks whether to update your message pointers.
Answer Yes to advance them and mark included addressed mail received. Answer No
for a preview you can download again. Packing mail alone does not mark it read.
A failed transfer leaves the mail available for another attempt. **S** changes
last-read pointers in the selected conferences; lowering one includes older mail
in your next New packet.

Compose replies in the reader and save its `.REP` packet. Return to Messages → L,
choose **U**, select ordinary **Retry**, then upload with a binary protocol. The
board reports imported replies, duplicates, rejections and possible duplicates.
Your authenticated account supplies the author; changing From in a packet cannot
make you another caller. Conference and private-recipient permissions still apply.

Uploading the same packet again normally skips replies already imported. Use the
explicitly confirmed **New submission** option only when you intend to post the
same content again. A possible duplicate in a different packet is held for your
review. Keep your original reply packet until you have checked the result.

“No new messages” means none match your selection and pointers. A stale packet
means mail or access changed; download again. An unavailable or malformed packet
may use an unsupported extension, contain an invalid date, exceed board limits,
or contain damaged archive data. Try a CP437 QWK/QWKE packet without attachments;
ask the Sysop if it still fails. Some characters, including the CP437 pi character,
cannot be represented safely by this profile and hold the packet rather than being
silently changed. A message beginning with a header-like line such as `From:`
can also hold a packet because some readers mistake it for the message author.
Read that message online before deciding whether to change your pointers.
Original LAKOTA pointer files are not supported.

Packets can contain private mail. Protect your local downloads and use the board's
secure connection when available. **?** shows help; **Q** returns to Messages.
QWK here is caller offline mail; it does not connect this board to a mail network.
