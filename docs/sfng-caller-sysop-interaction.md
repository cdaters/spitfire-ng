# SPITFIRE NG Caller/Sysop Interaction

This document is the canonical implementation and compatibility specification
for Stock SF37 Core Parity Increment 6: caller terminal preferences,
screen-oriented output, Sysop paging/chat, and the essential live operator
surface. Read it with the [stock parity checklist](stock-spitfire-3.7-parity.md)
and [multinode runtime specification](sfng-multinode-runtime.md).

## Caller profile and product identity

The 2026-08-21 caller-policy closure adds Main-menu `R` for the authenticated
caller's private profile and `A` for About/Credits. Profile collection and
editing share `CallerProfilePolicy`; each address, phone, email, and birth-date
group is disabled, optional, or required. Only the caller's own display
context may expand historical contact macros. Operator `PROFILE` and
`PROFILE-SET` expose enabled values only through the authorized operator
console and never reveal credential hashes.

Startup now separates the configured board name, the configured Sysop and
acquired node, and the software identity `SPITFIRE NG Bulletin Board System`
with the Cargo package version. About/Credits attributes the Rust project to
Craig Daters and contributors and separately credits the original SPITFIRE
BBS, Copyright (C) 1987-2010 by Mike Woltz, Buffalo Creek Software. It states
that SPITFIRE NG is an independent reimplementation and not an official
Buffalo Creek release.

About stays in the ordinary paged output path and adds an explicit Enter
acknowledgment before the ANSI Main menu redraw can clear it. Aborting the
About output unit skips that acknowledgment. The change does not alter welcome
artwork controls, menu command input, or binary transfer ownership.

## Historical Behavior Established

**Confirmed from the SPITFIRE 3.7 manual:** Main-menu `P` pages the Sysop. An
unavailable page uses `SFPGOFF`, an unanswered page uses `SFUNANS`, and a
duplicate outstanding page uses `SFPAGED`. The local operator could toggle
page availability and answer caller-initiated pages by entering chat. SPITFIRE
used `USERINIT` when that chat began and `CHATDONE` when it ended. Caller time
continued during caller-initiated chat. A Comment to Sysop was a distinct,
persistent message workflow and is not interactive chat.

The stock caller record also held terminal choices including graphics, display
width (40–144), page length (10–24), More prompts, scroll prompts, and hot-key
mode. Manual §5.7 documents `PROMPTOFF`, `NOABORT`, `ABORTON`, and `PROMPT` as
separate display controls. Section 8.2 says a completed page displays a More/
Continue prompt, pauses until caller input, and uses the caller's configured
10–24-line page length.

The preserved shareware `SPITFIRE.EXE` supplies the caller-visible key evidence
that the prose omits. Three adjacent Pascal strings beginning at file offset
`0x211E0` compose `MORE: <S>top, <N>onstop, < ENTER > to continue?`. This is a
context-specific paging contract, not a universal BBS abort table: S stops the
display, N requests Nonstop output, and Enter continues page by page. The
prompt does not state whether Nonstop mutates the saved caller preference;
SPITFIRE NG deliberately limits it to the current output unit because the
manual treats saved More preference as a separate caller setting. Other
documented contexts use their own keys—for example, message browsing names Q
and node chat names Escape.

The preserved `DISPLAY.ZIP` reinforces the separation. Supplied menu displays
start with `PROMPTOFF`, many ANSI displays also use `NOABORT`, and the supplied
Goodbye displays end with an explicit `PROMPT`; none contains a visible Q or
Escape MORE legend. All 55 parsed `SPITFIRE.HLP` records likewise contain no
general paging-key table. This artifact evidence resolves A-027 without
claiming a universal abort key.

**Modernized:** PC-speaker alerts, direct local keyboard function keys, direct
video-memory split screens, and RAM-drive chat files are implementation
mechanisms rather than SPITFIRE identity. SPITFIRE NG exposes an operator event
and line-chat API over safe in-process channels. Its MORE prompt preserves the
stock S/N/Enter choices and additionally accepts Q or Escape as undisplayed
aliases for Stop. Q is familiar to mature BBS paging implementations and
Escape is already a context-specific SPITFIRE cancellation convention; neither
alias is represented as an original MORE key.

