# M067 — SPITFIRE Events and network scheduling

This rights-safe summary paraphrases retained primary documentation. No original
manual, batch file, communications script, private research inventory or production
configuration is included. C1 remains historical authority. This was workflow
research, not additional CircuitNET binary archaeology.

## Historical findings

DOCUMENTED: SPITFIRE 3.7 offered thirteen Event slots A–M. M was reserved for
message packing. Its own Event state recorded clock time, weekdays, enabled state
and completion for the day. The supplied SF.BAT dispatched A–L through ERRORLEVEL
22–33, invoked operator programs and returned to the BBS loop. Ordinary Events
waited for logoff; on-time Events could shorten caller time, with documented limits.
Frontend operation could move scheduling responsibility outside the ready prompt.

DOCUMENTED: CircuitNET separated preparation (PRIMER/EXTRACT), external transfer,
and import. Dependent nodes normally called their HOST at an arranged mail window;
a HOST could similarly call its parent. Telix and Qmodem examples automated calls;
DSZ served external transfers. Combined and separate send/receive calls were
possible. MAILCALL selected a prepared HOST packet for the logged-in dependent and
invoked the transfer program; it was neither a scheduler nor a packet builder.

DOCUMENTED: retained examples differ in retry limits. Unsent packets were preserved
and successful copies could support resend. INFERRED: overnight mail/maintenance
windows were an ordinary intended workflow. UNKNOWN: a universal nightly frequency,
a general durable downtime catch-up contract or universal upstream-initiated polling.
Earlier initialization-hook examples import already-present packets; no separate
clock was found in those examples. Historical hook spelling differences remain
research uncertainty, not a newly implemented compatibility alias.

## Native NG reconciliation at the C4 checkpoint

| Service | Work preparation | Exchange trigger before C5 |
| --- | --- | --- |
| Caller QWK | Caller packet request | Caller download/REP upload |
| Network QWK/DOVE | Native build and publication decisions | Operator build/ingest and confirmed custody handoff |
| FTN NetMail | Native save commits outbound intent | Existing export/BinkP Poll or inbound session |
| FTN EchoMail | Existing native scanner | Explicit scan/export/Poll |
| BinkP and existing FTN FileEcho/FREQ | Existing native queues, claims and receipts | Explicit Poll or authenticated inbound session |
| CircuitNET offline | Native scan/export | Explicit import/receipt/ACK |
| CircuitNET live | Poll preparation; imported transit queues atomically | Explicit Poll or symmetric authenticated inbound exchange |

No generic timed scheduler or recurring outbound poll existed at that checkpoint.
Queue retry eligibility did not initiate a connection. Listener and session timers
served other purposes. QWK has no live connection boundary to schedule; backup
requires an exclusive cold-board lock. No production Synchronet inspection was
needed or performed. No production changes or external BBS polling occurred.

## C5 decision

Preserve the term Event and visible mail schedules. Use typed daemon services in
place of DOS batch/errorlevel dispatch. Prepare native work independently from
exchange timing, retain protocol/queue retry authority and provide one generic
recurrence service. See the [Events contract](../technical/events.md),
[manual](../manual/events.md) and [C5 report](m068-circuitnet-operations-events-distribution.md).
