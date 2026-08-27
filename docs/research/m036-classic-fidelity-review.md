# M036 Classic SPITFIRE Presentation Fidelity Review

## Purpose and boundary

This is the implementation record for M036, the rights-clean visual refinement
of `classic-spitfire`. It explains what changed after M035, which evidence
supports the change, and which historically visible behavior remains outside
the version-1 presentation contract. It does not authorize redistribution of
Buffalo Creek Software resources.

M036 changes authored presentation resources only. Authentication,
authorization, command dispatch, message and file state, caller statistics,
new-file checkpoints, time policy, and session transitions remain engine
owned. Profile format 1 and resource API 1 are unchanged. The resulting
package version is 1.1.0; Modern remains the setup default and Classic still
uses Modern as its configured base.

## Evidence reviewed

Primary evidence was inspected read-only outside the public source tree:
`SPITFIRE.DOC`, the exact
27-member `DISPLAY.ZIP`/`DISPLAY` set, and its BBS/CLR/MNU/RIP companions. The
loose DISPLAY files were verified byte-identical to their archive members.
The historical HLP and menu authorities identified by M032/M034 were used for
function and command context, not copied.

Redacted runtime observations retained in the private preservation record were
also reviewed as observed-screen evidence. They include welcome/authentication,
security-10 and high-security Main variants, built-in menu fallbacks,
Message/File/Sysop menus, statistics, failed-login and timeout notices, and
Goodbye. No image bytes entered the generated profile or public source tree.

The runtime images corroborate these structural findings:

- a stock security-10 Main CLR uses a light-gray outer field, colored inset,
  black drop shadow, centered title, compact two columns, and a separate
  status area below;
- exact-security Main resources can be materially different compositions,
  while an absent exact resource can expose a simpler built-in presentation;
- Message, File, and Sysop screens use distinct palettes and denser section
  grammar rather than one universal Main-menu motif;
- WELCOME1 participates directly in authentication, while message queue,
  caller statistics, new-file state, warnings, and time facts are live engine
  output around the authored displays;
- GOODBYE is a deliberate sign-off screen, not merely a closed socket; and
- RIP supplies composition evidence only. RIP rendering remains unimplemented.

The observations were hypotheses until compared with the primary resources.
The M036 implementation adopts only the corroborated grammar and current
SPITFIRE NG command/state model.

## M035 resource audit and disposition

`KEEP` means the M035 bytes remain suitable. `REFINE` means wording, title
placement, or palette changed within the same function. `REPLACE` means a new
independent composition supersedes the M035 generic panel. `DEFER` identifies
evidenced behavior that cannot safely or truthfully be implemented as a
format-1 asset.

| Resource(s) | Disposition | M036 result and reason |
|---|---|---|
| `SPITFIRE.HLP` | KEEP | Current SPITFIRE NG command prose was already accurate and independently authored. |
| `SFPRELOG` | REPLACE | Compact board, Sysop, node, and SPITFIRE NG identity establishes the connection before authentication. |
| `WELCOME1` | REPLACE | Expressive centered CP437/ANSI welcome now flows into real authentication without caller-facing engineering commentary. |
| `WELCOME2`, `NEWUSER` | REFINE | More historically natural transition wording; live post-login operations remain outside the resource. |
| `MAIN10` | REPLACE | New caller Main uses the evidenced framed/inset/shadow grammar and fixed two-column command placement. |
| `MAIN50` | REPLACE | Independently authored framed, blue-inset exact-security Sysop Main is materially distinct and includes only authorized current commands. |
| `MSG10`, `MSG50` | REPLACE | Compact section composition and blue/red ANSI identity replace the universal yellow frame. |
| `FILE10`, `FILE50` | REPLACE | Compact section composition and orange/blue ANSI identity distinguish Files from Main and Messages. |
| `SOP50` | REPLACE | Compact orange/green Sysop treatment replaces caller-facing authority prose. |
| `GOODBYE` | REPLACE | A complete, newly authored sign-off names SPITFIRE NG and the configured board without copying old advertising/artwork. |
| `SFONFAIL`, `PRIVATE`, `LOCKOUT`, `TOOMANY`, `SFTIMEUP`, `SFASLEEP` | REFINE | Red warning palette and concise current-state wording use events SPITFIRE NG already owns. |
| `SFPGOFF`, `SFUNANS`, `SFPAGED`, `USERINIT`, `CHATDONE` | REFINE | Centered compact panels retain the existing page/chat event semantics. |
| `SF1STM`, `SF1STF`, `SFMSG1`, `SFMSG2`, `SFIL1`, `SFIL2` | REFINE | Cyan section-transition treatment supports the differentiated menu grammar. |
| `SFDOWN`, `SFUP` | REFINE | Immersive caller wording replaces implementation/staging prose; binary transfer ownership is unchanged. |
| `ABOUT` | REFINE | Accurate identity/attribution remains explicit and records Classic 1.1.0; engineering context is kept here rather than in menus. |
| previous failed-login-attempt details | DEFER | Current engine does not expose the historical previous-attempt fact; it must not be faked. |
| current clock/minutes-used/minutes-remaining footer | DEFER | Format 1 lacks live macros for the complete historical footer. `@LOGTIME@` is an allowance value, not a safe general status renderer. |
| Doors, batch displays, questionnaires, events, RIP | DEFER | Category-B or RIP scope; neither commands nor assets are added by M036. |

