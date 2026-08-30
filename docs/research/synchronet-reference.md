# Synchronet Reference Corpus

## Purpose and Authority

A separately held Synchronet source snapshot was reviewed as a secondary
comparative engineering source for mature BBS design and implementation
patterns. It is not distributed by this repository.

It does **not** define SPITFIRE NG caller-visible behavior. The authority order
is:

1. original SPITFIRE documentation and artifacts;
2. accepted SPITFIRE NG compatibility, architecture, security, and decision
   documents; and
3. Synchronet as a comparative implementation reference.

This note is intentionally incremental. It records only material actually
inspected, not a claim that the corpus has been fully reviewed.

## Provenance and License

The local archive's root `LICENSE` file points to Synchronet's official
copyright page rather than embedding the complete terms. The official page
states that Synchronet Version 3 is GPL-covered, except XPDEV, SMBLIB, CIOLIB,
UIFC, and XSDK, which are LGPL-covered. Synchronet Version 3 is not public
domain.

Increment 3 used the corpus only for read-only comparative research. No
Synchronet source was copied, translated closely, linked, or committed. Any
future direct reuse requires a deliberate license review and an explicit
decision before implementation.

Official source: [Synchronet Copyright](https://synchro.net/copyright.html).

## Increment 3 Material Inspected

- `src/smblib/smbdefs.h` — stable message number versus storage structures,
  explicit private/deleted/read attributes, reply/thread links, message IDs,
  and last-message status.
- `src/sbbs3/readmsgs.cpp` — storage retrieval separated from display,
  privacy filtering, scan pointers/high-water state, and thread traversal.
- `src/sbbs3/postmsg.cpp` — reply linkage, private-message handling, recipient
  identity fields, and backend insertion.
- the root archive `LICENSE` pointer and the official license page.

The earlier RLogin-specific review remains separately documented in
[SyncTERM / Synchronet RLogin Auto-Login](syncterm-rlogin-autologin.md).

## SSH and caller-identity reference boundary

Synchronet was reviewed as a secondary engineering reference for modern BBS
identity and SSH architecture. Useful general lessons were identity
separation, stable numeric ownership, a bounded authenticated SSH handoff, and
terminal-state propagation into the ordinary session runtime.

SPITFIRE NG did not adopt Synchronet's caller schema, multi-name login rules,
SFTP behavior, packed storage, network-specific identity policy, or logging
semantics. Synchronet is not an authority for historical SPITFIRE behavior,
and no GPL code, data layout, constants, or test data were copied.

## Increment 4 Material Inspected

- `src/sbbs3/scfg/scfg.c` — hierarchical configuration entry points organized
  around implemented system areas rather than one undifferentiated settings
  form.
- `src/sbbs3/scfg/scfgnode.c` and `src/sbbs3/scfg/scfgsrvr.c` — separation of
  per-node settings from shared server/listener configuration and explicit
  service enabled/interface/port controls.
- `src/sbbs3/nodedefs.h` and `src/sbbs3/node.c` — explicit runtime node states,
  status records, and allocation/status responsibilities distinct from caller
  application behavior.
- `src/sbbs3/qtmonitor/nodewidget.cpp` and `src/sbbs3/main.cpp` — operator
  monitoring as a runtime concern separate from static configuration.

This was read-only comparative study. No code, command layout, status-file
format, or caller-visible Synchronet semantics was copied.

## Increment 5 Material Inspected

The ignored `sbbs_src.tgz` archive was inspected in place through bounded tar
output; it was not extracted into the repository.

- `src/sbbs3/filedat.h` — separation of file metadata identity, directory
  identity, paths, sizes, hashes, descriptions, and transfer accounting;
- relevant portions of `src/sbbs3/file.cpp`, `upload.cpp`, and `download.cpp`
  — path/filename checks, distinct transfer queues, temporary upload handling,
  post-transfer validation, success-only accounting, and presentation versus
  storage boundaries;
- `src/sbbs3/xmodem.c/.h` and `zmodem.c/.h` archive entries — evidence that
  mature binary transfer support is a substantial protocol subsystem with
  dedicated state and error handling, not a safe incidental terminal parser;
  and
- the corpus `FILE_ID.DIZ` — an example of description metadata as archive
  content, reviewed only to understand the generic extraction workflow.

The reviewed source carries GPL-2.0-or-later headers where inspected. No code
or data was copied, translated, linked, or used as committed fixture content.
Increment 5's Rust implementation was derived from the SPITFIRE manual and the
project's own design.

## Increment 6 Material Inspected

The archive was again read in place without extraction or modification.

- `src/sbbs3/nodedefs.h` — distinct node status and node action values,
  including logon, online activity, paging, local chat, and transfer states;
- `src/sbbs3/umonitor/chat.h` and `chat.c` — a Unix operator-chat client that
  targets one node, preserves/restores operator presentation state, observes
  caller departure, and keeps operator presentation separate from the caller
  session;
- `src/sbbs3/scfg/scfgchat.c` — configured page/notification mechanisms and
  chat configuration as their own administration concerns; and
- the inspected GPL-2.0-or-later source headers and archive license pointer.

The generic lesson adopted is to represent current node action explicitly and
to target a stable live session from a presentation-independent operator
service. SPITFIRE NG did not copy Synchronet's packed node record, chat files,
split-screen UI, external pager commands, control flags, or command keys.

## Binary-Transfer Increment Material Inspected

The official [reference index](https://wiki.synchro.net/ref:index),
[XMODEM](https://wiki.synchro.net/ref:xmodem),
[YMODEM](https://wiki.synchro.net/ref:ymodem), and
[ZMODEM](https://wiki.synchro.net/ref:zmodem) pages were reviewed. The
[SyncTERM manual](https://www.syncterm.net/Manual.html), installed SyncTERM
1.9rc4, and current SyncTERM 1.10a built from unmodified upstream commit
`dc5eb88e3852dfa673c7c72ab5df955b89a21dbc` established the exposed client
choices and external acceptance workflow. The local GPL transfer source
remained comparative material only.

Useful lessons were narrow protocol/session ownership, keeping file trust and
catalog mutation outside wire engines, layered ZDLE and Telnet escaping,
bounded state-machine retries, batch members as distinct files, and
success-only accounting. No Synchronet/SyncTERM source was copied or closely
translated. SPITFIRE's manual still defines names, selection, authorization,
and caller workflow.

Third-party Rust evaluation selected `zmodem2` 0.7.2 (MIT OR Apache-2.0): it
provides safe caller-driven sender/receiver state machines, CRC-16/CRC-32,
ZHEX/ZBIN/ZBIN32, ZDLE, ZRPOS, and batch support over an arbitrary byte stream.
`ymodem` 0.1.1 was too narrow/stale, `rmodem` 0.1.1 was incomplete and AGPL,
`rzsz` 0.1.4 was Unix-fd/filesystem oriented rather than a clean library fit,
and `txmodems` left Y/Z work incomplete. X/YMODEM and Telink were therefore
implemented independently from specifications; no external `sz`/`rz` process
is a production dependency.

Actual SyncTERM 1.9rc4 Telnet ZMODEM upload and download passed with displayed
CRC-32, exact size/bytes/SHA-256, successful statistics, and clean return to
Files. Current SyncTERM 1.10a then passed XMODEM checksum/CRC download,
XMODEM-128 upload under both observed receiver-requested integrity modes,
XMODEM-1K upload, YMODEM single/batch in both directions, and YMODEM-g
download. Temporary Wren bindings invoked the client's public transfer API;
the protocol source was not modified or copied.

The live YMODEM-g run established that this receiver answers metadata block
zero with `G` directly (not ACK then `G`) and does not ACK the empty batch
terminator. The current SyncTERM code also explains the remaining real ZMODEM
batch blocker: automatic ZRINIT dispatch invokes its single-file upload picker,
despite a separate batch choice in the ordinary UI. That is recorded as a
client dispatch limitation, not a synthetic batch success or a SPITFIRE
caller-visible semantic.

## A-027 Paging Comparison Material Inspected

This bounded comparison followed—not preceded—the primary SPITFIRE A-027
conclusion. `src/sbbs3/getkey.cpp` implements a `pause()` operation that prints
a configurable pause prompt, treats its configured Quit key or the global
Ctrl-C abort state as abort, treats ordinary input as continuation, and lets
Down Arrow request one additional line. `text_defaults.c` supplies a generic
`[Hit a key]` default prompt. The implementation therefore confirms that a
mature modern BBS can keep paging continuation and output abort distinct and
that Q-style abort is conventional.

None of those keys was used as evidence for SPITFIRE. SPITFIRE's preserved
executable independently establishes S=Stop, N=Nonstop, and Enter=continue.
Synchronet only supports retaining Q as an undisplayed modern alias; its
Ctrl-C/global abort state, Down Arrow behavior, configurable text system, and
global status flags were not adopted.

The durable future reference map now includes ANSI, XBin, node status,
FOSSIL, `CALLINFO.BBS`, `DOOR.SYS`, `DOOR32.SYS`, `DORINFO1.DEF`, QWK and
Synchronet QWK network extensions/remote commands, FidoNet extensions/files/
packets, and SMB. In particular, the
[SMB specification](https://wiki.synchro.net/ref:smb) is a future source for
the planned SMB backend and DOVE-Net work; it does not change today's SQLite
message/file stores.

## Useful Comparative Lessons

The following are generic sound BBS-engineering patterns also supported by
SPITFIRE evidence and the existing SPITFIRE NG architecture:

- keep stable storage identity distinct from caller-facing message numbers;
- enforce private visibility below presentation/menu code;
- keep message storage and session presentation separate;
- model last-read/high-water state per caller and message area;
- retain explicit durable reply relationships even when the historical UI
  presents threads primarily by subject; and
- avoid forcing transport or network metadata into the local caller-facing
  message model;
- organize a configuration UI by working BBS subsystems while keeping one
  authoritative validation/service layer beneath every UI;
- distinguish static node definitions, shared board services, and transient
  live node state; and
- configure services by enabled state and explicit interface/port rather than
  binding node identity to a protocol listener;
- keep file metadata and physical-path resolution behind a narrow service
  boundary rather than constructing paths in the menu/session layer;
- treat upload receipt, validation, duplicate detection, description
  extraction, and final catalog insertion as separate stages;
- update caller/file statistics only after a transfer has completed; and
- keep transfer protocol state/results separate from file authorization and
  storage so another protocol does not rewrite the file domain;
- distinguish a node's broad connection status from its current action, such
  as paging, chat, upload, or download; and
- keep operator presentation separate from page/chat coordination and target
  a stable session identity rather than trusting a reusable node number.

Increment 3 adopted these generic boundaries only where they also match the
SPITFIRE manual and accepted SPITFIRE NG design. The implementation is original
Rust code based on the project's own specification.

## Deliberately Not Adopted

- Synchronet command keys, prompts, scan modes, user policy, and caller-visible
  message behavior do not replace documented SPITFIRE behavior.
- SMB is not implemented in Increment 3.
- Synchronet's full flag set, polling/voting, networking, moderation, and
  thread UI are not projected into the small stock-core model.
- Synchronet user IDs or RLogin identity are not treated as SPITFIRE caller
  authority outside the separately documented optional compatibility mode.
- Synchronet SCFG labels, node-record formats, node limits, service process
  model, and monitor controls do not define SPITFIRE NG configuration or
  historical parity.
- Synchronet file-library numbering, command keys, transfer queues, protocol
  defaults, filename policies, and FILE_ID.DIZ semantics do not replace
  documented SPITFIRE behavior.
- GPL X/Y/ZMODEM source was not copied or closely transcribed. A future binary
  protocol implementation requires an explicit maintained-library and license
  evaluation.
- Synchronet's packed node data, local chat files, split-screen operator UI,
  page programs, and node flags were not adopted. SPITFIRE NG uses an original
  Rust in-process interaction service and SPITFIRE terminology/resources.

## Open Research Leads

- Review SMBLIB's exact locking and durable numbering behavior before designing
  the planned SMB backend.
- Compare SMB private/read receipt semantics with SPITFIRE's non-public and
  received-message behavior before mapping flags.
- Study Synchronet's high-water and per-sub-board scan state only when the
  complete SPITFIRE current/all/queued scan workflow is implemented.
- Inspect message import/network boundaries during the later SMB, QWK,
  DOVE-Net, FidoNet, or CircuitNet milestones, each under its own scope and
  license review.
- Find a practical real-client ZMODEM multi-file path and external
  Telink/1K-XMODEM-g and YMODEM-g upload/batch peers without treating GPL
  reference source as reusable project code.
- Revisit bounded archive metadata/DIZ extraction only when the stock archive
  view and upload-validation follow-up is active.
- Revisit mature operator/control IPC designs when making the console attach to
  an already-running server or joining separately launched shell sessions to
  that server's node pool.
