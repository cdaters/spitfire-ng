# Operator startup, maintenance, and recovery

<!-- help-topic: configuration.storage -->

> Applies to current source. The 0.1.0 Development Preview download predates
> sfmonitor, sfconfig, and the current protected operator controls.

Use `spitfire setup` to create a new board and its first Sysop, `sfmonitor` to
operate a running board, and `sfconfig` to administer its typed settings.
Always name the intended board's `spitfire.toml`. Setup refuses an existing
board; it is not a reset or a replacement for normal configuration.

## First start and normal operation

1. Follow [Quick Start](quick-start.md) to run setup and check the new board.
2. Before startup, open `sfconfig --board /path/to/board/spitfire.toml --offline`
   and deliberately enroll the local operator permissions you need under
   **Operators**. Review and save. Creation gives only six monitor reads.
3. Quit offline sfconfig, then start `spitfire run /path/to/board/spitfire.toml`.
4. In another terminal, run `sfmonitor --board /path/to/board/spitfire.toml`.
   Use Nodes for live caller controls and Dashboard for daemon shutdown.
5. Choose **System Configuration**, Enter to open sfconfig, and Q to return to
   the monitor. Both tools independently authenticate to the same board.

Q exits only the current operator application. Shutdown requires the separate
explicitly enrolled action and its confirmation. Saved settings show whether
they affect new sessions or require a later external restart.

## Lost connection or configuration conflict

If the daemon stops while online sfconfig is open, its heading says
**ONLINE — disconnected; edits retained**. The draft remains in memory for
inspection and cancellation; it is not written to disk or switched to offline
mode. Quit and reopen deliberately. An uncertain save may have committed:
inspect the current revision after reopening before entering another change.
A successful save or recovered successful receipt remains saved even when that
save removes your subsequent read access.

For a conflict, keep the draft while reviewing the message. **R**, then explicit
confirmation, discards it and reloads current configuration. Re-enter the intended
change. There is no silent merge or stale overwrite. See [sfconfig](sfconfig.md).

## Recover your own operator permissions

Removing a capability takes effect at the next daemon authorization check;
removing Read configuration can also prevent the next sfconfig refresh.
Support discovery, ownership, root/Administrator, and BBS Sysop level never
grant an IPC mutation automatically.

If another enrolled operator can restore your grants through sfconfig, use that
normal online path. Otherwise:

1. Stop the daemon using another authorized operator, the owning foreground
   console's QUIT, or the normal local Ctrl-C stop path of the process you run.
2. Close other cold-board tools. Open the selected board in explicit
   `sfconfig --offline` mode as the local account with board filesystem access.
3. In **Operators**, add the current local identity if absent, then select each
   required capability. Review the additions and save deliberately.
4. Quit sfconfig and start the board normally. Attach again and verify the
   displayed permissions. New Windows SID admission requires this restart.

Offline access holds exclusive board ownership and is deliberate local
administration; it does not expand read-only bootstrap. Never delete a lock
file or loosen socket/pipe permissions to bypass ownership or authorization.

## Recover bad configuration

An invalid candidate is rejected before saving. An externally damaged saved
file can prevent both daemon startup and offline sfconfig opening; neither
silently repairs unknown fields. Keep the original board stopped and preserve
its files for diagnosis.

If the saved file remains valid but a setting was a mistake, use offline
sfconfig to change that field, review, save, and restart. If startup reports
**recovery required**, reopen the same typed authority once to reconcile a
completed file replacement. If it still fails, use a known-good full backup:

```sh
spitfire restore /path/to/backups/known-good /path/to/recovered-board
sfconfig --board /path/to/recovered-board/spitfire.toml --offline
```

The target must be new and its parent must exist. Inspect the recovered board,
quit sfconfig, then start the recovered configuration explicitly. Verify Sysop
login, caller/message/file state, permissions, and one caller journey before
resuming normal operation. Keep the damaged board stopped; two restored copies
must not serve as the same board concurrently. The new-root route works even
when damaged current configuration prevents same-board replacement validation.

A restore returns the entire authoritative snapshot to its recorded state;
post-backup changes are absent. It retains operator grants, command receipts,
audit, and SSH keys, but never resumes old caller/chat/operator connections.
Fresh attachment is mandatory. If restored grants omit your identity, use the
explicit offline enrollment above before startup.

Each typed save also keeps one complete `spitfire.toml.previous`. Treat it as a
bounded recovery input, not a clickable undo or a whole-board backup. Do not
copy it over a running configuration, edit receipt metadata, or mix a different
board's configuration and database. The supported full recovery procedure is
[Cold Backup and Restore](../operator/backup-restore.md).

## Maintenance status and supported operations

Maintenance / Errors has three established service owners:

| Owner | What to inspect | Execution boundary |
|---|---|---|
| File integrity and review | Unavailable storage, pending review, incomplete transfer counts and related Activity | Approved file-domain services; no attached repair command is added here. |
| Activity retention | Detail/summary retention periods and retained events | Existing bounded B-017 services; no cleanup button or scheduler runtime is implied. |
| Backup and recovery | Backup outcomes in Activity and failure notifications | Stopped-board `spitfire backup` / `spitfire restore`. |

Use F1 in Maintenance / Errors or Storage / Backup for the shared service
routes. In sfmonitor help, Up/Down and Page Up/Page Down scroll, Home returns
to the beginning, and Esc closes help.

A notification means attention is needed. Acknowledgement records that you saw
it; it does not fix storage, complete a transfer, validate a backup, or remove
the historical warning/error. Activity is the retained outcome history;
Maintenance summarizes current attention/domain counts and the last 24 hours
of warning/error history. These views may therefore show the same issue for
different reasons without contradicting one another.

Before a cold backup, gracefully shut down the daemon and quit offline sfconfig
and other cold tools. The exclusive lock must be free. Backup validates its
manifest and authoritative content; no separate online backup or generic repair
runner is exposed. A snapshot includes its own backup-start event, while that
backup's final completion/failure is recorded on the source after the outcome
is known. Absence of its own completion from a restored snapshot is expected.

Caller security/lifecycle editing retains the existing daemon-owning
[local console](../operator/caller-management.md#inspect-and-change-callers).
Message/file-area editing retains the stopped-board
[domain editor](../operator/configuration.md). Do not start either beside a
running owner. Pack/purge, B-022 screen/export/print, networks, doors, scheduler
runtime, service deployment, and release packaging are outside these controls.
