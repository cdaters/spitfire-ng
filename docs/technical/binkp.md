# Native BinkP transport — N4 implementation contract

Status: **N4 COMPLETE / ACCEPTED**; controlled independent interoperability passed.
No live public-network claim.
This refines [M044](../research/m044-networking-foundation-gate.md) and consumes
the accepted [native FTN core](ftn-core.md). Native messages, scanner/tosser,
routing decisions and the common outbound queue remain canonical.

## Wire and service interfaces

`sf-net::binkp` owns bounded frames, file descriptors, address presentation and
CRAM-MD5 primitives. FTS-1026.001 and FTS-1027.001 are primary. FTS-1028.001 supplies NR offset rules. The original N4 baseline was 1.0;
the response-batch contract below adds negotiated 1.1 response batches with 1.0 fallback.
Command/data framing is independent of sockets and native message storage.

Daemon session workers perform `prepare(link, policy-version, mode)`, handshake,
exchange and finish. Exchange leases existing N3 work and acknowledges each item
only after the matching M_GOT. Receive completes private custody and N3 tossing
before issuing M_GOT. The latter means custody, not a promise of semantic import:
malformed mail remains durable quarantine. Test mode never claims work or accepts
inbound custody. M_SKIP retains work. Offset requests address only already-offered
immutable artifacts. Incomplete inbound bytes never enter the tosser.

Static transport configuration refers to existing FTN link IDs; it does not copy
routes or final destinations. Permitted local AKA sets are explicit. Admission
requires the configured remote endpoint, including domain and point. Domainless
compatibility must be an explicit link setting. CRAM is required by default;
plain compatibility is explicit, and credentials are write-only operator inputs.
CRAM authenticates the originator to the answerer; M_OK and an address claim do
not prove answerer identity cryptographically or encrypt message content.

Durable transport additions are claims/results and link health, around the shared
queue. A claim records daemon generation and frozen policy identity. One exchange
per link and two total sessions are admitted. Restart invalidates stale claims;
restore holds uncertain work, preserves accepted receipts, and never resumes a
socket/session. Origin serial reconciliation remains explicit because a backup
cannot know messages originated after its snapshot.

## Negotiated response batches

The session negotiates BinkP 1.1 only from an explicit case-insensitive
`binkp/1.1` VER token received before authentication is sent/completed. Missing,
late or unsupported versions retain 1.0 behavior. Local VER precedes ADR so the
peer can select the protocol before sending its authentication. This changes no
AKA, link, destination or file authorization.

FTS-1026.001 defines the 1.0 single-batch baseline and does not define FREQ.
FSP-1024.000 is an unfinished **proposal**, whose sections 3 and 4.1–4.3 describe
multiple batches, empty-batch termination and mandatory NR offset negotiation.
Installed independent JSBinkP behavior and a bounded empty-session wire capture
corroborate the two-direction EOB handshake. Scripted authenticated sockets and a completed independent FREQ exchange
validate the response-batch model, including acknowledgements after EOB.

In 1.1, EOB ends a send batch. A received file offer starts a new peer batch and
invalidates received empty-batch evidence. Acknowledgements may straddle peer
EOBs and preserve that evidence. A new outgoing file starts a fresh local batch;
local confirmations reaffirm local empty batches for the peer. At least two
empty EOBs in each direction, no outstanding acknowledgements/partial reception,
and a drained output queue complete the session. Sending another empty EOB
requires new peer EOB progress, so a waiting loop cannot flood commands. New
file offers are accepted only through normal authenticated offer/import authority;
unannounced data and handshake commands during transfer remain errors.

At bounded batch transitions the backend may claim newly queued file work for
the same authenticated session, using remaining session file/byte capacity and
existing durable claims. No reauthentication, routing expansion, automatic FREQ
reissue or receipt reset occurs. NR `-1` is a typed request for an offset; reception
requests offset zero and transmission waits for matching M_GET before data. The
ordinary numeric Offer parser remains strict. Unsupported options are not enabled.

Existing limits remain: 30-second handshake, 60-second idle and 600-second total
session deadline; 64 files per direction, 64 MiB per direction, 4,096 commands.
An additional 128 EOBs per direction bounds batch transitions. A close/timeout
with missing payload does not complete its durable FREQ request; schema-27 retry
eligibility and maximum attempts remain authoritative.