## Persistent Terminal Preferences

SQLite schema 5 adds these caller-owned values:

- graphics: `auto`, `ansi`, or `text`;
- optional explicit screen width, validated to 40–144;
- optional explicit page length, validated to 10–24;
- More prompt, scroll-prompt, and hot-key flags.

Transport capability and caller choice are separate. ANSI is emitted only when
the transport reports ANSI support and the caller has not selected text. An
explicit caller width/page length overrides negotiation. In automatic mode,
the negotiated dimensions are used; when unavailable, the native defaults are
80 columns and 24 lines. The caller changes one preference at a time with Main
menu `U`; validation and persistence use the caller/database domain APIs rather
than menu-only checks.

Hot-key preference is preserved now, but complete one-keystroke stock input is
still a fidelity follow-up because the current common terminal command boundary
is line-oriented.

## Paging and Chat State

`InteractionHub` is an in-process coordination service keyed by stable
`SessionId`, not node number. A node may be reused only after its owning session
ends, so a stale page cannot attach to a later caller on the same node.

The state sequence is:

```text
online -> page-pending -> chatting -> online
```

The Sysop may be available or unavailable. A page is bounded by a timeout and
can be answered or declined. Two callers can page independently. Answering
creates paired caller/operator chat handles with bounded line exchange. Either
side can end chat; disconnect cleanup removes the page/chat slot and the node
returns through normal disconnect/release handling.

## Operator Surface

`spitfire console <CONFIG-FILE>` starts the configured listeners and the
portable terminal operator console in the same process. Its implemented
commands cover:

- node/status inspection;
- page listing, availability, answer, decline, and chat;
- controlled session disconnect;
- caller listing without credentials;
- caller enable/disable and numerical security changes;
- clean console/server shutdown.

These commands use `OperatorService`, which is presentation-independent so a
future web operator interface can reuse the same authorization and runtime
operations. Password hashes and supplied transport credentials are never
shown.

## Caller-facing Sysop navigation boundary

The final A-022/A-025 closure adds the stock caller-facing Sysop section
without turning it into the complete maintenance system. The generated Main,
Message, and File menus expose their documented `@` command only at the
configured Sysop security. Their immutable historical identifiers enter the
same `MenuSection::Sysop`; a valid `SFSYSOP.MNU` controls visible commands and
`SOP<security>.CLR`/`.BBS` supplies optional exact-security artwork. Sysop `Q`
returns to Main, `G` uses the common Goodbye path, and `X` changes only the
active session.

Missing Sysop artwork falls back to the security-filtered menu. Missing or
malformed `SFSYSOP.MNU` falls back to a narrow `Q`/`X`/`G` menu so boards
created before the resource was generated remain usable. An advanced command
present in a supplied historical menu receives a deterministic unavailable
message and stays in the Sysop context. This boundary grants no host shell,
printer access, event maintenance, caller/message packing, log inspection, or
other maintenance operation; those remain B/C/D work in the parity ledger.

The ordinary `spitfire run` process remains non-interactive. A separately
launched `spitfire shell` process also still owns an independent in-process node
pool. Resolving cross-process control and shell participation should use an
authenticated local control/IPC design; fragile filesystem locks are not the
recommended direction.

## Output Paging and Abort

`PagingTerminal` wraps the existing transport-independent `Terminal`. Resource,
help, menu-fallback, message, and file presentation begin explicit output
units. The wrapper counts rendered lines, prompts at the effective page height,
and presents the preserved `S`top / `N`onstop / Enter choices. Stop suppresses
only the remainder of the current unit. Nonstop suppresses further prompts only
for that unit; the next unit restores the caller preference. Q and Escape are
modern Stop aliases. The paging response does not consume a subsequent menu
command. Aborting a message before it is fully presented does not advance
last-message-read state.

