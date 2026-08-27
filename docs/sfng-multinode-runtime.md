# SPITFIRE NG Multinode Runtime

## Purpose and Historical Basis

Increment 4 replaces the temporary Node 1 singleton with a race-safe node
pool shared by every terminal adapter. The goal is to preserve SPITFIRE's
caller-visible node identity and simultaneous-board operation without
recreating DOS process and file-locking limitations.

The preserved SPITFIRE 3.7 manual confirms:

- nodes are numbered from 1 upward and the product documents up to 255 nodes;
- the Sysop configured a current node number and total node count;
- historical multinode operation ran multiple SPITFIRE/DOS copies;
- WORK, MESSAGE, and DISPLAY could be shared, while SYSTEM and EXTERNAL were
  configured per node in the documented arrangement;
- `SFWHOSON.DAT` represented live node activity; and
- Who's On and node chat were part of the multinode caller experience.

Those findings apply to historical operation. SPITFIRE NG does not infer that
255 must be a native engine limit or that separate processes/directories are
required. Full Who's On and node chat remain follow-on parity work.

## Modern Node Model

`NodeId` is a positive `u32`, while validated configuration currently limits a
single board to 4,096 nodes as a practical resource bound. This preserves all
documented historical node numbers without embedding 255 as an unnecessary
internal limit.

Each configured node has:

- stable node number;
- enabled/disabled state;
- optional description; and
- transient state: `waiting`, `connecting`, `login`, `online`, `page-pending`,
  `chatting`, `uploading`, `downloading`, `disconnecting`, or `disabled`.

An occupied snapshot can include session ID, authenticated caller identity,
transport kind, and connection time. Passwords, supplied RLogin credentials,
and other secrets never enter node status.

## Allocation and Transport Independence

All enabled transports feed the same allocation path:

```text
Telnet / raw / RLogin / shell / serial / modem / future SSH
                            |
                            v
                 lowest available enabled node
                            |
                            v
                    common session engine
```

Transport kind does not reserve or identify a node. The allocator selects the
lowest numbered waiting node under one mutex-protected operation. Concurrent
acquisitions therefore cannot receive the same node. A lease owns that node
for the session and releases it explicitly after clean closure or implicitly
on every early error/drop path.

The current standalone `spitfire shell` command loads one board runtime for
its local process. In-process local/synthetic terminals use the same node
manager as listeners. Cross-process node coordination is not claimed; a future
service-control socket or single daemon-owned shell adapter is required before
independently launched `shell` and `run` processes can safely share one live
node pool.

## Lifecycle and Release Guarantees

The normal state sequence is:

```text
waiting -> connecting -> login -> online -> page-pending/chatting
                                  -> online -> uploading/downloading
        -> online -> disconnecting -> waiting
```

Authentication failure, input EOF, abrupt transport loss, normal Goodbye,
time-limit closure, runtime/session error, and Hayes carrier loss all end the
lease. Disabled nodes are visible but never allocated. A connection that finds
no enabled waiting node receives a bounded busy message and is disconnected;
it never enters caller authentication or consumes a session slot.

## Status Publication

The runtime publishes a versioned `runtime-status.toml` snapshot beneath the
logical WORK directory using an atomic replacement. `spitfire status` combines
that snapshot with static configuration and persistent board identity. The
file is removed on clean runtime drop. Because a crash can leave it behind,
status presents it as published/current-or-stale evidence, not an operating
system process lock.

Increment 5 adds transfer state and the caller-visible catalog filename to the
snapshot. It never publishes a physical storage or staging path. Completion,
cancellation, or transfer failure restores `online`; the existing error/drop
path still releases the node if the session itself ends.

The status command remains read-only. Increment 6's `spitfire console` runs in
the same process as listeners and adds page/chat, caller inspection, and
controlled disconnect through a presentation-independent operator service.
Attachable remote/local control still requires authenticated IPC; see
[Caller/Sysop Interaction](sfng-caller-sysop-interaction.md).

## Verification

Automated tests prove:

- lowest-node allocation and released-node reuse;
- concurrent acquisition assigns each node at most once;
- disabled-node handling and all-nodes-busy behavior;
- release after failed authentication, normal logout, EOF/disconnect, and
  synthetic serial/modem session termination;
- four simultaneous Telnet/raw/RLogin/raw callers occupy four nodes;
- a fifth caller receives the busy response;
- after one caller disconnects, a replacement acquires the released node; and
- multiple listeners of the same type remain distinguishable by name; and
- file transfers publish and clear filename-only activity while two nodes can
  download the same verified file concurrently.

Physical serial/modem hardware remains unverified. Full historical
`SFWHOSON.DAT` compatibility, Who's On, node-to-node chat, per-node
SYSTEM/EXTERNAL adapters, and multi-process coordination are unresolved.
