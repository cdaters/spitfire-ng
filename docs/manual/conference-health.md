# Conference Health

Conference Health shows how conferences are used on this BBS. It separates local
posting and reader activity from incoming network volume. An echo receiving 3,000
messages with no local reader progress deserves a different review from a local
conference with a few posts and regular readers.

Open **Conference Health** with Tab in sfmonitor. The overview shows readers,
local posts and inbound posts for 30 days. Press Enter for the evidence behind a
status, including 7-, 30- and 90-day windows, replies, active threads, last activity
and the previous 30 days. Dates are labeled UTC. No caller names or message bodies
appear in this view.

In detail, Up/Down scroll the evidence. Use `f` to cycle local/FTN/QWK/CircuitNET and status filters, `s` to switch between
readership and message-volume sorting, `i` to include retired conferences, and
Page Up/Down for pages. `r` reloads the latest stored results; it does not run an
aggregation. The display shows pending work and the time of each completed result.

## Start collecting useful results

Reader-progress tracking is enabled after upgrade. Historical messages are
reconstructed in bounded batches; historical readership is unavailable. Run a
rollup and create its recurring Event while the daemon is running:

```text
sfconfig health BOARD rollup
sfconfig health BOARD schedule
sfconfig health BOARD status
```

`BOARD` is the board configuration file, as in other sfconfig commands. `schedule`
creates an Event named Conference Health every five minutes. It refuses to replace
an existing Event with that ID. Use the [Events controls](events.md) to change its
schedule or disable it. Its action is `{"kind":"conference-health"}`. No separate
analytics scheduler runs. Larger initial backlogs can take several runs.

Settings have this order:

```text
sfconfig health BOARD configure on 365 30 off 10
```

This enables tracking, retains 365 days, uses 30 days for dormancy, leaves the
caller bulletin off and limits an enabled bulletin to ten entries. Retention may
be 180–730 days; dormancy may be 7, 30 or 90 days; bulletin size may be 1–20.
Set the fourth value to `on` to enable Hot Conferences. Disabling tracking leaves
retained history in place; re-enabling starts a new period of known coverage.
Use `sfconfig health BOARD page 32` for the next JSON page, including retired
areas. The JSON contains aggregate evidence, never individual reader identities.

Viewing requires the operator's Board Statistics permission. Direct rollup needs
Change Online Configuration. Settings and the schedule shortcut need Change
Sensitive Configuration. Existing generic Event Run Now controls retain their
Event permissions. These are local operator controls; no network peer can request
board readership statistics.

## What a reader count means

A reader is an account whose native conference read pointer advanced during the
window. Reopening an older message adds nothing. Skipping forward adds one
progress observation, not a claimed read of every skipped message. Successful QWK
packet completion can also advance the pointer; downloading a packet does not
prove that its contents were read. Pointer resets do not count as reading.

The same account counts once per window even when it advances on several days,
while its tracking identity remains. Deleting an account discards that link. If
it is restored or recreated, later activity counts as a new reader. These totals
are observations of accounts, not a census of individual people.
Public conference activity is measured; private or specifically addressed local
messages do not contribute. Before the displayed tracking start, readership is
unknown. Do not interpret that missing history as zero interest.

## Interpreting the status

| Status | Evidence |
| --- | --- |
| Locally Active | At least one local post and one reader in 30 days. |
| New / Insufficient History | Too little observed readership history for the dormancy period. This can apply to an older board just upgraded. |
| Locally Unread | Inbound messages but no reader progress during the configured dormancy period, with sufficient observation history. |
| Dormant | Neither public messages nor reader progress during that period. |
| Quiet | Other observed activity, including a conference read regularly without new posts. |
| Catching Up | Work remains; do not act on an incomplete result. |
| Disabled | Collection is disabled. |

Rising, Stable and Falling compare current and previous 30-day reader counts.
Small samples or less than 60 days of monitoring show Insufficient History.
The detail gives raw counts; there is no opaque score or content-quality judgment.
See the [technical specification](../technical/conference-health.md) for exact
window and trend rules.

A network mapping describes where an area currently connects. The Local filter
means no current mapping; it does not imply that every retained message originated
locally. Counts remain combined native totals when several mappings exist. It does not add another copy
to message counts. Incoming messages remain incoming even after forwarding;
network volume is never counted as local reading. “Retired” includes inactive local
conferences and areas whose remaining mappings are inactive/retired. Showing or
hiding them does not remove their messages or history.

A warning is a reason to review a conference, not an instruction to remove it.
Talk with callers, check mappings and consider whether an optional area is useful.
Health never deletes messages, changes Dossiers or retires an official catalog area.

## Hot Conferences

When enabled, callers choose `B` for Bulletins and then `H` for Hot Conferences.
The plain-text list ranks available public-only conferences by reader count and
shows local/inbound post counts. It is built for that caller each time. Current
native access checks exclude restricted conferences; an ordinary caller cannot
see Sysop-only areas merely because those areas exist in a network catalog.
No shared static bulletin can retain a now-restricted conference's statistics.
While projection work is pending, results are withheld; an initial
backlog can leave the bulletin empty until rollup catches up.

## Retention and recovery

Daily anonymous reader observations and small message projections are retained
for the configured period. Account deletion removes the account-to-token lookup;
retained aggregate readership still counts those observations. Individual reader
lists are not exposed. Nothing is sent to an analytics service.

Normal board backup/restore includes settings, observations, pending work and
completed results. Restore the board normally, then resume its Event. Repeating a
rollup replaces projections and cannot add another read. Clock rollback suppresses
future activity until its time is reached. Retention cleanup is bounded and may
need subsequent Events after a long shutdown. Read progress also removes one
expired daily observation at a time, preventing continued storage growth if no
rollup Event is scheduled. Idle expired rows wait for activity or rollup. Results are local operational
information, not an audit of who read an individual message.