FireComm review was read-only (`docs/21-phase-10-transport-maturation.md`,
`firecomm-transfer/src/safety.rs`): adapt explicit protocol state and capture
exclusion; retain NG exact-name rejection instead of filename rewriting.
FireComm has no BinkP protocol authority or dependency role. No source was copied.

## Selected boundaries

TLS-wrapped streams are deferred: the primary baseline defines no STARTTLS or
universal TLS mode; independent peer support alone does not create a standard.
Compression, CRYPT, CRC extensions and persistent partial-file resume are not
advertised. Retries restart inbound reception safely at zero; outbound M_GET
offsets remain part of the base protocol. N4 admitted only N3 packet artifacts;
the accepted [N7 file contract](ftn-files.md) supplies current FileEcho/TIC/FREQ
authority, and [N6 hub authority](ftn-hub.md) supplies AreaFix.

The defaults follow M044: connect 10 seconds, handshake 30 seconds, idle 60 seconds,
session 10 minutes, two sessions, one per link, 16 MiB per packet, bounded session
files/bytes, and durable retry backoff. Exact implemented limits and threat review follow; operator workflow and
independent evidence are recorded in the linked manual and M048 report.

## Bounded secondary reference findings

Synchronet BinkP/BinkIT API and session/queue callback documentation were reviewed
read-only. ADOPT independent bidirectional packet exchange and point/AKA checks;
ADAPT link-specific advertisement and completed-file callbacks to native custody;
REJECT BSO/file deletion as queue authority, debug frame logging, SMB coupling and
credential-context expansion from arbitrary presented AKAs. DEFER CRYPT and TLS.
No reference source, algorithms, comments or configuration are copied.

NodelistDB's BinkP tester was reviewed read-only. ADAPT bounded endpoint/latency and
separate address-validation results; REJECT anonymous probing as authenticated
mail admission, remote text as trusted diagnostics and global testing analytics.
Directory lookup remains N3 authority, with explicit endpoint override precedence.

FireComm's transport interface and Phase 10 transport document were reviewed
read-only under the cross-project policy. ADAPT worker ownership, bounded commands,
handshake-versus-application separation and explicit security profiles. Its TLS
trust implementation is FireComm-specific; no shared dependency or code copying.

## Schema 24 and queue transactions

The schema-23 native tables and static FTN routing policy remain unchanged in
ownership. Transactional migration 24 adds five small relational tables:

| Table | Authority |
| --- | --- |
| `binkp_link_health` | One active session ID/generation per N3 link; policy digest, attempt/success/error, latency, held/backoff and Test Link cooldown |
| `binkp_queue_claims` | Unique existing queue ID leased to that active session, frozen queue version and offered flag |
| `binkp_peer_addresses` | At most 32 explicitly admitted typed endpoint observations per link |
| `binkp_capabilities` | At most 32 recognized safe option tokens per link |
| `binkp_custody_receipts` | Immutable link/artifact receipt, charged to the shared bounded history budget |

Migration does not fabricate sessions or touch message bodies. Failure rolls the
whole migration back. Claims overlay the existing pending/ready/retry/held/accepted/
failed queue states; no BinkP-owned parallel queue exists. Immediate transactions
serialize admission and claims across SQLite connections. Frozen native publication
versions and routing policy are revalidated before materializing work. Link-local
advertisement also constrains queued source AKAs. N3 final destinations remain
separate from the selected transport next hop.

An offered claim is required for acceptance. Matching filename/size/time M_GOT
commits the existing queue's accepted state and delivery attempt, then removes the
claim. A late operator restriction cannot erase a true peer acknowledgement.
Failure returns unacknowledged claims to retry/held/failed with a durable attempt;
accepted items stay accepted if another file or session termination fails.
Hold/release operations require queue CAS, N3 policy compatibility and no active
claim. Release cannot reset exhausted attempt history.

