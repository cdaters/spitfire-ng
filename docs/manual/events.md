# SPITFIRE NG Events

An Event tells the daemon when to perform a native action. A caller's message is
stored immediately. Its durable preparation obligation is recorded in that same
commit; CircuitNET and FTN scanners promptly prepare eligible outbound work while
the daemon runs. The exchange Event decides when a connection is attempted. Saving
a message does not mean that a neighbor has received it.

Historical SPITFIRE called these jobs Events. NG keeps that term and the familiar
mail-run workflow, with typed internal services in place of SF.BAT and modem scripts.
The operational event log is a record of activity, not a second scheduler.

## Choosing exchange behavior

| Policy | What happens |
| --- | --- |
| Immediate | New durable activity prompts an exchange, with minimum spacing. Successful sessions continue draining additional batches; a failed finite Poll needs new activity or Run Now. |
| Scheduled | Work prepares promptly, but automatic connections wait for the Event's due time. Useful for an arranged daily window or regular polling. |
| Manual / Poll Only | Work prepares promptly. Only Run Now or an explicit operator Poll initiates outbound exchange. |
| Hybrid | Prompt exchange plus recurring catch-up, useful for an always-on link and for END nodes that must poll to receive HOST traffic. |

Existing installations have no implicit Events and retain manual outbound polling.
Create a Hybrid Event deliberately after testing the link. An inbound authenticated
Poll may still exchange queued traffic under any policy: these policies govern
outbound initiation, not a promise to refuse an incoming neighbor until a window.
Use hold or listener/peer policy when a link must be unavailable in both directions.

## Manage Events with sfconfig

The daemon must be running. These commands use the authenticated local operator
channel; they never open a competing scheduler or edit live SQLite.

```text
sfconfig events BOARD list
sfconfig events BOARD save event-a.json
sfconfig events BOARD enable event-a
sfconfig events BOARD disable event-a
sfconfig events BOARD run event-a
sfconfig events BOARD history event-a
```

`save` adds or edits a definition. This is the bounded expert Events surface.
`Run Now` requires network-run permission; save and enable/disable require sensitive
configuration permission. Running Events cannot be edited. List shows version,
enabled state, next due time, current run, last result and consecutive failures.
History shows the last 50 runs, their original action, trigger, timestamps and
result. Times in machine-readable output are UTC Unix seconds; the definition's
IANA timezone determines daily wall times. sfmonitor displays UTC explicitly.

