# Native Conference Health

Conference Health is a derived, local projection of native conferences, public
messages and last-read progress. It is not message authority, a network analytics
service or moderation scoring. [Historical context](../research/m073-conference-health-sfcnfuse.md)
and the [operator manual](../manual/conference-health.md) explain its purpose.

## Durable interface and sources

Schema 36 adds one generic projection family:

* `conference_health_config`: settings, monitoring start, bounded seed cursor and
  last rollup time.
* `conference_health_work` / `conference_health_dirty`: invalidated message IDs and
  conference IDs. These are durable obligations, not counters of posts.
* `conference_health_messages`: one retained public native delivery per message ID,
  placement time, local/inbound flag, reply/thread information and optional opaque
  local-poster token. No body, subject or author display name.
* `conference_health_tokens`: random 128-bit opaque account token lookup. Native
  account deletion destroys the link. Anonymous retained observations remain.
  Restoring/recreating an account assigns a new token on later activity; it can
  count again in the same window. Readership and local-poster counts are not a
  deduplicated census of people across account deletion/restoration.
* `conference_health_reads`: one conference/day/token row with progress count and
  most recent time. This supports distinct retained tracking identities across windows,
  rather than incorrectly summing daily uniques.
* `conference_health_snapshots`: cached aggregate evidence by native conference.

`RuntimeDatabase::conference_health_rollup` is the sole projection writer.
Native SQL triggers enqueue corrections inside the original message transaction.
Read triggers observe strict high-water advances to active public/all-callers
messages. Rereads, backward movement and explicit reset-version changes do not
count. Native online reads and committed QWK download pointers share this source;
these are progress observations, not exact reads, dwell time or human attention.
Private/local-recipient messages are excluded. Historical messages may be seeded;
historical read pointers are never fabricated into old readership observations.

Local/inbound classification uses the native `origin_kind`. Message placement time
on this board, not a remote author's timestamp, assigns its window. FTN, QWK and
CircuitNET mappings contribute context only. One native message contributes once
even if it has several adapter mappings. No outbound-delivery counter is inferred
from a mapping. Unique poster counts cover known local accounts only; external
names are not treated as stable people. Deleted authors lacking an existing token
cannot be retrospectively identified.

## Windows, threads and status

7/30/90 windows use UTC calendar days, including the current partial day. At time
`t`, start is `(floor(t/86400)+1-days)*86400`; end includes `t`. Previous 30 days
are the adjacent full days before the current 30-day start. Daily progress records
whose most recent time is still in the future are withheld after a clock rollback.
This can temporarily undercount earlier progress in the same day; it does not
invent timestamps. Storage and display explicitly use UTC, so DST cannot duplicate
buckets. Monitoring start is the later of tracking start and native conference
creation. Re-enabling collection starts new known coverage.

Reply counts use native parent links. Active threads count distinct root IDs among
messages placed in the window, including replies to older roots. Thread starts
count messages without parents in the window. Parent walks are bounded to 64;
missing/cross-conference/cyclic/deeper chains are marked incomplete and use a
provisional root. Native link corrections invalidate dependent descendants.
Native helpers derive average replies per active thread and reader-progress/post
ratios; zero denominators return None, not infinity. Aggregate JSON exposes the raw
components, rather than separate ratio fields. Retained total and
last-activity timestamps cover the configured retention, not unlimited lifetime.
Unread per-caller backlog, external unique people, longest thread and lifetime
counts are not inferred from insufficient evidence.

Status precedence:

1. Local posts and readers both nonzero in 30 days: Locally Active.
2. Observed days shorter than configured dormancy window: New/Insufficient History.
3. Inbound posts but zero progress in dormancy window: Locally Unread.
4. Zero posts and progress in that window: Dormant.
5. Otherwise Quiet.

Pending projection work overrides status with Catching Up; disabled collection
shows Disabled. Details preserve evidence and freshness. Trends require 60 observed
days and at least five combined current/previous readers. Rising requires at least
three more readers and a 25% increase; Falling at least three fewer and a 20%
decrease. Otherwise Stable. There is no weighted score, user ranking, sentiment
analysis or automatic network/configuration mutation.

## Bounds and freshness

A rollup transaction projects at most 1,000 messages and 32 conference snapshots.
A daemon action runs at most 16 batches, checking shutdown and a ten-second budget
between batches. A single indexed aggregation is not forcibly interrupted; cost is
proportional to retained observations in the selected conferences. Initial migration
records `MAX(message_id)` and creates an index on native parent links; it does not
scan/rewrite all old messages into analytics in one migration transaction.

Projection windows use conference/time indexes. The operator screen reads cached
snapshots and bounded native mapping context, never message bodies or the entire
native message base on refresh. Boards retain the existing 784-conference limit;
local IPC returns at most 32 rows per page. An hourly invalidation, evaluated within
rollup, ages otherwise quiet snapshots. Reads/posts invalidate affected conferences
promptly; normal display freshness follows the configured Event, not a live claim.

Retention defaults to 365 days, configurable 180–730. Each batch deletes at most
1,000 expired message projections and 1,000 reader rows; after a long downtime,
cleanup needs repeated runs. Each successful read-progress observation also removes
at most one expired reader row through the day index. An observation creates at
most one daily row, so absent/disabled rollup Events cannot cause continuing growth
once old rows become eligible. Idle rows can remain until the next observation or
rollup; retention is not a wall-clock deletion service. Tokens for active accounts
are bounded by accounts;
deleted accounts lose their link. Native conference deletion cascades projections.
Network retirement never deletes local conferences or analytical history.

## Events, access and recovery

The generic `conference-health` typed Event action uses existing daemon Events.
No new scheduler, thread, shell hook or adapter timer exists. The sfconfig schedule
shortcut creates a five-minute Scheduled Event with RunOnce missed policy. Normal
Event claims, history and restart/shutdown authority apply. No Event is silently
installed. This action accepts Scheduled or Manual policy; Immediate/Hybrid exchange
policy belongs to networking actions. Direct operator rollup uses the same implementation and SQLite writer
serialization. Repeating work replaces each message/snapshot rather than adding a
second contribution.

Local operator IPC minor 18 advertises ConferenceHealth. Read access requires Board
Statistics; direct rollup requires Change Online Configuration; settings/schedule
require Change Sensitive Configuration. Local mutation receipts/audit are reused.
This does not change CIRCUITNET-NG 1.4 or send analytics to network peers. Generic
Event Run Now retains its existing Event capability gate.

Caller Hot Conferences is optional native Bulletin content, rendered as plain text.
It reuses current MessageBackend conference authorization, excludes non-public-only
areas, retired contexts and pending snapshots, and limits results to 20. It returns
aggregate counts, never account tokens, names or bodies. No static cross-user
bulletin or separate Bulletin subsystem is created. Native Sysop-only catalog
access remains enforced by the existing conference authority.

SQLite backup includes config, cursor, work, anonymous observations and snapshots.
Restore does not replay read events or resurrect an Event claim. Future snapshots
are withheld; rollup after clock rollback rebuilds them excluding future placement.
Projection validation participates in board snapshot validation. Transport privacy
is unchanged: encrypted CircuitNET transport does not provide end-to-end message
privacy, and directed routing is not private mail.

## Future boundary

File Area Health could reuse the operational pattern for downloads/uploads and
network file context. It is not implemented here. No File analytics, third-party
interoperability, public service deployment or governance change belongs to this
component.
