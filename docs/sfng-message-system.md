# Native SPITFIRE NG Message System

This is the canonical implementation and compatibility specification for the
native message subsystem introduced in Stock SPITFIRE 3.7 Core Parity
Increment 3 and extended in post-0.1.0 source with advanced discovery and
auditable message mutation. It explains what is implemented, which stock
behaviors were established from Buffalo Creek's SPITFIRE 3.7 manual, how
authorization and persistence work, and which historical features remain
incomplete.

The broader multi-backend direction remains in [Message System
Design](07-message-system.md). Exact historical file findings remain in
[Legacy Data and File Formats](06-legacy-file-formats.md) and the
[Historical SPITFIRE Overview](HISTORICAL-SPITFIRE.md).

## Original SPITFIRE Behavior Established

The primary operational source is the original SPITFIRE 3.7 manual held
outside this public repository, especially its conference configuration and
Message Menu sections.

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
- Specific Caller Messages searches current, all, or queued conferences for
  public messages to, from, or both to/from a resolved caller. A threshold
  Sysop may see otherwise authorized private results.
- Text Search accepts one to six key words, searches current, all, or queued
  conferences, displays each visible match with a continue prompt, and reports
  a final matching-message count. Original-runtime evidence confirms ASCII-
  insensitive body substrings, Subject exclusion, and contiguous-phrase
  behavior for the tested space-containing query.
- Last-message-read state is per caller and conference. Historical storage
  uses each conference's `SFMSGx.LMR`; the native backend need not reproduce
  that physical layout.
- Historical conference files include `SFMSGx.DAT`, `.PTR`, `.IDX`, and
  `.LMR`. They are a future compatibility backend, not the native SQLite
  representation.
- Stock permits up to nine nonduplicate carbon copies. Live SF37 evidence
  creates one separately numbered primary delivery and up to nine CC
  deliveries sharing content; each delivery has its own recipient, RECEIVED,
  and deleted state. Conference policy lets the sender, direct recipient, or
  CC recipient delete that delivery, while configured threshold-Sysop status
  can delete/undelete, toggle public state, and use read-time Copy. The only
  observed Copy workflow handles same/cross-conference copying and recipient
  change, always retains the source, and offers no automatic source delete.
  Threshold `P` retains a named recipient for public→private and offers
  `Address Message #N to "All Callers"? [Y/n]` for private→public: Yes changes
  the audience to All, while No keeps the name. Identity, author, and RECEIVED
  state survive these transitions.

## Implemented Domain Model

`sf-core::message` defines the storage-independent types used by the session:

- `ConferenceId` is a stable internal identity; `Conference::number` is the
  caller-facing number.
- `MessageId` is a stable internal identity; `Message::number` is allocated
  independently within its conference.
- `Conference` holds its stock-oriented access mode, read/post levels,
  public-only and caller-deletion policy, line limit, and privileged levels.
- `Message` joins an immutable author/content payload with a separately
  numbered delivery envelope: named or All-Callers audience, primary/CC role
  and ordinal, recipient snapshot, conference placement, visibility,
  active/deleted lifecycle, monotonic state version, receipt state, timestamp,
  and optional reply parent.
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
- bounded caller/text discovery returning message references;
- atomic posting with an optional ordered CC fan-out;
- effective queued-conference lookup and replacement;
- last-read updates and lookup;
- direct-message receipt lookup;
- caller received/sent/available counts;
- dispatch-authorized mutation capabilities, delete/undelete and audience
  toggle; and
- same/cross-conference Copy with optional recipient change.

`RuntimeDatabase` is the Increment 3 SQLite implementation. The session and
transport adapters do not issue SQL. Future original-SPITFIRE, SMB, or network
adapters must enforce the same authorization contract rather than relying on
menu hiding.

The local message domain is authoritative. Future QWK, SMB/DOVE-Net, FidoNet,
CircuitNet, or other adapters must import and export through these domain
interfaces instead of writing a parallel message store or bypassing delivery,
visibility, receipt, mutation, and audit rules. This is an architecture
boundary only; those network and offline-mail adapters are not implemented.

