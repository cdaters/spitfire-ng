# SPITFIRE NG Secure SSH Caller Transport

## Purpose and status

Current source adds SSH-2 as an encrypted interactive caller transport. This
was delivered as the M042.5 modern transport milestone, not as a stock
SPITFIRE Category-B capability. SSH
enters the same node pool and session engine as Telnet, RAW TCP, RLogin,
serial/modem, and local terminal sessions. After M042.5, the stock roadmap
returns directly to proposed M043, B-002/B-003.

This service is only a route to the SPITFIRE NG caller experience. It does not
provide an operating-system shell, Unix account login, `exec`, SCP, SFTP,
arbitrary command execution, port/agent/X11 forwarding, remote processes,
remote filesystem access, or SSH subsystem access. An SSH protocol `shell`
request starts the in-process BBS session; every other process, subsystem, or
forwarding request is rejected.

## Caller identity

Schema 13 separates three identities without changing stable `CallerId`:

| Identity | Purpose | Visibility |
|---|---|---|
| Login identifier | SSH and future secure authentication lookup | Operator diagnostics; not normal caller attribution |
| Display handle | Normal public BBS identity | Caller lists, messages, chat/presence, file attribution, and future public UI |
| Real name | Compatibility or policy-required identity | Private by default; operator or explicit future-network use only |

For example, one caller can have login identifier `pixelwizard`, display
handle `PixelWizard`, and real name `Avery Example`. SSH maps only the login
identifier. Local SPITFIRE presentation uses the handle. A future FidoNet
adapter may require the real name; a future SMB/DOVE-Net adapter must choose
its identity through explicit adapter policy. None of those future adapters
is implemented.

Login identifiers are stored, unique, ASCII lowercase, 1 through 32 bytes,
begin with an ASCII letter or digit, and contain only letters, digits, `-`,
`_`, or `.`. Supplied SSH usernames are normalized to lowercase before exact
lookup. They are not silently tied to display capitalization. An explicit
local-operator identity mutation may rename one, but stable caller ID remains
the owner of credentials, messages, audit, and any future public key. A rename
that collides with another login or handle is rejected. Identity mutations
use the caller lifecycle state version, so a stale operator cannot overwrite a
concurrent access or identity change.

Schema-12 callers migrate transactionally. Existing display identity remains
the handle and is copied to the private real-name field so the former combined
value is not discarded. A login identifier is derived once by lowercasing the
safe ASCII form and replacing unsupported runs with a hyphen. Deterministic
caller-ID suffixes resolve collisions within the 32-byte bound; callers are
never merged. Existing message attribution snapshots are not rewritten.

New callers retain the compatible registration flow: the supplied caller name
becomes the initial handle and compatibility real name, and a unique login
identifier is derived and stored. The local operator can make the values
independent with:

```text
IDENTITY Current Handle|new-login|Public Handle|Private Real Name
```

Leave the last field empty to clear the real name. `CALLERS` shows stable ID,
login identifier, and handle, but not real name. Traditional caller-name login
continues to accept the handle so upgraded boards do not strand callers.

## Authentication and lifecycle

The SSH password callback uses the same SQLite caller credential and Argon2id
verifier as ordinary login. There is no SSH password database. On success it
creates a one-use verified grant containing caller ID and the authenticated
state version, never the password. The common session consumes that grant,
reloads the caller, rechecks lifecycle and JOKER policy, applies ordinary
board/private/subscription/time/node policy, and enters the normal post-login
journey. The caller is not asked for the same credentials again.

Disabled and Deleted callers receive generic SSH authentication rejection.
Unknown login identifiers and wrong passwords are also generic where
practical; raw supplied identities are not logged. JOKER denial reveals no
matched rule. Subscription expiry still derives effective security without
destroying base security. Every later BBS command uses M042 dispatch-time
lifecycle/security reauthorization, and lifecycle invalidation closes the
ordinary shared session rather than using SSH-specific policy.

Password authentication is the only accepted method in current source.
Public-key authentication is a future extension point associated with stable
caller ID, not handle or real name. There is no public-key management UI.

## Secondary engineering reference

Synchronet was reviewed only as a secondary engineering reference for a
modern BBS identity and SSH boundary. The useful lessons were to separate
authentication identity from public and policy-specific identities, keep
stable numeric ownership, hand an authenticated connection into the common
BBS runtime, and propagate negotiated terminal state. SPITFIRE NG did not
adopt Synchronet's caller schema, network policies, or implementation, and no
GPL code was copied. Synchronet is not an authority for historical SPITFIRE
behavior.

## Configuration and setup

