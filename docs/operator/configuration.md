# Configuration

## Status

- **Verified:** Normal setup/configuration has created and reopened board
  identity, node/listener policy, conferences, file areas, and the packaged
  Modern presentation on clean boards.
- **Development Preview:** The interactive `spitfire config` surface below is
  the current supported operator boundary.
- **Planned:** Live reload, broad destructive maintenance, safe board-path
  relocation, and a remote administration interface are not implemented.

## Use the supported configuration surface

Stop the board, then run:

```bash
spitfire config /path/to/board/spitfire.toml
```

The interactive menu validates configuration and coordinates with runtime,
backup, and restore through the board-wide operation lock. Live reload is not
implemented.

## The important save rule

The menu has two kinds of changes:

| Sections | Commit behavior |
|---|---|
| 1 General, 2 Nodes, 3 Terminal Services, 4 Caller Defaults, 7 Presentation Profile | Held in memory until you select `S`. `Q` without `S` discards these static edits. |
| 5 Message Conferences, 6 File Areas | Transactional and immediate. `Q` does not undo them. |

This distinction is intentional because static configuration lives in TOML,
while conferences and file areas are operational records in SQLite.

## 1. General system and Sysop identity

This section edits:

- board name;
- Sysop display name;
- IANA board timezone;
- public/private admission and private threshold; and
- Sysop caller name.

The Sysop display name is branding. The Sysop caller name must match an actual
caller account and is used for login and Comment to Sysop. Setup creates that
account. Changing names does not create or rename a caller.

Daily policy follows the configured board-local civil date. Choose the real
operating timezone instead of leaving UTC accidentally.

## 2. Nodes

The node count is the number of simultaneous sessions one server process may
own. Node allocation is transport-independent and selects the lowest waiting
enabled node. Reducing the count removes out-of-range overrides when the
static configuration is saved.

The native model supports one process owning the node pool. Do not start
`run`, `console`, or `shell` as independent concurrent owners of one board.

## 3. Terminal services

The menu lists configured services and edits one service's enabled state and
network bind endpoint. Normal setup creates Telnet, RAW TCP, and RLogin.

Both setup and this configuration surface accept `y`, `yes`, `n`, and `no`
case-insensitively, preserve Enter/default behavior, and reprompt after invalid
input. Disabling a listener skips its bind/port questions; validated internal
defaults may remain stored for a later re-enable.

| Bind | Meaning |
|---|---|
| `127.0.0.1:2323` | Local computer only; safe first-run default. |
| `0.0.0.0:2323` | Every IPv4 interface; requires deliberate firewall/network/security review. |
| a specific LAN address | Only that interface; useful for a trusted local network. |

Telnet, RAW, and RLogin do not encrypt passwords or terminal content. Binding
beyond loopback is an explicit risk decision. SSH is not currently available.

Use ports above 1024 unless the host has been deliberately configured for a
privileged service. Every enabled listener must have a unique endpoint.

## 4. Security and caller defaults

This section controls:

- new-caller numerical security;
- minutes per call and per board-local day;
- first-day minutes;
- maximum calls per board-local day;
- keyboard-idle timeout;
- the optional fixed post-login journey (`none` or engine-owned `stock`); and
- disabled/optional/required postal, phone, email, and birth-date groups; and
- optional subscription warnings and expired-security policy.

Numerical security gates menu commands, conferences, file areas, and the
caller-facing Sysop boundary. Keep the ordinary new-caller level below the
Sysop threshold. Collect only profile data the board actually needs; contact
fields are private, but unnecessary collection is still unnecessary risk.
The `stock` post-login journey reads real message, caller, and new-file state
before Main. It is board/session policy, not a presentation-profile script.

Subscription policy is off by default. To enable it on a stopped board, edit
the caller section in `spitfire.toml`:

```toml
[caller.subscription]
enabled = true
warning_days = 7
expired_security = 5
```

`warning_days` must be 0–365 and `expired_security` must be 0–9999. Dates are
assigned per caller from `spitfire console`; they remain valid through the
displayed date in the board's configured time zone. Expiry lowers effective
security without overwriting base security. See
[Caller Management](caller-management.md).

## 5. Message conferences