Discovery returns references rather than cached body/authority snapshots. The
session reopens every result through the ordinary conference/message path
immediately before display. This remains deliberately narrower than the
eventual backend described in `docs/07-message-system.md`; packing/retention,
network metadata, and imports will extend the boundary only when exercised.
Schema 11 provides immutable payloads, normalized fan-out and separately
numbered ordered delivery identities, per-delivery tombstone/receipt state,
Copy/Forward lineage, state versions, and same-transaction mutation audit.
There is no atomic move: cross-conference Copy retains the source, and any
later Delete is independently requested and authorized.

## SQLite Schemas 3, 8, and 11

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

Schema 11 `auditable_message_mutation` is current. It rebuilds legacy messages
into immutable `message_payloads`, `message_fanouts`, separately mutable
delivery `messages`, and named `message_delivery_recipients`; All Callers is an
audience kind rather than a fake caller. `message_lineage` records Copy versus
Forward, while append-only `message_mutation_events` records only bounded
identifiers/state/actor/outcome—not subject, body, or recipient lists.

Migration 10→11 is one validated transaction. Existing rows become one
payload/fan-out/delivery while preserving message IDs and numbers, author,
subject/body BLOB bytes, reply parent, visibility, deletion, receipts, and
last-read. Before commit, Rust validation compares counts, identities, numbers,
byte lengths, recipient cardinality, state, and foreign keys. Failure leaves
schema 10 unchanged. There is no downgrade; rollback uses the old executable
and a pre-upgrade cold backup.

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

Discovery uses the same authority. Text search includes the actor's visible
public/authored/received mail and current threshold-Sysop visibility. Specific-
caller search adds the manual's public-only restriction for ordinary callers.
Deleted rows remain unavailable to ordinary callers and discovery. Threshold
direct/read scans may reopen them for contextual Undelete. Search terms and
message content are not logged.

Every mutation reloads actor/message/conference/recipient state at dispatch.
Ordinary sender, direct recipient, or CC recipient may tombstone only that
delivery when conference caller deletion is enabled; unrelated callers and a
named-but-below-threshold Sysop cannot. Threshold status may delete any
delivery, undelete a deleted identity, toggle audience, and Copy. State-version
predicates turn stale actions into deterministic conflicts. Each committed
operation and its privacy-safe audit event share one transaction; a failed or
denied action records no false committed event. Ordinary navigation skips
deleted deliveries, while threshold readers can reopen them and receive
contextual Undelete.

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
- `S` — choose current/all/queued scope, resolve an active caller, choose
  From/To/Both, and display authorized matching messages without changing read
  state;
- `T` — choose current/all/queued scope, enter one to six bounded body terms,
  and display authorized matches one at a time without changing read state;
- `D` — tombstone the displayed delivery when dispatch authorization permits;
- `U` — threshold-only and contextual while the displayed delivery is deleted;
- `P` — threshold-only public/private and All-Callers audience transition;
- `C` while reading — threshold-only same/cross-conference Copy; `Change? Yes`
  selects a new recipient and is the stock Forward workflow;
- `F`, `Q`, `G`, `X`, and `?` — the existing shared navigation, logoff,
  expert-mode, and help paths.

Entry of a named primary automatically offers `Carbon copy #1:` through at
most `Carbon copy #9:`. A blank finishes early. The same validation rejects
the author, primary/CC duplicates, nonexistent/inactive callers, and a caller
whose queue excludes the destination; valid earlier CCs survive a rejected
attempt. Save commits the entire fan-out or none of it.

Copy allocates a new destination conference-local number and independent
active/unread/version state while sharing immutable payload bytes, preserving
original author and reply parent, and linking the destination to its source.
The source never changes. A recipient change preserves original authorship and
records the threshold actor only in private audit metadata. No separate Move
or Forward command and no automatic source deletion exist.

