# Native SPITFIRE NG Message System

This is the canonical implementation and compatibility specification for the
native message subsystem introduced in Stock SPITFIRE 3.7 Core Parity
Increment 3 and completed through the 2026-08-22 Category-A message-
interaction closure. It explains what is implemented, which stock behaviors
were established from Buffalo Creek's SPITFIRE 3.7 manual, how authorization
and persistence work, and which historical features remain incomplete.

The broader multi-backend direction remains in [Message System
Design](07-message-system.md). Exact historical file findings remain in
[Legacy Data and File Formats](06-legacy-file-formats.md) and the
[Historical SPITFIRE Overview](HISTORICAL-SPITFIRE.md).

## Original SPITFIRE Behavior Established

The primary operational source is the preserved, ignored
`research/samples/shareware-software/sf37-2/spitfire.doc`, especially its
conference configuration and Message Menu sections.

Confirmed stock behavior relevant to this increment includes:

- SPITFIRE supports numbered message conferences; version 3.7 documents up to
  784.
- A conference has a description, read security, entry/post security, an
  equal-or-greater versus exact-level access mode, a public-only option, a
  25–99-line message limit, and up to five privileged security levels.
- The Message Menu provides conference change, read, browse, enter, reply,
  “Your Messages,” File/Main transitions, help, and logoff paths.
- A local addressee must be a known caller. An empty addressee means “All
  Callers.” A caller cannot choose the persisted author identity.
- Stock “non-public” messages are visible to their sender and recipient;
  unrelated callers must not receive their content. The Sysop has privileged
  visibility.
- Main-menu Comment to Sysop creates a non-public message addressed to the
  configured Sysop caller in Conference 1. This is durable mail, not the
  interactive page/chat feature.
- Replies preserve the subject relationship in the caller experience.
  Stock thread traversal groups visible messages with the same exact subject.
- Reply CTRL+Q redisplays the original, accepts a bounded line range, and
  inserts the original sender's initials before each quoted line. Quoted lines
  cannot be edited.
- Each caller has a message-conference queue. Conference 1 cannot be removed;
  queued scans ignore optional conferences not selected by that caller, and
  addressed mail cannot be left in an optional conference absent from the
  recipient's queue.
- “Your Messages” reports new directly addressed messages, already received
  messages, messages sent, and total visible messages. A named Sysop may
  preview without marking directly addressed mail received.
- Last-message-read state is per caller and conference. Historical storage
  uses each conference's `SFMSGx.LMR`; the native backend need not reproduce
  that physical layout.
- Historical conference files include `SFMSGx.DAT`, `.PTR`, `.IDX`, and
  `.LMR`. They are a future compatibility backend, not the native SQLite
  representation.

## Implemented Domain Model

`sf-core::message` defines the storage-independent types used by the session:

- `ConferenceId` is a stable internal identity; `Conference::number` is the
  caller-facing number.
- `MessageId` is a stable internal identity; `Message::number` is allocated
  independently within its conference.
- `Conference` holds its stock-oriented access mode, read/post levels,
  public-only policy, line limit, and privileged levels.
- `Message` holds immutable author and recipient snapshots, optional caller
  identities, CP437-preserving subject/body bytes, timestamp, visibility,
  kind, and optional reply parent.
- `MessageKind::SysopComment` distinguishes the stock Conference 1 comment
  path without inventing a second mail store.
- `MessageActor` identifies the authenticated caller whose current persisted
  state and security are rechecked by the backend.

SQLite row IDs are never presented as historical message numbers merely by
accident. The two concepts are intentionally separate.

## Backend Boundary

The session uses the narrow `MessageBackend` contract for:

- recipient resolution;
- accessible-conference listing and lookup;
- visible-message listing and retrieval;
- posting;
- effective queued-conference lookup and replacement;
- last-read updates and lookup; and
- direct-message receipt lookup; and
- caller received/sent/available counts.

`RuntimeDatabase` is the Increment 3 SQLite implementation. The session and
transport adapters do not issue SQL. Future original-SPITFIRE, SMB, or network
adapters must enforce the same authorization contract rather than relying on
menu hiding.

This remains deliberately narrower than the eventual backend described in
`docs/07-message-system.md`; maintenance, deletion, network metadata, search,
and imports will extend the boundary only when exercised.