SSH is disabled for existing and newly set-up boards unless the Sysop answers
Yes to `Enable SSH caller access? [no]:`. No listener is silently opened by an
upgrade. Setup then asks only for bind address and port. The nonprivileged
default is `127.0.0.1:2222`, avoiding both privileged port 22 and the host's
normal administrative SSH service.

A complete listener entry is:

```toml
[[transports]]
name = "ssh"
enabled = true
type = "ssh"
listen = "127.0.0.1:2222"
host_key = "ssh/host-ed25519"
maximum_unauthenticated_connections = 32
maximum_authentication_attempts = 3
handshake_timeout_seconds = 30

[transports.terminal]
ansi = true
cp437 = false
width = 80
height = 25
```

The host-key path must be a safe relative path below the configured SYSTEM
directory. Connection and attempt limits, handshake timeout, username length,
packet/channel queues, PTY dimensions, and input buffering are bounded. The
normal caller inactivity/time limits apply after the authenticated session
starts.

## Host key and cryptographic policy

The first enabled startup creates a board-local Ed25519 key, by default at
`SYSTEM/ssh/host-ed25519`. On Unix, the key file is mode `0600` and its
directory is mode `0700`. It is loaded unchanged on later startups and is
never committed, printed, or logged; only its public SHA-256 fingerprint is
reported. `spitfire status` shows the configured key path and either the
fingerprint or `not generated`.

SYSTEM is included in native cold backup, so restore preserves the SSH host
identity. To rotate deliberately, stop the board, make and protect a cold
backup, move the existing key to a protected recovery location, and start the
listener to generate a new key. Verify the new fingerprint through
`spitfire status` before callers accept it. Rotation legitimately causes SSH
host-key warnings and requires known-host updates.

The embedded server currently uses `russh` 0.63.1 with its `ring` backend,
password authentication, and the configured Ed25519 host key. SPITFIRE NG
does not enable SSH-1, DSA, CBC/3DES compatibility, SHA-1 compatibility, weak
MACs, or downgrade modes for vintage clients. Clients must support the
configured modern SSH algorithms; the server does not weaken its defaults to
accommodate an older client.

## PTY, encoding, nodes, and diagnostics

SSH records the declared TERM value and bounded character dimensions; it does
not invent client software identity. PTY window-change events update the
terminal capability visible to the shared paging/presentation layer. Values
matching `xterm` use ANSI with the normal UTF-8-oriented text path by default.
`ansi` and SyncTERM-like TERM values use ANSI/CP437 BBS defaults. Saved caller
terminal preferences remain authoritative. The transport never silently
decodes historical CP437 bytes as UTF-8.

SSH callers use ordinary configured nodes, maximum-node limits, duplicate
session policy, time accounting, multinode message/file locking, active-
session invalidation, status publication, and graceful disconnect. `spitfire
status` reports SSH transport, listener endpoint, login identifier plus public
handle, TERM, encoding, rows/columns, node, lifecycle/security presentation,
and menu/renderer state. It does not show real name, password, host private
key, or authentication material.

Semantic logs cover listener start/stop, accepted connections, generic failed
authentication, success by stable caller ID, PTY/resize state, established
session, and disconnect. They do not contain passwords, private keys, packet
contents, contact fields, real names, matching JOKER rules, or unnecessary
failed-login identifiers.

## Client and recovery acceptance

Automated tests use a real SSH-2 client connection to verify generic wrong and
unknown-user rejection, case-normalized login, no double authentication,
PTY/resize propagation, Main/Message/File traversal, status diagnostics,
clean disconnect, host-key stability/permissions, and protocol failure for an
`exec` request. Direct authority tests cover Disabled, Deleted, JOKER-denied,
oversized, and wrong-password callers. Existing M042 tests cover subscription
adjustment and active-session invalidation through the same common session.

The installed macOS OpenSSH client completed password authentication, Main,
the policy-safe caller-profile boundary, terminal-preference display, message
browsing, Files, Goodbye, and clean disconnect against a disposable board.
Qodem 1.0.1
in documented external-SSH mode authenticated, negotiated `ansi`/CP437 at
80×23, and reached Main, Messages, and Files; its noninteractive harness ended
at input EOF rather than claiming a clean Goodbye pass. Installed SyncTERM
1.9rc4 opened TCP but did not complete the modern SSH handshake in the tested
version and configuration; the server policy was not weakened. This result
does not make a broader claim about other SyncTERM versions or configurations.
Telnet acceptance for both clients remains unchanged.

Cold backup/restore tests preserve exact SSH configuration, host-key bytes,
schema-13 login/handle/real-name state, credentials, and caller ownership. A
restored board presents the same host fingerprint and authentication identity.
