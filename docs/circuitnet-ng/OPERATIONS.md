# SPITFIRE NG operator commands

These guides use SPITFIRE NG's sfconfig, sfmonitor and spitfire executables.
Install a compatible SPITFIRE NG release first; this kit contains documents and
configuration artifacts, not executable software. Other implementations use their
own configuration tools while following the same network policy and protocol.

## Command conventions

Run commands from your board directory. `spitfire.toml` names your board's existing
configuration. Replace it if yours has a different name. Commands using MYBBS,
MYHOST, HOST_ADDRESS or similar names require your approved enrollment values.
A backslash at the end of a displayed command continues it on the next line in a
POSIX shell. On other shells, join the lines and omit the backslashes.

Configuration commands that create profiles, enroll keys, pin/import catalogs or
change mappings require a stopped board. Stop the daemon gracefully first and
restart with `spitfire run spitfire.toml` afterward. Live Poll, Test Link, status,
hold/release, Dossier controls and Events use the running daemon's operator service.
Do not start a second daemon to work around a board lock.

## Operator access

The local operating-system identity must have the relevant operator permissions.
With the board stopped, run `sfconfig --board spitfire.toml --offline`, open
Operators, select the intended local identity and review its capabilities.
Configuration work needs Read Configuration and the sensitive-configuration
permission; live work needs the corresponding network status, Test Link or Run
permission. Review and save deliberately. Do not grant caller accounts operator
capabilities as a shortcut. See the installed SPITFIRE NG manual's
`docs/manual/sfconfig.md` for the full capability editor.

## Inspect a running board

```
sfconfig circuitnet spitfire.toml live-status circuitnet-ng
sfconfig circuitnet spitfire.toml why circuitnet-ng MYHOST
sfconfig circuitnet spitfire.toml route-test circuitnet-ng MYHOST
sfconfig events spitfire.toml list
```

Run `sfmonitor --board spitfire.toml` to inspect Networks, catalog health, queues and
subscription requests. Its Events view shows next runs and failures and supports
Run Now with the appropriate permission. No view needs message bodies to explain
network delivery status.

## Why did mail not move?

Check the catalog revision and local mapping first. An ignored, unknown or retired
area cannot export. A Sysop-only mapping must retain restricted local access.
Next check the direct neighbor's Dossier, hold state and last exchange result.
A pending approval is not an active subscription. A scheduled policy may simply
be waiting for its next Event. Test Link isolates enrollment/connectivity problems;
Poll then attempts queued work. A successful Test Link alone transfers no mail.

```
sfconfig circuitnet spitfire.toml test-link circuitnet-ng MYHOST
sfconfig circuitnet spitfire.toml poll circuitnet-ng MYHOST
sfconfig circuitnet spitfire.toml hold circuitnet-ng MYHOST
sfconfig circuitnet spitfire.toml release circuitnet-ng MYHOST
```

Confirm a receipt or completed queue entry before calling a delivery complete.
An unreachable peer leaves work queued. Repeated posts are not a repair for an
unacknowledged delivery; retry the existing work.

## Limits and recovery planning

The current catalog retains at most 256 conference identities, including retired
ones, and 4,096 revisions. These are history-preserving bounds, not quotas for
active discussion. Plan a reviewed software/catalog migration before reaching
one; do not delete tombstones or reset revision numbers. There are at most 64
retained signing-key epochs. Status and advanced catalog details aid planning.

Established tree changes are not an automatic topology-migration service. Agree
changes with the Administrator and affected neighbors, hold traffic, back up and
reconcile outstanding receipts before changing relationships.

File transfer and archive policies impose bounded sizes and inspection limits.
Consult the supplied technical Files specification before enabling a file-area
mapping; do not increase limits merely to make an unsafe archive pass.

Back up the board database together with its configured payload stores. A missing
content store is not a complete Files backup. Catalog authority/history and local
mapping decisions are retained. Restoring over a board with newer known catalog
or signing-key state fails closed. Recover current trusted history rather than
removing that safeguard. Signing-key backups have separate custody: see
[KEY-CUSTODY](KEY-CUSTODY.md).