All 31 caller-critical CLR resources retain BBS counterparts with the same
function and command set. `SFPRELOG` remains intentionally BBS-only and is
selected safely for every capability.

## Adopted visual grammar

The menus remain fixed, resource-authored 80-column compositions. Command
letters use prominent `<K>` notation and the same current `.MNU` authority as
M035. No adaptive column algorithm, command remapping, or profile security
logic was added.

- Caller Main: light-gray outer band, magenta inset, black shadow, centered
  title, balanced two-column commands, and open space below for engine output.
- Sysop Main: a distinct framed blue-inset exact-security composition. The
  observed unframed cyan screen is recorded as built-in fallback evidence, not
  misrepresented as an authored exact-security resource.
- Messages: compact blue/red section treatment.
- Files: compact orange/blue section treatment.
- Sysop: compact orange/green section treatment.
- Welcome/Goodbye: more expressive centered CP437 rules and restrained ANSI.
- Informational/event panels: compact CP437 frames with function-specific
  warning, transfer/section, informational, or neutral colors.

These are independent SPITFIRE NG interpretations. No border sequence,
wording block, telephone advertisement, logo, cursor program, or artwork was
traced or copied from a historical file or screenshot.

## Screen ownership and dynamic state

The fixed engine-owned `caller.post_login_journey = "stock"` remains the only
historical login-sequence integration. It renders real waiting/received/sent
message counts, real caller/message/file statistics, and the real new-file
checkpoint/question before Main. M036 does not add a hook, state machine, or
new engine-owned English string.

Decorative prelogin, welcome, menu, and Goodbye resources may clear and render
as one authored unit. Help, About, message/caller statistics, messages, and
file listings retain paging/acknowledgment behavior. Transfer streams never
pass through presentation paging. A historically dense clock/time footer is
recorded but deferred until an engine-owned, live, localizable status surface
is specified.

## Security, localization, and modernization

Exact-security resolution is unchanged: Classic supplies art only for the
normal setup authorities at security 10 and 50. Missing exact artwork uses the
existing generated/fallback behavior. A security-10 caller cannot see or enter
the Sysop menu; ANSI and BBS callers receive identical authorization.

Behavior stays engine-owned and profile prose stays in resources, preserving
future separation between visual profile and language pack. Remaining
engine-owned localization surfaces include authentication prompts, stock
post-login summaries/questions, live statistics, paging prompts, menu command
prompts, and session/time warnings emitted directly by workflows.

Classic retains Argon2id authentication, full-year dates, international
profile fields, privacy policy, reliable bounded input, modern transports,
SQLite persistence, multinode isolation, transfer integrity, backup/restore,
and accurate SPITFIRE NG identity.

## Rights and provenance result

Every generated display/menu asset is `historical-inspired`, created and held
by Craig Daters and SPITFIRE NG contributors, licensed
`LicenseRef-SPITFIRE-NG-Project`, and marked redistribution `allowed`. Each
record names this review as evidence, records the M036 independent
modification, and carries generated size/SHA-256 metadata. The unchanged HLP
has its own record explaining that its M035 independent wording was retained.
No unresolved asset is installed.

Historical originals, screenshots, RIP resources, obsolete telephone/version
copy, and unsupported command art remain rights-blocked or scope-deferred and
outside the package.

## Acceptance record

The final quality, package, fallback, client, terminal-size, security, and
regression results are recorded in
[Classic SPITFIRE-Inspired Presentation Profile](../classic-presentation-profile.md),
[Presentation Profiles](../presentation-profiles.md), and the private
engineering record. This review ends at operator acceptance; it does not
authorize RIP, Category-B implementation, or website publication.