## SQLite Schemas 3 and 8

Migration 3 upgrades existing schema-2 boards transactionally and preserves
board/caller state. It adds:

- `message_conferences` for stable identities, caller-facing numbers,
  descriptions, access mode, read/post security, privacy policy, line limits,
  and active state;
- `conference_privileged_security` for documented exact-level exceptions;
- `messages` for per-conference numbering, author/recipient identities and
  display snapshots, byte subjects/bodies, reply parent, visibility, kind,
  deletion state, and timestamps;
- `caller_last_read` keyed by caller and conference; and
- `callers.messages_posted` as the first message-related caller statistic.

Schema 8 upgrades schema-7 boards transactionally without changing existing
callers, profiles, conferences, messages, last-read pointers, files, or
statistics. It adds only:

- `caller_message_queue`, keyed by caller and conference, for optional queue
  membership; migration seeds active callers with Conference 1 and the runtime
  also treats Conference 1 as an effective invariant for later callers; and
- `caller_message_receipts`, keyed by caller and message, for idempotent first
  receipt of mail addressed directly to that caller.

Queue membership and receipts are independent of `caller_last_read`. A normal
read advances the conference high-water and records a receipt when the caller
is the direct recipient. Re-reading or reconnecting does not double-count.
Named-Sysop preview changes neither state. Sent counts derive from surviving,
visible authored messages rather than trusting the older cumulative
`messages_posted` field.

Foreign keys, uniqueness checks, range checks, normal scan indexes, and
parameterized queries are used. Message subjects and bodies are SQLite BLOBs
so CP437 bytes do not undergo an accidental UTF-8 conversion.

## Authorization and Privacy

Every backend operation reloads the actor by stable caller ID and rejects
disabled, deleted, or missing callers. Conference authorization applies the
configured access mode, Sysop threshold, privileged security levels, and
separate entry/post level.

Private-message filtering occurs inside backend list and direct-read paths.
The message is visible only to:

- its author;
- its intended recipient; or
- a caller currently at the configured Sysop security threshold.

Posting obtains the author from the authenticated actor. No caller-supplied
author field exists. Recipients must resolve to an active local caller.
Last-read mutation always uses the authenticated actor's caller ID, never a
caller ID supplied by terminal input.

Queue and receipt lookup re-run that same caller/conference/message
authorization. Unrelated callers cannot query private bodies, private receipt
state, or another caller's received/sent presentation. Effective queues are
filtered to currently accessible conferences. Conference 1 is mandatory;
optional conference selection is per caller and persists independently across
nodes and reconnects.

## Caller Experience

The synthetic fixture provides two persistent conferences:

1. `General`
2. `SPITFIRE`

Each contains one clearly synthetic welcome message. No historical Buffalo
Creek message content is committed.

The Message Menu supports:

- `C` — list accessible conferences with unread counts and change current
  conference;
- `B` — browse visible To/From/Subject headers with unread/private indicators;
- `R` — choose This, All, or Only Queued conferences; start at the first unread
  visible message; move next/previous or directly by number; reply; and enter
  same-subject thread navigation;
- `E` — enter a public or non-public message;
- `Y` — show new waiting, already received, sent, and total available counts;
  list received or sent messages with status; and directly read a listed
  conference/message;
- `A` — add/delete/list/select queue conferences, include all accessible
  conferences, or remove all except mandatory Conference 1;
- `F`, `Q`, `G`, `X`, and `?` — the existing shared navigation, logoff,
  expert-mode, and help paths.

Main-menu `C` implements Comment to Sysop. It resolves the configured Sysop
caller, stores a private `SysopComment` in Conference 1, and fails clearly
without saving if the configured Sysop caller has not been initialized.

## Composition and Recovery

The composer is a bounded line-oriented editor shared by every terminal
transport. Entering a blank line opens the stock command menu:

- Save, Edit, Abort, Continue, Begin Again, Replace Line, List, Insert Line,
  and Delete Line(s) are implemented;
- `/S` and `/A` remain safe direct equivalents for existing callers and
  scripted sessions;
- reply CTRL+Q reviews the original and imports an inclusive line range with
  sender initials such as `MW> `; imported quote lines are immutable;
- each line is bounded;
- subject, total body size, and conference line count are bounded; and
- EOF/disconnect before confirmed save leaves no partial message.

