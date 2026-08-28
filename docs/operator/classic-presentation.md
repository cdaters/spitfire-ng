# Classic SPITFIRE-Inspired Presentation

Classic SPITFIRE-Inspired 1.2.0 is an independently authored presentation
package installed by normal SPITFIRE NG setup. It recreates the compact,
80-column CP437/ANSI character of the historical caller experience without
shipping original Buffalo Creek DISPLAY, HLP, MNU, or RIP bytes.

Modern remains the default. Classic changes display artwork, menu arrangement,
terminology, color, and contextual help. It does not change authentication,
authorization, caller privacy, messages, files, transfers, SQLite state,
multinode behavior, or command identifiers.

## Select Classic

Stop the board, then run:

```text
spitfire config /path/to/board/spitfire.toml
```

In **Presentation Profile**, keep mode `profile`, set the active profile to
`classic-spitfire`, and keep `modern-ng` as the base. In **Security / Caller
Defaults**, set **Post-login journey** to `stock` if the board should perform
the historically inspired live message/caller/new-file sequence before Main.
Save the static configuration and confirm `spitfire status` reports:

```text
Active: classic-spitfire 1.2.0
Base: modern-ng 1.1.0
Menu presentation: display-overrides
Status: ready
Post-login journey: stock
```

The presentation selection and post-login policy are intentionally separate.
The profile cannot execute commands; the fixed engine journey cannot be
redefined by package data.

## Terminal behavior

Use ANSI plus CP437 with SyncTERM or Qodem for the CLR presentation. Classic
is designed for 80x24/25 and remains safe on narrower or wider terminals, but
its fixed menus do not dynamically reflow. Decorative welcome, menu, and
goodbye artwork renders as one uninterrupted unit. Help, About, statistics,
messages, and listings retain the session pager and acknowledgments.

For a plain RAW/text listener, configure that listener with `ansi = false` to
select Classic BBS resources. RAW is not inherently text-only; its configured
terminal capabilities remain authoritative.

## Fallback and customization

Section 7 of `spitfire config` separately selects `display-overrides` or
`generated` menus. In display-overrides mode, exact-security menu resolution
is board override -> Classic active profile -> engine-generated menu. Modern
base exact-menu art is intentionally not inherited. Other displays retain
board -> Classic -> Modern -> built-in resolution. An ANSI caller prefers a
same-layer CLR and falls back to BBS; text selects BBS directly. Missing,
malformed, or terminal-unsupported exact menu art safely generates from the
authoritative `.MNU` records.

The setup-created `.MNU` files remain command and security authority. The
packaged Classic menu art matches the stock NG setup command sets at exact
security 10 and 50. Exact suffixes use the caller's assigned level, not the
configured Sysop threshold. If a Sysop customizes `.MNU`, provide matching
board-local menu artwork or select `generated`; static labels cannot
automatically follow a changed menu. Classic never adds or remaps commands.

Security values are operator-defined from 0 through 9999. The caller's level,
the configured Sysop threshold, each `.MNU` record's minimum, and an exact art
suffix are separate. Threshold 50 is the SPITFIRE NG setup default, not a
universal historical value. The historical manual says the distribution
included `SOP999`, while the inspected archive contains `SOP100`; this remains
an unresolved source discrepancy, not a setup rule.

Version 1.1.0 gives caller Main, Sysop Main, Messages, Files, and Sysop distinct
historically evidenced visual treatments instead of applying one frame to
every section. Welcome and Goodbye are more expressive, but live message,
caller-statistics, new-file, time, warning, and security values still come only
from SPITFIRE NG workflows. Classic never supplies counters or session state.
The engine adds board-local time, caller/last-call/call-count facts, live
per-call/daily remaining minutes, and any generic previous-denial notice around
the presentation. Profiles cannot alter these values. A prior denial is shown
only to that caller after successful authentication and is then acknowledged;
no password, supplied wrong value, address, or detailed security telemetry is
displayed.

Cold `spitfire backup` and `spitfire restore` preserve the selected profile,
descriptor, resources, package license, provenance, and stock journey setting.

## Identity and rights

Classic always identifies the running product as SPITFIRE NG. Its About text
credits the original SPITFIRE separately and states that this is not an
official Buffalo Creek Software release or endorsement. Every rendered asset
has allowed redistribution and independent-composition provenance in the
descriptor. Historical originals remain research evidence and are not part of
the installed package.