Example `event-a.json` (replace synthetic profile/neighbor IDs with your configured
ones, and use your board's configured timezone):

```json
{
  "id": "event-a",
  "name": "CircuitNET Mail Run",
  "enabled": true,
  "action": {"kind": "circuitnet", "network": "circuitnet-example", "node": "HOST1"},
  "schedule": {"kind": "interval", "seconds": 900},
  "timezone": "America/Phoenix",
  "policy": "hybrid",
  "missed": "run-once",
  "minimum_spacing_seconds": 30
}
```

Use `node: null` for all enabled outbound neighbors in the profile. Held peers
remain held. For BinkP, use `{"kind":"binkp","link":"configured-link-id"}`.
Its existing FTN authentication, queue claims, backoff, FileEcho and FREQ services
remain authoritative. The Event does not redesign or bypass them.

For a daily 02:00 mail run:

```json
{"kind":"daily","hour":2,"minute":0,"days":[0,1,2,3,4,5,6]}
```

For Monday–Friday at 03:30 use hour 3, minute 30 and days `[1,2,3,4,5]`.
Sunday is zero. For no automatic recurrence use `{"kind":"manual"}` with policy
`manual` or `immediate`. Interval bounds are 5 seconds through 31 days; short
intervals are intended for isolated testing. Normal network intervals should be
agreed with the neighboring Sysop. Minimum spacing is 5 seconds through one day.

QWK network build/ingest and custody handoff remain explicit operator operations.
There is no automatic QWK network connection service to schedule. Caller QWK packet
download/upload remains caller-driven. Backup remains a cold-board operation;
a warm, consistent snapshot API is needed before Backup can become an Event.
Startup/once-only schedules and external commands are deferred. There is no shell
interpolation or command runner in the Event model.

## Run Now and monitoring

In sfmonitor Networks, press **E** for Events. Select an Event, press Enter for
its details, or R to request Run Now through the normal confirmed operator action.
Repeated requests coalesce with a pending or active run; disabled Events do not run.
One enabled Event owns each concrete target; overlapping all-peer and single-peer
definitions are rejected. A single worker runs Events serially, and existing per-link session permits protect
against overlap with incoming sessions and manual Polls. A busy Event reports Busy;
Hybrid/Scheduled recurrence provides the next opportunity.

CircuitNET Networks retains peer health, queues, pending remote subscriptions and
Dossier controls. `sfconfig circuitnet BOARD live-status PROFILE` also reports each
peer's next scheduled exchange. `sfconfig circuitnet BOARD why PROFILE NODE` provides
a safe link explanation: disabled, held, running, peer-health attention, scheduled
wait, or no queued work. It does not read or print message bodies.

## Downtime, clocks and recovery

`missed: "run-once"` catches up once after downtime; it never runs a week's backlog.
`missed: "skip"` advances missed slots to the next future one. A run advances its
next slot before starting network work. An interrupted run is recorded as such;
restart and restore clear running authority. Network receipts decide what still
needs delivery. Restored uncertain network work retains the existing review holds.

Daily schedules use the named timezone. A nonexistent spring-forward time is
skipped. A repeated fall-back time runs at its first occurrence only. Interval
schedules use persisted UTC times and count from the latest start, including Run Now.
Clock rollback does not repeat a claimed slot;
a forward jump causes at most one catch-up. Editing a definition recomputes its
future schedule. A board timezone change does not silently rewrite Event timezones.

The daemon checks durable preparation work independently of the exchange worker.
Scheduler wakeups are bounded; Run Now/configuration wakes it directly. Actions use
existing finite network session deadlines and shutdown cancellation. During graceful
shutdown no new Event is admitted, and active work drains under the daemon's
existing bounded shutdown rules. Pending work is never deleted because a peer is
unavailable. Use ordinary cold backup/restore to preserve definitions and history.

## Why didn't this message move?

1. Confirm the native message was saved and the conference is active, public and
   mapped to the intended network codename, with send enabled and valid posting
   identity policy. A local/private access-restricted delivery is not a public
   CircuitNET publication.
2. For normal broadcast, confirm the direct neighbor subscribes in its Dossier.
   For directed traffic, use Route Test and confirm the destination has an active
   receive mapping. Directed intent must be fixed before publication; the native
   atomic directed-post API is safe while automatic preparation runs. The older
   operator `direct` command can select only a still-unpublished native message;
   use `sfconfig circuitnet BOARD stage-direct PROFILE MESSAGE_ID NODE` while the
   daemon is stopped for that two-step workflow. Already-published messages cannot
   have their route changed.
3. Check queue state and its retained reason: held policy, exhausted attempts,
   pending, ready, retry, or already accepted. A completed queue is not missing mail.
4. Check Event enabled state, policy, next due, spacing and last result. Manual needs
   Poll/Run Now; Scheduled waits for its slot; Hybrid catches up after failures.
5. Check the peer's enabled/outbound/hold state, authenticated identity and last
   health result. Test Link diagnoses admission without sending conference traffic.
   An online inbound listener alone does not initiate an END's outgoing poll.
6. Check pending remote Dossier approval and the typed result at the requester.
   Approval requires another exchange for the result to arrive; use Hybrid or a
   recurring Scheduled Event for unattended inbound collection.
7. After restore, review uncertain queue holds and release only through the existing
   policy-valid retry workflow. An Event cannot bypass them.

Transport encryption protects traffic while traveling between configured CircuitNET
nodes. Conference messages are readable according to destination BBS access rules.
Directed routing chooses a node; it does not make the message private or end-to-end
encrypted.
