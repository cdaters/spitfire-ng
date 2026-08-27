# Messages

## Configure conferences

Stop the board and choose section 5 in `spitfire config`. Conference changes
are immediate. Conference 1 is mandatory because Comment to Sysop uses it.

A new board has General and SPITFIRE conferences but no starter message. The
first practical operator task is to log in as the Sysop caller and post an All
Callers welcome message.

## Message menu

Enter Main `M`:

| Command | Current behavior |
|---|---|
| `C` | List accessible conferences and unread counts; change conference. |
| `R` | Read This, All, or Only Queued conferences. |
| `B` | Browse visible message headers. |
| `E` | Enter a public or private local message. |
| `Y` | Show received/sent/available counts and open received/sent lists. |
| `A` | Add, remove, list, or reset the caller's conference queue. |
| `F` | Move directly to Files. |
| `Q` | Return to Main. |
| `G` | Log off. |
| `X` | Toggle session-local expert mode. |
| `?` | Contextual help. |

Commands are filtered by caller security and the supplied `SFMSG.MNU`.

## Post a message

1. Select `E`.
2. Enter a known local caller name, or press Enter for All Callers.
3. Enter a subject.
4. Type one line at a time.
5. Enter a blank line to open the editor command menu.
6. Select `S`, then confirm with `Y`.

The line editor supports Save, Edit, Abort, Continue, Begin Again, Replace
Line, List, Insert Line, and Delete Line(s). `/S` and `/A` are direct save and
abort equivalents. A disconnect or abort before confirmed save leaves no
partial message.

A non-public message is visible only to its author, intended recipient, or a
currently authorized Sysop. The author identity always comes from the logged-
in caller.

## Read and reply

`R` opens the conference scan menu:

- This Message Conference scans the current conference;
- All Message Conferences scans every accessible conference; and
- Only Queued Conferences scans Conference 1 plus the caller's selected
  optional conferences.

The reader supports next, previous, direct message number, reply, stock same-
subject thread traversal, and quit. A reply may retain or change the subject.
CTRL-Q in reply composition reviews the original and imports a bounded line
range with sender-initial prefixes; imported quote lines are immutable.

Normal reading advances that caller's last-read pointer and records a direct
receipt idempotently. Reconnects and rescans do not double-count it.

## Your Messages

Message `Y` reports:

- new directly addressed messages;
- already received directly addressed messages;
- messages sent; and
- total messages visible to the caller.

Choose received or sent presentation, then enter the displayed
`conference/message` pair to open a body. Public All Callers messages appear
in the sender's sent list but are not direct received mail for every caller.

## Comment to Sysop

Main `C` creates a private Conference 1 message addressed to the configured
Sysop caller. It is durable mail, not the interactive Page the Sysop/chat
workflow. The configured Sysop caller account must exist.

## Current boundary

Local conferences, queues, privacy, receipts, replies, threads, and Your
Messages are implemented and verified. QWK/LAKOTA, network mail, broad search,
copy/move/forward, deletion/packing, and complete maintenance are later
scopes. See [Native SPITFIRE NG Message System](../sfng-message-system.md).
