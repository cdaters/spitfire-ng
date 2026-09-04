# Protected Operator Attachment

<!-- help-topic: operator.security -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)

Schema 19 and B021-A provide the first protected control-plane slice for a
running board. It is deliberately read-only. The `spitfire operator` client,
the foreground console, and future `sfmonitor`/`sfconfig` clients share the
daemon-owned `OperatorService`; none reads SQLite, transient status files, or
diagnostic logs as operational authority.

## Durable state

Migration 18→19 is transactional and adds only:

- `operator_command_journal`, a bounded 30-day retry receipt for future
  state-changing commands; and
- `operator_control_audit`, append-only semantic security accountability for
  authentication, protocol, authorization, and later privileged actions.

Both start empty. Migration does not synthesize commands or audit and does not
alter schema-18 caller, message, file, transfer, storage, observability, or
security-audit authority. Cold backup preserves both tables. Restore never
preserves live connections, challenges, subscriptions, endpoint files, or a
running daemon generation.

## Local endpoint and identity

On Unix-like systems the daemon normally binds `sfop.sock` inside the
configured board-private Work directory. If that pathname exceeds the native
Unix-socket limit, it derives a board-specific socket name under an
owner-specific `/tmp/spitfire-ng-operator-<uid>` directory instead. Either
parent is a real directory owned by the board owner and is tightened to mode
`0700`; the socket is mode `0600`. Startup rejects symlinks, wrong ownership,
non-socket collisions, and a live pre-existing endpoint; only a stale socket
is removed. The SHA-256-derived short name identifies the board without
exposing its path. There is no TCP operator listener.

The daemon obtains the peer UID from the Unix socket and maps it to the typed
`operators.local_identities` board configuration. New setup and fixture boards
record the creating board owner's UID with the six B021-A read capabilities.
For an existing board whose list is empty, ownership of the board root is the
bootstrap identity. The policy is reloaded and authorization is checked again
for each request, so removing an identity or capability affects an attached
client's next dispatch.

On Windows the endpoint is
`\\.\pipe\spitfire-ng-operator-<board-id>`, where `board-id` is the first 32
lowercase hexadecimal characters of a domain-separated SHA-256 digest of the
canonical UTF-16LE configuration path. It is deterministic per board without
exposing the path or a secret. The daemon creates at most 32 local-only pipe
instances with first-instance protection and an explicit protected DACL.

The DACL grants generic-all access to the daemon process SID and exact client
read/write/synchronize/metadata rights to configured operator SIDs, excluding
pipe-instance creation. It contains no `Everyone`, `Anonymous`, or broad
`Authenticated Users` grant. Administrator membership, elevation, and
LocalSystem are not implicit operator authorization. A daemon service SID and
human operator SID may differ.

The server briefly impersonates the connected named-pipe client, opens its
thread token, reads `TokenUser`, canonicalizes the SID, and reverts under RAII
before it performs other work. The application protocol never supplies peer
identity. Setup records the creating Windows SID; an older board with no
Windows SID fails closed rather than trusting its first connection. No
fallback key is used on normal Windows systems.

The board policy is reloaded at each dispatch. Removing a SID or capability
therefore denies the next request on an existing connection. The listener
DACL is fixed when it is created, so adding a previously absent SID requires
listener recreation or daemon restart before a new connection can pass both
layers. Windows kernel object lifetime means there is no Unix-style stale
socket file to remove.

## Protocol

Protocol 1.0 uses a four-byte big-endian length followed by a typed JSON
message. JSON is an encoding, not a generic RPC surface: every request and
response is a closed enum. Frames are limited to 1 MiB, requested features to
32, ordinary deadlines to five seconds, and accepted deadlines to 30 seconds.
Zero, oversized, truncated, malformed, wrongly sequenced, and unsupported
messages fail explicitly without exposing Rust errors or backtraces.

The sequence is:

1. client hello with major/minor and requested features;
2. server hello with negotiated minor/features, schema, opaque daemon
   generation, one-use challenge, and operator-session ID;
3. challenge echo bound to the authenticated connection and generation; and
4. typed read requests carrying the session ID, generation, request ID, and
   deadline.

A major mismatch fails. A compatible minor negotiates the common feature set.
Feature support and operator authorization remain separate checks. Daemon
restart changes the generation and requires a fresh connection, peer check,
negotiation, and challenge.

Ordinary successful reads do not create durable audit noise. Authentication,
protocol incompatibility/violation, and sensitive read denials create semantic
control-audit rows without payloads, responses, or secrets. An unauthenticated
peer is recorded only as `unknown-peer`; its OS identity is not retained.

## Read projections

The transport delegates to the existing schema-18 services for board status,
node list/detail, recent operational events, live events, notifications,
statistics, recent callers, and maintenance status. Recent-event queries keep
their typed filters, 100 default, 500 maximum, 31-day detail window, stable
ordering, and snapshot cursor across the protocol. The live operation uses the
existing 2,048-event ring and a connection-lived bounded 256-event subscriber
queue. Cancellation drops that ephemeral subscription. A gap is explicit; the
client recovers through durable recent events.

Only privacy-safe projection types cross the endpoint. They exclude login
identifiers, private real names/contact fields, credentials, message bodies
and recipients, terminal input, endpoints, host paths, file contents, keys,
raw packets, and future network secrets.

## Failure and lifecycle

Client timeout, crash, or disconnect drops only connection-local state. It
does not stop the daemon or callers and leaves no durable IPC session. Multiple
read clients may attach concurrently. The initial CLI returns a nonzero
process status for endpoint, authentication, authorization, compatibility,
timeout, and safe internal failures.

B021-A exposes no page, chat, time grant, disconnect, shutdown, notification
acknowledgement, configuration, backup, maintenance, or arbitrary host action.
Those commands require later B021-B/B021-C authorization and stale-target
acceptance.

Native Windows tests exercise SID parsing, DACL construction,
multi-client attachment, every read projection, dispatch-time revocation,
malformed/oversized frames, protocol mismatch, replay, stale generation, and
disconnect. A separate-process Windows journey additionally proves setup,
ordinary CLI use, two caller connections, two live subscribers, denial of a
disposable unlisted local account, and restart/reattach. The same
`OperatorClient` and protocol are used on Windows and Unix.

See the [B021-A implementation record](../research/m039-tranche-7-b021a-protected-operator-attachment.md)
and [B021-AW Windows acceptance](../research/m039-tranche-7-b021aw-windows-operator-attachment.md).