The editor is intentionally line-oriented. Local arbitrary-path CTRL+F import
is not exposed through remote sessions or generalized into an external-editor
subsystem, and no full-screen editor was introduced.

## Replies and Last Read

A reply stores a durable parent `MessageId`. SPITFIRE prompts whether the
subject should change; declining preserves the original bytes exactly, which
keeps the reply in the stock same-subject thread. Thread Start, Forward,
Backward, and Exit operate on visible messages in conference/message order and
return to the original read position. A changed-subject reply remains a reply
in durable metadata but does not join the stock subject thread.

Reading a visible message normally advances `caller_last_read` monotonically
for that caller/conference and creates an idempotent direct receipt when the
caller is its recipient. Reconnects reuse both states. A direct read denied by
privacy cannot mutate either. Named-Sysop preview displays visible messages but
intentionally changes neither last-read nor receipt state.

## Error Behavior

Expected invalid conference/message/recipient choices are reported to the
caller without panicking. A canceled or disconnected composition is not
stored. SQLite failures and malformed persisted data become typed errors and
follow the existing session cleanup path, which releases the acquired node.

## Transport Independence

No message behavior exists in a transport adapter. Telnet, raw TCP, RLogin,
Unix stdio, direct serial, and modem-established sessions all enter the same
authenticated session and `MessageBackend` path. Optional RLogin auto-login
uses the ordinary credential verifier before message access.

Direct serial and Hayes paths are covered with synthetic PTYs/state machines;
physical hardware remains unverified.

## Known Fidelity Gaps

- Conference administration is not yet caller/Sysop interactive; the fixture
  seeds configured records through the core API.
- Text/caller search and automatic logon/daily scanning remain follow-on work;
  interactive This/All/Only Queued scanning is complete.
- Message timestamps are stored as Unix seconds and rendered as explicit UTC;
  board-local timezone policy and exact stock date presentation remain open.
- Deletion/undelete, copy/move/forward, carbon copies, Sysop preview, purge,
  and maintenance are not implemented.
- Original SPITFIRE `.DAT/.PTR/.IDX/.LMR` is not an active backend yet.
- SMB, QWK/LAKOTA, DOVE-Net, FidoNet, and CircuitNet are explicitly outside
  Increment 3.

## Tests and Reproduction

Committed synthetic tests cover migrations from schema 2 and schema 7,
conference security,
exact-level/privileged access, fixture seeding, public posts, author
enforcement, replies, private visibility including direct reads, stale
disabled sessions, last-read, unread counts, cancellation/interruption,
invalid recipients, queue persistence/enforcement, idempotent receipt state,
sent/received accounting, reconnects, per-caller isolation, all scan scopes,
same-subject traversal, CP437 quote bytes, every bounded editor command,
named-Sysop preview, Sysop routing, RLogin auto-login, and the common network/
stdio/serial/modem session path.

The 2026-08-22 closure acceptance created a board through the normal setup
service, installed controlled Conference 2 queue/direct/thread fixtures, and
completed the new workflow over a real Telnet listener with ANSI/SyncTERM-
style negotiation. The caller quoted and replied, traversed the thread, ran
all and queued scans, opened received/sent lists, disconnected, and reconnected
with one received and one sent message persisted. A RAW TCP session then
opened the persisted sent reply body. This was automated listener acceptance;
the external SyncTERM application was not available in the execution
environment, so no claim of a new manual SyncTERM build/version run is made.

The Increment 3 acceptance run also started the actual fixture executable and
connected to its localhost Telnet listener. A new caller read both seeded
conferences, replied, posted `Hello from SPITFIRE NG`, sent private mail to a
second caller, left a Sysop comment, logged off, and reconnected. The public
posts and read high-water persisted; the intended recipient could directly
read the private subject/body, while the unrelated caller neither listed it
nor opened it by number. Listener shutdown remained clean. Temporary
acceptance credentials and board state lived outside the repository and were
not committed.

To create a new fixture board:

```text
cargo run -p sf-bbs -- init-fixture ./var/fixture-board
cargo run -p sf-bbs -- init-sysop ./var/fixture-board/spitfire.toml
cargo run -p sf-bbs -- run ./var/fixture-board/spitfire.toml
```

The fixture is generated and ignored. Never replace its synthetic messages
with proprietary corpus data in committed tests.