N3 toss transactions supply semantic idempotence, including a crash between a
message commit and transport receipt. A completed packet is acknowledged only
after immutable private artifact custody and N3 import/routing/quarantine. Replay
runs through those durable N3 receipts; transport receipt alone never substitutes
for message identity. Restoring an old snapshot cannot invent subsequent peer
history. Unsent queues and origin serials therefore remain held for review.

## Framing, state and transfer detail

The codec checks the network-order two-byte header, command bit and 15-bit length
without structure casts. Empty frames are ignored boundedly. IDs above 127 and
malformed known commands fail; unknown optional command IDs below 128 are ignored
within the command budget. Optional trailing NUL is accepted; embedded NUL is
rejected. NUL greeting text may contain legacy bytes, but only recognized ASCII
option tokens are interpreted. Remote free text is never persisted or echoed.

Phases are Connecting, Greeting, Authenticating, Ready, Exchanging, Finishing,
Complete and Failed. The answerer sends a fresh 32-byte OS-random challenge first.
It selects exactly one configured link from ADR, then advertises that link's AKA
set. The caller validates ADR before PWD and requires an acceptable first greeting
under the CRAM-required profile. RustCrypto HMAC/MD5 verifies the FTS-1027 response
in constant time; known vectors include FTSC and RFC 2202 long-key cases. Optional
PWD extension tokens do not become password material. Unknown options do not
change authentication, routing, encryption or compression policy.

The nonblocking worker reads and writes concurrently, allowing real bidirectional
batches without a send/send deadlock. Each direction has at most one current file.
M_FILE metadata is checked before allocation. Only `.pkt` is accepted in N4;
unsupported types receive M_SKIP. Modern and legacy filename escaping are decoded
before rejecting path separators, absolute/hidden names and unsafe characters.
Remote filenames are metadata only: no filesystem path is constructed from them.

An interrupted incoming file is discarded; no partial file can survive a process
restart or be tossed. A nonzero inbound offset is answered with M_GET at zero and
the replacement header must match the requested file identity. Data or EOB before
that restart is rejected. Valid outbound M_GET offsets reseek the same immutable
offered artifact. Persistent partial resume remains absent; NR offset requests
are supported as specified in the response-batch contract. Outgoing M_SKIP retains work. M_GOT is matched to the current
file descriptor; unsolicited or mismatched acknowledgements fail closed.

EOB ends the current sending batch. Session completion additionally requires all
outgoing results, empty private partial state and drained frames. Negotiated 1.1
requires empty-batch evidence in both directions; fallback 1.0 uses its original
single-batch termination. Successful termination is checked before subsequent EOF.
Unknown capabilities and late VER cannot upgrade a 1.0 session.

## Bounds and security review

| Threat/resource | Enforcement |
| --- | --- |
| Unauthenticated admission / session flood | Two daemon-wide sessions, including greetings; one durable session per link; M_BSY at capacity; no queue claims or toss before authentication |
| Password guessing | Fresh random challenge, required CRAM default, finite auth errors, held link and durable cooldown/backoff; unknown peers never obtain a configured secret |
| Address spoofing / cross-domain collision | Full typed domain+zone/net/node/point expected identity; explicit domainless policy; only configured and presented aliases admitted for packet headers |
| Frame/command abuse | 32,767-byte wire frame; 4,096-byte command; 4,096 commands/session; bounded queued frames and exact incremental reads |
| Address/option amplification | 32 addresses, 32 recognized option observations; no arbitrary remote text in health |
| Filename/path overwrite | Decoded name 128 bytes, escaped representation 512; validated printable basename; never used as a path |
| Memory/disk exhaustion | 16 MiB/file, 64 files/session, 64 MiB each directional stream; two sessions; one current incoming and outgoing packet each; N3 artifact/history budgets still bind |
| Slow peer | Connect/DNS 10 s; handshake/auth 30 s; idle 60 s; absolute session 600 s; abort drain 100 ms; cancellation checked during connection and exchange |
| Queue races / same-link collision | Immediate DB claims, unique queue/session identity, frozen N3 policy and publication checks; hold refuses active custody |
| Replay / interrupted ACK | N3 message receipts and durable artifact custody; no M_GOT before complete toss/custody; no accepted queue state from TCP writes |
| Configuration/credential change | Expected policy digest, audited privileged operation, current-policy and credential-generation cancellation |
| Downgrade / TLS confusion | RequireCram never silently falls back; AllowPlain is explicit; no TLS/CRYPT advertisement or opportunistic fake encryption |
| NetMail disclosure | No bodies, secrets, raw packets, filenames or peer greeting text in events/health/audit; private payloads stay behind native access checks |