The wrapper does not belong to SQLite, file storage, message storage, or a
specific network protocol. Telnet, raw TCP, RLogin, shell, direct serial, and a
modem-established session therefore receive the same behavior.

The renderer already carries negotiated terminal type, ANSI/CP437 support,
rows, and columns plus caller overrides. It counts emitted line endings within
one output unit; it is not a full ANSI cursor-position emulator. The
caller-experience audit found no primary evidence requiring that larger model.
Decorative displays retain resource-controlled paging, while interactive
information uses this bounded output-unit behavior and an owning workflow may
hold before a clearing return transition.

The message-interaction closure additionally implements the manual's named-
Sysop preview rule. Identity is matched to the configured Sysop caller name,
not granted merely by a high security number. Previewed messages use the same
backend visibility checks but do not advance last-read or create a direct-
message receipt. The canonical queue/receipt and “Your Messages” behavior is
in [Native SPITFIRE NG Message System](sfng-message-system.md).

## Evidence and Verification

**Automated:** schema-4-to-5 migration, preference ranges/persistence and
capability precedence, MORE Enter/S/N behavior, current-unit reset, Q/Escape
aliases, `NOABORT`, no stale command input, transfer isolation, concurrent
pages, timeout/decline, chat exchange, stale-session rejection, node-state
restoration, operator caller changes, About MORE/end acknowledgment at 80×10,
and a clean setup-created board journey.

**Integrated:** a board created through the normal setup service initialized a
Sysop, four nodes, conferences, file areas, listeners/resources, and then ran
login, caller creation, terminal preference change, message read/reply,
Comment-to-Sysop, file list/download/upload, live page/chat, logoff, reconnect,
and persistence checks through the real session engine.

**Hardware-unverified:** physical direct-serial and Hayes hardware. Their
common session boundary remains PTY/simulation-tested.

## Known Fidelity Gaps

- no universal historical output-abort key is claimed or required; preserved
  primary evidence establishes the context-specific MORE choices;
- stock hot-key mode now executes menu commands immediately on network and
  serial adapters; Unix stdio deliberately retains line input under ordinary
  canonical terminal discipline, and non-menu/binary input is isolated;
- the Category-A display/navigation journey and confirmed macro table are
  complete; advanced event/questionnaire/bulletin resource breadth remains
  tracked under B-003/B-004/B-006/B-020;
- hidden forced chat remains deferred; current B021-B2 instead implements
  operator invitation with caller consent and ordinary-time pause/resume;
- chat capture, caller-to-caller node chat, page schedules, and external page
  mechanisms are follow-on stock/advanced work;
- `spitfire console` is an in-process operator mode, not an attachable control
  client for an already-running server.

## Six-Month Recovery

Current B021-B uses the existing InteractionHub and caller/session authority
for page availability, pending-page answer/decline, consent-based operator
chat, and graceful disconnect. Chat is bounded, authenticated, exact-session
bound, and non-persistent; normal completion restores the caller's prior
context. Operator loss ends chat without ending a valid caller session.
Individual disconnect notice/no-notice shares transfer/accounting cleanup.
Daemon shutdown uses that same lifecycle with a distinct board-shutdown
notice and bounded drain, not a second interaction system. See the
[B021-B2 report](research/m039-tranche-7-b021b2-chat-disconnect.md) and
[B021-B3 integrated report](research/m039-tranche-7-b021b3-shutdown-integrated.md).

The caller/session engine, not a transport adapter, owns preferences, MORE,
page, and chat behavior. Start with this document, then inspect
`crates/sf-core/src/terminal.rs`, `interaction.rs`, and `session.rs`, followed by
`crates/sf-bbs/src/operator.rs` and `runtime.rs`. Schema 5 is authoritative for
caller preferences. Before changing input semantics, re-check the original
manual and keep any unresolved historical key behavior explicitly labeled.
The resource precedence, macro table, help mapping, and immutable menu-ID
contract are canonical in `06-legacy-file-formats.md`.
