# Configure a board with sfconfig

> Applies to current source. Build/install `sfconfig` and `sfmonitor` from the
> same source checkpoint. The published Development Preview download is unchanged.

`sfconfig` is SPITFIRE NG's native configuration application. It edits named
board settings, validates them, and shows their operational effect before saving.
The daemon continues serving callers while online configuration is open.

## Start directly or from sfmonitor

For a running board:

```sh
sfconfig --board /path/to/board/spitfire.toml
```

In `sfmonitor`, select **System Configuration** and press **Enter**. The monitor
hands the terminal to the `sfconfig` executable installed beside it. Quit
sfconfig to return to a refreshed monitor. Each application authenticates
independently against the same explicitly selected board.

For a stopped board, explicitly select offline mode:

```sh
sfconfig --board /path/to/board/spitfire.toml --offline
```

**ONLINE** means the daemon owns every read and save. **OFFLINE** means the
configuration service holds exclusive board ownership; the daemon and other
cold-board tools cannot start until sfconfig exits. An attachment error never
silently selects offline mode. Reopen the application after a lost daemon
connection. Draft edits remain visible until you cancel, reload, or quit.

## Navigate and edit

Use **Tab / Shift-Tab** to move between sections and **Up / Down** to select a
field. **Home / End** and **Page Up / Page Down** move through long lists.
Press **Enter** to edit a field, then Enter to retain the edit locally. **Esc**
cancels that field edit. Changing sections retains all staged edits.

The sections are General, Nodes / Listeners, Caller Access, Presentation,
Security, Operators, Messages / Files, and Storage / Backup. Message conferences
and file areas show real settings as read-only summaries; their existing
stopped-board editors remain available through `spitfire config`. Board identity
renaming and storage relocation are not online field edits. No pages are offered
for unimplemented networks, doors, or jobs.

Press **?** or **F1** for help on the current section and the edit/save workflow.
Page keys scroll long help and save reviews. Press Esc to close help.
A 100×30 terminal is preferred; 80×24 is usable. Below 60×20 the application
shows a resize notice and retains your work.

## Review, save, or cancel

An asterisk marks a changed field and the header says **Unsaved changes**.
Press **S** to validate all staged changes and open Review. Check each old/new
value, any operator-capability additions/removals, and the operational effect.
Press Enter to **Save / Apply**, or Esc to continue editing. Invalid values never
reach configuration storage. Field and section errors explain relevant limits.

Press **C** to cancel all local edits, with confirmation. **Q** quits only
sfconfig. It asks before discarding unsaved work and exits directly when clean.
Confirmed saved changes remain persisted even if a subsequent read loses access.

## Understand when a setting takes effect

| Effect | What happens |
|---|---|
| Applied online | Operator-profile changes affect the next authorization check. Existing connections receive no permanent grant. |
| New sessions | Subsequent callers receive the saved caller/admission policy. Existing callers keep their captured policy. |
| Restart required | Nodes, listener bindings/enabled state, timezone, and presentation/language selections persist while the daemon retains its active values. |
| Offline only | Cold backup/restore, identity maintenance, and storage relocation retain their existing separate ownership requirements. |

There is no restart button and saving never restarts the daemon. Use the existing
sfmonitor graceful shutdown when appropriate, then start the daemon through your
normal launch or deployment mechanism. Adding Windows pipe principals also
requires restart for admission; revocation is checked during dispatch.

## Handle a configuration conflict

Two operators can open the same revision. After one saves, the other cannot
save over it: **Configuration changed since you opened this screen.** Drafts
remain available for review. Press **R** and confirm to discard the old draft and
reload the latest configuration. Re-enter the intended changes, review, and save.
The application never silently merges operator arrays or uses last-writer-wins.

If a save reply is lost, retry the same unchanged save to recover its recorded
result. Do not assume a disconnected screen means the save failed. Reopen and
read the current revision before making a different change.

## Enroll operator permissions explicitly

OS operator identities are separate from caller accounts and Sysop security
levels. A Unix identity is a UID; a Windows identity is a SID. Bootstrap and
existing omitted/default profiles retain exactly the six established monitor
reads. They do not gain configuration mutation rights or automatic administrator
privileges.

For first enrollment, stop the board and open sfconfig with `--offline`. In
**Operators**, the current local identity can be added with a read-only profile.
For an existing identity, select its individual capability rows and press Enter
to toggle each desired grant:

- **Read configuration** permits configuration snapshots.
- **Change ordinary configuration** permits ordinary configuration saves.
- **Change security and operator profiles** is additionally required for security,
  listener/admission, or operator-profile changes.

Review and Save explicitly commit enrollment. The application shows descriptions
for the existing monitor/control capabilities as well. No wildcard or grant-all
command exists. Each profile must contain 1–32 unique recognized capabilities;
there may be at most 32 unique principals. **D** stages removal of the selected
principal; it takes effect only after Review and Save. Avoid removing your own
required access unless you intend to recover through exclusive offline access.

## Secrets and recovery

Security shows SSH private-key state only: **Missing**, **Configured**, or
**Invalid**. Private-key bytes are never displayed or prepopulated in an editor.
SSH key generation/rotation retains the existing transport/maintenance boundary;
this MVP does not add a key replacement or clear operation. There are currently
no password/token fields in the static board configuration. Caller credentials
remain in caller authority and are never configuration snapshots.

Every successful replacement preserves one complete prior configuration beside
`spitfire.toml`, named `spitfire.toml.previous`. Later saves replace that one
backup; they do not create a growing history. For full recovery protection use
[Cold Backup and Restore](../sfng-backup-restore.md), which retains exact current
configuration and database together. The previous-file convenience copy does
not substitute for a full board backup.

A **recovery required** result means the file may already have committed. Reopen
through the same typed authority; it reconciles the file's receipt link before
allowing another save. If validation or recovery still fails, keep the board
stopped and follow the full backup/restore procedure. Do not overwrite a running
board or edit the SQLite journal. The previous file is a stopped-board recovery
input, not an online undo command.

## Common errors

| Message | Action |
|---|---|
| Cannot read configuration | Check daemon state, the selected board, protocol compatibility, and Read configuration enrollment. Use `--offline` only for a stopped board. |
| Cannot acquire offline authority | Close the daemon and other offline tools, then reopen. Never delete a lock file to bypass ownership. |
| Permission denied | Review the separate ordinary and sensitive configuration capabilities. Support discovery does not grant permission. |
| Invalid field/section | Use contextual help for numeric bounds, profile-mode requirements, unique listener addresses, and capability limits. |
| Save outcome uncertain | Retain the unchanged draft/CommandId and recover its receipt, or reopen and inspect the current revision before further work. |
| Handoff failed | Install the matching sfconfig executable beside sfmonitor; verify configuration access. The monitor restores its own terminal. |

Real Windows TUI, named-pipe configuration, SID enrollment, handoff, and atomicity
acceptance remain **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**. Apple Silicon
macOS is the native acceptance platform for this slice. See the
[Configuration Technical Reference](../technical/configuration.md) for authority,
versioning, recovery, and extension boundaries.