The conference screen can add, edit, enable, or disable a conference. It
configures caller-visible number/name/description, read and post security,
threshold versus exact read access, public-only policy, caller message-deletion
policy, and maximum message lines. Caller deletion permits the sender, direct
recipient, or CC recipient to tombstone only that delivery. It never grants an
unrelated caller authority and does not limit threshold-Sysop mutation.

Conference 1 is mandatory because Comment to Sysop uses it. Disabling a
conference preserves messages and identity. There is no destructive
conference delete or message-purge workflow in the Development Preview.

## 6. File areas

The area screen can add, edit, enable, or disable an area. It configures:

- caller-visible number, name, and long description;
- immutable safe storage key;
- read and upload security;
- threshold versus exact access;
- lower-security preview;
- no-charge behavior;
- maximum upload MiB; and
- up to five privileged security levels.

Creating an area also creates its confined storage directory. Its number and
storage key cannot be changed later because a casual edit must not orphan
metadata or bytes. Disabling preserves both.

## 7. Presentation profile

Normal setup selects `modern-ng` as both active and base. Section 7 can change
the two confined profile IDs or select `legacy-resources` for a pre-M031 board
that still owns direct SYSTEM/DISPLAY content. It also selects
`display-overrides` or `generated` menu presentation. Generated mode derives
Main/Message/File/Sysop directly from the parsed, security-filtered `.MNU`
authority; display-overrides mode accepts exact-security board/active-profile
BBS/CLR art and generates safely when it is absent, malformed, or unsupported.
At a capable 80-column ANSI terminal, generated mode uses equal 30-character
cells with a fixed eight-character gutter. Plain, constrained, or overlong
localized output falls back to a bounded single column without changing
commands or authority.

These two modes can look different for callers at different exact security
levels. In `display-overrides`, a caller with matching `MAIN10` art receives
that resource while a caller without matching `MAIN50` art receives the
generated fallback. That is exact-security resource selection, not a
security-selected generated layout algorithm. Use `presentation.menu_mode =
"generated"` when every security level should use the same generated geometry.
For a live caller, `spitfire status` reports
`renderer=exact-security-board-override`,
`renderer=exact-security-active-profile`, `renderer=generated-stock`, or
`renderer=expert-suppressed` so the selected path is explicit.
Save with `S`, then run
`spitfire status` before restarting. A valid selection reports profile IDs,
versions, effective source, and `ready`; a missing/invalid active package
reports `DEGRADED` and the base or built-in fallback in use.

Profile changes are cold configuration. They change presentation only and do
not change terminal ANSI preference, commands, security, messages, files, or
transfers. See [Presentation Profiles](../presentation-profiles.md) before
installing or authoring a package.

Setup also installs `classic-spitfire` and `minimal-terminal` without selecting
them. For Classic selection and the separate stock-journey setting, see the
[Classic Presentation Operator Guide](classic-presentation.md).

## Language and locale

`[language].default_locale` selects engine-owned prose independently of the
presentation profile. Clean setup installs and selects `en-US`; configuration
menu item 8 changes the locale. Stop the board, validate/install a package with
the public `language-validate` and `language-install` commands, select it with
`spitfire config`, then require `spitfire status` to report the intended
default/effective package and `READY`. See [Language Packages](localization.md).

## Configuration authority

`spitfire.toml` is authoritative for static settings. SQLite is authoritative
for callers, conferences, messages, file areas, catalog metadata, and
statistics. Cataloged file bytes live below the configured logical external
path. Do not manually edit SQLite or move managed paths.

The complete validated format and authority boundaries are in
[SPITFIRE NG Setup and Configuration](../sfng-setup-configuration.md).

## Setup prompt behavior

M037.3 closes the bounded setup/operator findings: disabled
RAW/RLogin/Telnet listeners
skip bind/port questions; listener prompts accept `y`, `Y`, `yes`, `n`, `N`,
and `no` case-insensitively and invalid input reprompts; disabled listeners can
retain safe internal defaults without forcing irrelevant questions. The
interactive security decision is simply `Sysop security threshold [50]:`;
historical level research remains in the reference documentation. Setup also
names every installed presentation
package and separately asks for active/base profile, generated/display-override
menus, post-login journey, new-caller security, Sysop threshold, and initial
Sysop caller security.