Main-menu `C` implements Comment to Sysop. It resolves the configured Sysop
caller, stores a private `SysopComment` in Conference 1, and fails clearly
without saving if the configured Sysop caller has not been initialized.

Current-source text matching is body-only, ASCII-case-insensitive contiguous
substring matching with all NG whitespace-delimited terms required and a
displayed-message count. Original-runtime evidence establishes case-insensitive
substrings, body rather than Subject, and message rather than occurrence
counts. It also establishes that stock treats the tested space-containing
`alpha beta` expression as one contiguous phrase: ordered-noncontiguous and
reversed bodies do not match. NG intentionally retains its more useful all-term
rule as a documented modernization while preserving the stock command, scope,
visibility, presentation, and read-only behavior. CP437 bytes outside ASCII are
not decoded, folded, or normalized. Search examines at most 10,000 candidates
and returns at most 100 ordered references; the continue prompt provides one-
result-at-a-time paging.

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
- Original matcher/count behavior needed for B-008 acceptance is resolved and
  bounded native discovery is VERIFIED with its all-term modernization
  documented. Explicit OR and quoted-phrase syntax remain possible future
  enhancements. Automatic logon/daily scanning remains follow-on work.
- Message timestamps are stored as Unix seconds and rendered as explicit UTC;
  board-local timezone policy and exact stock date presentation remain open.
- Carbon copies, tombstone/undelete, public/private audience changes,
  source-retaining Copy/Forward, and threshold mutation are verified. Physical
  purge/packing/retention and broader maintenance/audit viewing remain future
  work. Named-Sysop read-only preview remains verified and distinct from
  threshold mutation authority.
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
named-Sysop preview, caller/text discovery scope and visibility, malformed and
bounded terms, CP437 exact matching, deterministic result limits, concurrent
post/search connections, no read-state mutation, Sysop routing, RLogin auto-
login, and the common network/stdio/serial/modem session path.

Schema-11 coverage adds schema-10→11 preservation and failure rollback; CP437 payload
sharing; primary plus nine ordered CC deliveries; duplicate/self/nonexistent/
queue rejection; per-delivery receipts/tombstones; sender/direct/CC/unrelated/
threshold authorization; conference deletion policy; contextual Undelete;
both audience-toggle branches; same/cross-conference Copy; Forward recipient
change; source retention; destination numbering/receipt independence;
lineage/audit privacy and append-only enforcement; stale/two-connection
conflicts; discovery/preview regressions; and schema-11 cold backup/restore.

Message-mutation live acceptance used only disposable synthetic callers and
content. Qodem 1.0.1 ANSI/CP437 deleted a direct-recipient delivery. SyncTERM
ANSI/CP437 displayed the separate CC number, CC/primary header, and deleted
only that CC delivery. RAW/text verified unrelated denial, threshold delete/
contextual Undelete, public→private, private→public with All Callers Yes and
No, same- and cross-conference Copy, Forward through recipient change, source
retention, and restored-board recipient privacy. Two simultaneous threshold
clients displayed one version; the first Delete committed and the stale second
received localized conflict/redisplay. Cold backup/new-root restore preserved
an identical logical SQLite dump and the restored caller reopened the private
Forward with original author attribution.

Advanced-discovery live acceptance used synthetic callers over RAW/text,
Qodem 1.0.1 ANSI/CP437, and SyncTERM 1.9rc4 ANSI. Each caller posted one public
message, found it through text and self-From discovery, returned to Main, and
logged off normally. A read-only database check showed last-read zero and no
receipts for all three callers.

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

## M045 / schema 20 caller QWK integration

The [QWK offline Technical Reference](technical/qwk-offline.md) defines the implemented adapter, native authority, delivery/pointer semantics, private artifact custody, transactional receipts and recovery. Caller QWK uses ordinary authenticated message permissions and existing binary transfers. No QWK networking, DOVE-Net, FTN, scheduler or separate message store is added. Earlier dated schema/milestone descriptions retain their historical scope.
