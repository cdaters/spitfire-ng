# Configure a board with sfconfig

[Conference Health settings and rollup](conference-health.md) describes native readership/activity and the optional caller bulletin.

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
Security, Operators, Messages / Files, Storage / Backup, and Networks. Message conferences
and file areas show real settings as read-only summaries; their existing
stopped-board editors remain available through `spitfire config`. Board identity
renaming and storage relocation are not online field edits. Networks opens named forms for the implemented QWK/DOVE, FTN and BinkP engine.
Doors and general jobs remain unimplemented.

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

For the complete first-start, permission recovery, invalid configuration, and
cold-backup journey, see [Operator Startup and Recovery](operator-recovery.md).

## N2 QWK networking

Use the existing operator capability editor to enroll network-status, network-run and network-queue explicitly. Partner/map requests currently use the typed CLI; there is no new Networks TUI page.
See [QWK networking](../manual/qwk-networking.md) for the implemented scope and interoperability limits.

## N3 FTN minimum

The [FTN Sysop chapter](ftn-core.md) documents the implemented typed sfconfig
policy import, protected packet/directory operations and read-only status counts.
Cold restore holds uncertain FTN work and origin serial allocation; no manual SQL
release is prescribed. Full Networks UI and BinkP remain later work.

## N5 network configuration

The [Networks manual](network-operations.md) covers typed policy/mapping forms,
conference selection, separate static/relational CAS, write-only credentials,
listener effects and stopped-board origin recovery. Save or cancel the general
draft before opening Networks. No raw TOML editor or generic command entry is used.

## N6 hub extension

The [FTN hub manual](ftn-hub.md) defines downstream/point configuration, separate area subscriptions, authenticated AreaFix, bounded rescan, safe activity and per-recipient recovery. Schema 25 and operator protocol 1.10 add these typed services; existing N1–N5 authority remains intact. Network bodies and credentials are absent from operator projections.

## N7 file-network extension

FileEcho, TIC, native-file hatching and exact approved FREQ reuse native file authority and BinkP transport. sfconfig offers typed file policy/mapping/subscription/grant/hatch forms; sfmonitor Networks → 9 Files includes staging, history and Enter delivery detail. [File-network workflows and authority](ftn-files.md). No raw path or credential appears in projections.

## Caller names and posting policy

Board fields **Posting identity** and **Require first and last name** are
independent. The first controls the fallback posting name; the second controls
private collection at signup/profile editing. Both default to handle-friendly
operation. Conference settings can inherit or choose Handle allowed / Real name
required. FTN/QWK mapping and link forms expose their own posting requirement;
conference selection shows the saved effective requirement and source, without
caller names. Draft changes take effect only after the existing review/save flow.

Use [caller management](../operator/caller-management.md) for the private profile
commands. See [identity precedence](../technical/identity-policy.md) for exact rules.

## CircuitNET C2 cold-board commands

`sfconfig circuitnet <board-config> <action> <network> [arguments]` exposes typed
identity/tree setup, codename mapping, manual Dossiers and safe offline status.
The same host service performs explicit scan/export/import/acknowledgement/retry.
It requires a stopped board, the existing operation lock and the applicable
operator grants. It is not an online TUI cockpit. See the complete
[CircuitNET journey](circuitnet.md) before enabling trusted offline custody.
