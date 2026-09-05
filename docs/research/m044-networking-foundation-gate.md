# M044 — Public message-networking architecture summary

M044's design is binding for the networking implementation sequence. This public
summary describes accepted architecture; it does not redistribute private corpus
indexes, research history, standards text, historical files or secondary source.
M045/N1 implements caller offline QWK on schema 20. Later networking remains
unimplemented. See the [N1 report](m045-networking-n1-qwk-offline.md) and
[Technical Reference](../technical/qwk-offline.md) for implemented contracts.

## Native authority and adapter ownership

SPITFIRE NG has one native message authority: stable MessageId/ConferenceId,
immutable payloads, delivery identities, audience, access, mutation and receipts.
QWK and future FTN translate to and from that authority. There is no separate QWK
message database, SMB runtime dependency or normal-operation SMB compatibility
target. Earlier speculative mixed live message-store proposals are superseded.

`sf-net` owns pure bounded interchange codecs and later transport state machines.
`sf-core` owns typed native identity, mappings, authorization, receipts and atomic
mutations. `sf-bbs` owns daemon execution, restricted artifacts, configuration
activation and caller/operator integration. Operator clients cannot bypass native
services. Format machinery stays separate from caller and future network-partner
workflows; DOVE-Net will use the same QWK codec and network architecture.

## N1 caller QWK contract

Messages L opens D/U/S/Q and help. Download selects New or To You, then All,
Selected or Queued conferences. New uses native conference read pointers; To You
includes authorized directly addressed mail, including previously read messages.
Generation uses a bounded read snapshot and retains native IDs/versions, mapping
and policy, pointer/reset versions, high waters and private artifact identity.
Access is rechecked before transfer and confirmation. Failed or cancelled transfer
advances nothing. Successful transfer plus affirmative caller confirmation advances
pointers monotonically and marks only included addressed deliveries received.
Explicit pointer resets use compare-and-swap and invalidate stale completion.

ZIP Stored/Deflate, fixed 128-byte QWK/REP records, CONTROL, message indexes and
CP437 framing follow primary format documentation. The selected QWKE long-header
subset preserves fields without truncated identity lookup. Literal CP437 pi and
unrepresentable metadata are refused, not silently changed. Original logical
padding length is ambiguous; immutable raw artifacts retain wire evidence. Native
payload encoding is not silently migrated. Export is board-local minute precision;
REP wall time without an offset remains unresolved provenance alongside native
receipt UTC. Original LAKOTA LMR bytes remain evidence-qualified; N1 must not invent
a decoder or claim that a private manifest is an interoperable pointer file.

Reply author comes from the authenticated caller, never untrusted From. Current
native conference posting, private-recipient and content limits apply to every
reply. Outer framing is validated before mutation. Semantically valid members may
commit individually, with native post/statistics and immutable receipt in one
transaction. Retry resumes committed results. Exact retransmission identity uses
caller/profile, canonical ordered member names/uncompressed bytes and member
ordinal/digest, independent of ZIP recompression. Same content in another submission
is held for explicit review; deliberate repetition requires confirmed new-submission
intent. No exactly-once promise is made across rollback to an older backup.

The normal binary transfer engines are reused. Packet uploads are private intake,
not public catalog files. Complete artifacts and receipts are retained under
bounded custody; partial transfers have no live authority after restart/restore.
Journaled incomplete writes can be cleaned, but referenced or unknown files cannot
be silently collected. Backup includes durable receipts, mappings, manifests and
artifacts and validates their consistency. Operational events contain no bodies,
private recipients, login/real names, packet dumps or paths.

## Configuration, limits and future network phases

Configuration answers what should happen. Runtime SQLite answers what happened,
what remains to be done and what must not be duplicated. Typed relational mapping
configuration binds stable native identities to explicit wire keys; volatile
packet state does not belong in TOML. Protected credentials and artifacts have
safe identities. Existing read-only operator bootstrap, explicit mutation enrollment,
local authenticated IPC, CAS and idempotence remain binding.

The N1 limits include 16 MiB compressed/64 MiB expanded archives, 1,024 members,
1,000 messages, 64 KiB native bodies, 72-byte subjects, a 100:1 expansion limit,
1 GiB artifact custody and 2 million/512 MiB duplicate history. Current exact
admission and manifest limits are in the N1 Technical Reference. Paths, encryption,
links/devices, overlapping or inconsistent archive members and unsupported nesting
are refused before native mutation. Capacity pressure holds intake without erasing
retained identities. Packet jobs are bounded and daemon owned; no shell archiver,
external message authority or scheduler is needed for the caller workflow.

The later sequence is N2 QWK partners/DOVE profile; N3 FTN addressing, native
external origins/private transit, routing/scanner/tosser/queues and directory
foundation; N4 BinkP and controlled independent leaf/point acceptance; N5 full
Networks operator views; N6 routing/hub/point-boss and optional AreaFix/rescan;
N7 FileEcho/TIC/FREQ and attachments. FTN addresses include points, network/domain
and multiple local AKAs from inception. Private NetMail is distinct from area-based
EchoMail. Transport custody acknowledgment is distinct from remote native import.
DOVE-specific enrollment/routing conventions do not redefine generic QWK or create
a second store. Minimum safe operator controls accompany each implemented phase.

None of those later protocols/features is implemented by N1. No nodelist/pointlist
parser, FTN packet/address code, BinkP, scheduler, Networks UI, doors or CircuitNet
implementation is implied by this design. B-021 remains VERIFIED; B-022 remains
NOT STARTED. Windows live networking acceptance is deferred to a real Windows
environment; macOS is the primary N1 acceptance environment.

## Format/reference policy

Primary SPITFIRE documentation determines historical outcomes. Patrick Lee's
[QWK layout 1.6](https://wmcbrine.com/mmail/specs/qwklay.html), Mark Herring's
[1stReader programmer reference](https://wmcbrine.com/mmail/specs/qwk1st.html) and
Peter Rocca's [QWKE 1.02](https://wmcbrine.com/mmail/specs/qwke.html) determine their
format profiles. Later FTN implementation must verify applicable [FTSC standards](https://ftsc.org/docs/)
for its phase. Independent implementations are interoperability and engineering
references, never permission to copy code/resources or replace standards with
observed behavior. The N1 report records exact secondary peer purpose, version,
license and ADOPT/ADAPT/REJECT/DEFER dispositions.
