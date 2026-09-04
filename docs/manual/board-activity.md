# Board Activity and System Statistics

<!-- help-topic: operator.activity -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)
>
> **Latest downloadable release:** SPITFIRE NG 0.1.0 Development Preview
>
> The downloadable Development Preview predates these activity services.

SPITFIRE NG keeps a short, useful history of board activity so an authorized
operator can answer practical questions: Are callers connecting? Did a
transfer finish? Is an external file location unavailable? Did the latest
backup succeed? The history records outcomes and safe identifiers, not what a
caller typed or the contents of messages and files.

Schema 18 provides the shared activity, statistics, notification, and
maintenance services. Schema 19 adds a protected local attachment path and
the `spitfire operator` read-only commands. `sfmonitor` and `sfconfig` are not
implemented yet. Do not open the SQLite database as a substitute for an
operator client.

<!-- help-topic: operator.dashboard -->

## Attach to a running board

Start the board normally, then use a second terminal and name that board's
configuration file explicitly:

```console
spitfire operator status /path/to/board/spitfire.toml
spitfire operator nodes /path/to/board/spitfire.toml
spitfire operator events /path/to/board/spitfire.toml
spitfire operator statistics /path/to/board/spitfire.toml
spitfire operator notifications /path/to/board/spitfire.toml
spitfire operator callers /path/to/board/spitfire.toml
spitfire operator maintenance /path/to/board/spitfire.toml
```

`watch-events` waits briefly for new activity. Every command attaches to the
already-running daemon, reads one bounded view, prints it, and exits. Closing
the operator command does not stop the board or disconnect callers.

<!-- help-topic: operator.security -->

The operating-system account that created a new board is initially allowed to
use these local read views. On Unix the board records that account's UID; on
Windows it records the stable account SID. Other local accounts are denied
unless explicitly listed in the board's operator configuration. BBS security
level and named Sysop status do not grant host-operator access.

The same commands work on Windows; name the intended board configuration just
as shown above. If the endpoint is unavailable, verify that the named board is
running. If authorization fails, use the board creator account or enroll the
intended host operator while the board is stopped, then restart the daemon so
the protected endpoint admits the new account. Do not edit named-pipe/socket
permissions manually or query SQLite directly. A protocol-mismatch message
means the CLI and running daemon need compatible current-source builds.

<!-- help-topic: operator.callers -->

## What the activity history contains

The Activity service records meaningful milestones such as a caller session
starting or ending, a message being saved, a transfer completing or failing,
a storage location becoming unavailable, and a cold backup starting,
completing, or failing. It does not record every transferred block, polling
cycle, screen update, or keystroke.

The normal history never contains passwords, login names, private real names
or contact details, message subjects or bodies, file contents, terminal input,
network packet data, private keys, remote addresses, or host filesystem paths.
A completed-call entry may retain the caller's public handle for the Recent
Callers view.

<!-- help-topic: operator.nodes -->

## Nodes and live status

The live status service reports configured and active nodes, callers online,
active transfers, storage warnings, recent errors, and open notifications.
An authorized node view may also show the public handle, connection type,
online time, current board section, terminal type and size, presentation
profile, and transfer state.

This is a view of the existing node manager, not a second copy of node state.
It does not reveal the caller's password, login name, private profile,
terminal input, message text, or remote address. Watching or recording a
caller's screen is not part of this feature.

<!-- help-topic: operator.statistics -->

## Today and lifetime statistics

Today means the board's configured local calendar day. The daily summary can
include calls started and completed, new callers, messages saved, successful
uploads and downloads, transferred bytes, failed or cancelled transfers, and
warning/error counts. Day changes follow the board timezone, including
daylight-saving transitions. A timezone change starts a separately versioned
daily summary rather than relabeling earlier facts.

Lifetime values continue to come from the board's existing caller, message,
file, and transfer counters. Detailed activity and daily history begin when
schema 18 is activated; SPITFIRE NG does not invent older event history during
an upgrade.

Recent Callers lists completed sessions from the retained activity history.
It uses public handles and safe call facts. Caller-facing rankings and public
report publication are not implemented by B-017.

<!-- help-topic: operator.errors -->

## Notifications, errors, and maintenance

Notifications are reserved for conditions that may need attention, such as a
failed backup, unavailable storage, a node fault, or an operational error.
Acknowledging a notification records that an authorized host operator saw it;
it does not erase or rewrite the source activity entry. Stale simultaneous
acknowledgements fail safely.

The maintenance view combines open notifications with recent warnings and
errors, unavailable storage locations, files awaiting review, and active or
incomplete transfers. Diagnostic tracing remains a separate host aid and is
not used to calculate these results.

<!-- help-topic: operator.retention -->

## Privacy and retention

By default, detailed activity is retained for 30 days and daily summaries for
400 days. Cleanup works in small batches so it does not monopolize the board.
Security and operator audit records are outside ordinary activity cleanup.

Retention settings are versioned. Before shortening either period, an
operator client must show the current number of affected records and confirm
that exact impact. Cold backups preserve retained activity, summaries,
settings, notifications, and operator acknowledgement audit. The fast live
activity ring is memory-only and starts empty after restart or restore.

Backups contain sensitive board data even though normal activity is redacted.
Protect backup directories according to [Backup and
Recovery](../operator/backup-restore.md).

## Reports and publication

Schema 18 exposes bounded, read-only facts that later reports can use. It does
not yet generate bulletin files, print reports, write arbitrary exports, or
publish caller-facing rankings. Those operations belong to the later report
and publication work and will use the same values rather than maintaining a
second set of counters.

For field definitions, ordering, query limits, transactions, and recovery,
see [Operator Observability](../technical/observability.md).