Secrets are stored separately from TOML under the private SYSTEM credential
directory. Unix directory/file modes are 0700/0600, symlinks and unsafe modes are
rejected, and atomic replacement avoids partial values. Backup/restore retains
these private permissions. Windows follows existing board ACL architecture;
live acceptance remains deferred. A credential command's idempotence fingerprint
replaces its value before hashing, so a durable receipt is not an offline password
verifier. Reusing its CommandId replays the first update; a new update needs a new ID.

## Daemon, operators and observability

Protocol minor 8 negotiates BinkpNetwork while retaining QWK minor 6 and FTN minor
7. Read access uses network-status. Poll requires network-run, Test requires the
new explicit network-test capability, queue controls require network-queue, and
credentials/configuration require change-sensitive-configuration. Read-only
bootstrap gains no new mutation or probing power. IPC commands retain current
principal checks, CommandId receipts, CAS and operator audit.

sfconfig imports typed transport policy and performs write-only credential
update/explicit clear. sfmonitor projects only a small dashboard link summary;
CLI Poll/Test/Hold/Release requests run asynchronously in the daemon. No TUI owns
sockets. Health exposes active, queued, last attempt/success/error, latency,
backoff/held, admitted addresses and recognized capabilities. Session, authentication,
identity, custody, completion, failure and hold/retry events carry finite codes.
Test success is a session result with zero local custody; remote eager offers are
skipped. C5 adds a generic daemon-owned [Event scheduler](events.md) that invokes this native Poll boundary; there is no competing BinkP-specific recurring poller.

Shutdown closes admission, cancels active work under existing daemon drain
accounting, flushes durable outcomes and discards partial memory. Restart calls
transport recovery before new listener admission. Restore first clears live
claims/generations, retains accepted receipts and then applies N3 uncertainty holds.
The restored listener can start normally, and credentials remain Configured.
No socket, handshake or partial file is resumed from a backup.

See the [Sysop workflow](../manual/binkp.md) and
[M048 evidence and reproduction](../research/m048-networking-n4-binkp.md).

System DNS resolution has a separate two-call process-wide ceiling. A timed-out
system resolver may finish later, but retains its slot until it exits; subsequent
polls cannot accumulate abandoned resolver threads. Session cancellation/deadline
returns promptly, numeric IPs bypass DNS, and at most sixteen resolved endpoints
are considered. No queued mail ownership is attached to an abandoned lookup.

Transport creates no additional persistent outbound copy or partial staging file.
Session buffers are released after completion/failure. Canonical N3 immutable
artifacts remain only under the accepted queue/provenance/receipt ownership and
shared capacity policy; transport does not delete receipt-owned evidence or grow
an unrelated packet spool. Capacity exhaustion holds custody safely for review.

## N5 authority extension

[Network operations and recovery](network-operations.md) specifies protected protocol 1.9
projections, named configuration forms and CAS, queue actions, retained-reference
checks and verified origin/acceptance reconciliation. Schema remains 24. Older
restore descriptions above describe the safe held default; N5 adds verified
recovery without inventing post-snapshot history. Wire/message authority is unchanged.

## N6 hub extension

The [FTN hub contract](ftn-hub.md) defines downstream/point configuration, separate area subscriptions, authenticated AreaFix, bounded rescan, safe activity and per-recipient recovery. Schema 25 and operator protocol 1.10 add these typed services; existing N1–N5 authority remains intact. Network bodies and credentials are absent from operator projections.

## N7 file-network extension

FileEcho, TIC, native-file hatching and exact approved FREQ reuse native file authority and BinkP transport. sfconfig offers typed file policy/mapping/subscription/grant/hatch forms; sfmonitor Networks → 9 Files includes staging, history and Enter delivery detail. [File-network workflows and authority](ftn-files.md). No raw path or credential appears in projections.
