# Updating SPITFIRE NG

For an adopted D1 installation, stop the board cleanly, then run its installed
launcher:

```sh
spitfire update --check
spitfire update --dry-run
spitfire update
```

`spitfire` here means `INSTALL/bin/spitfire` (`spitfire.exe` on Windows).
Use `spitfire version` to see the active runtime. The release source is a local
directory configured at adoption; D1 needs no remote update service.

The update checks compatibility and available storage, creates and verifies a
pre-upgrade recovery checkpoint, verifies the signed release, runs migrations
and offline board validation, then activates the new runtime. It retains the
previous runtime and checkpoint. The board remains stopped; use your normal
startup command or service procedure after success. Update never starts network
sessions or runs Events as a validation shortcut.

A running board or another maintenance operation causes refusal. Stop `run`,
`console`, `shell` and offline configuration tools first. Do not manually replace
files inside the installation's `releases/` directory.

## Failed update and interrupted update

Before commit, the previous installation remains authoritative. If migration or
validation fails, the staged changes are abandoned. If activation has begun,
SPITFIRE restores the verified pre-upgrade database/configuration and validates
the old runtime before permitting normal operation. Payloads and board resources
are outside the update's write set.

After a crash or reboot, run:

```sh
spitfire update --recover
```

Normal managed commands also recover a pending update before proceeding. A
committed update only finishes cleanup: recovery never rewinds data acquired
after commit. Missing or corrupt recovery evidence blocks startup with an error.
Keep the installation and checkpoint intact; do not delete the journal or board
locator to bypass the error. If the launcher cannot read a hot SQLite management
journal, use the preserved launcher explicitly:

```sh
INSTALL/bin/spitfire deployment recover INSTALL
```

An interrupted **manual disaster restore** follows the separate
[manual restore interruption runbook](backup-restore.md#interrupted-manual-restore-keep-the-board-stopped).
Neither recovery command above resolves a pending `ManualRestore` phase. D1 has
no supported deterministic finalization command for ambiguous manual publication.
Keep all candidates stopped and preserve the artifacts for reviewed recovery;
automatic pre-commit update recovery does not establish their authority. Runtime
rollback is not a database restore or a manual-restore recovery command.

## Runtime rollback preserves current data

```sh
spitfire rollback --list
spitfire rollback
```

Rollback checks previous runtimes against the current database schema,
configuration and required durable features. It validates a separate copy of
current state with the selected runtime, then changes only the runtime selection.
New messages, users, Files, Events, CircuitNET catalog/custody evidence and FTN
history remain intact. An incompatible or damaged runtime is refused.

An interactive terminal requires confirmation. Automation may use
`spitfire rollback --yes`. This is not a database downgrade or backup restore.
If no previous runtime is compatible, keep the current installation and obtain a
compatible corrective release. Do not force an older executable to open the board.

## Adopt an existing NG board once

D1 supports stopped native boards with relative, disjoint logical directories.
The board and installation roots remain separate. Use the same host account
that owns the board, and a release source containing a signed package matching
the adopting executable:

```sh
/path/to/spitfire deployment adopt /bbs/board/spitfire.toml /bbs/install /bbs/releases /bbs/release-public-key.hex
/bbs/install/bin/spitfire version
```

The parent of `/bbs/install` must exist; the installation itself must be new.
Obtain the release public key independently from the trusted distributor. Release
trust is separate from CircuitNET catalog trust. A downloaded package cannot
replace your local pin. Repeating the same adoption command can finish an
interrupted adoption; it does not replace an existing installation.

Put `/bbs/install/bin` on your PATH or point your existing service command at its
launcher. D1 does not create services, change service accounts or move libraries.
The managed runtime package currently contains `spitfire`; companion tools are
separate. Online operator clients continue through the active daemon. Standalone
non-selected executables cannot perform offline writes to an adopted board.

## Backups, storage and limits

See [Backups and Restore](backup-restore.md) and [Deployment Storage](deployment-storage.md).
External file libraries do not have to be duplicated. Metadata-only managed
payload backups require matching separately retained payloads for restore.

Release defaults do not overwrite local displays, presentation/language packages,
network policy or operator settings. Configuration is strictly validated; unknown
fields are rejected. Format 1 configuration is migrated to format 2 in staging;
format 2 is left byte-for-byte unchanged.

There is no live rolling upgrade, automatic service restart, remote release
channel, reverse schema migration or continuous quota service. Real Windows
activation/reboot acceptance is **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**.
Classic SPITFIRE 3.7 migration is separate future work; D1 upgrades existing NG
boards. Release builders should use the [technical contract](../technical/deployment.md).
