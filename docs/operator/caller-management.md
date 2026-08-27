# Caller Management

## How callers are created

On a public board, a terminal connection asks whether the person is a new
caller. Registration collects:

1. a unique case-insensitive caller name of at most 30 printable ASCII bytes;
2. a password within the configured length range, entered twice;
3. only the profile groups enabled by Sysop policy; and
4. the configured new-caller security/time policy.

Optional profile fields may be blank. Required fields must validate.
`/Q` at a profile prompt cancels the incomplete registration. Passwords are
stored only as salted Argon2id PHC hashes.

On a private board, new-caller registration is intentionally absent. Only an
existing active caller whose verified account meets the private security
threshold is admitted.

## Private-board onboarding limitation

There is no host-side `ADD CALLER` command yet. To prepare callers for a
private board without editing SQLite:

1. Bind listeners to loopback or another tightly controlled trusted network.
2. Keep the board public only for the controlled registration window.
3. Start `spitfire console` and let the intended caller register normally.
4. Use `SECURITY <level> <name>` to assign the private-board threshold.
5. Stop the console with `QUIT`.
6. Run `spitfire config`, switch the board to private, set the threshold, and
   select `S`.
7. Restart and verify the caller before exposing the listener.

Do not open a public Internet registration window for this workaround.

## Inspect and change callers

Start the board with the operator console:

```bash
spitfire console /path/to/board/spitfire.toml
```

Examples:

```text
CALLERS
DISABLE Example Caller
ENABLE Example Caller
SECURITY 20 Example Caller
PROFILE Example Caller
PROFILE-SET email Example Caller|caller@example.invalid
PROFILE-SET phone Example Caller|
```

Disabling prevents login without deleting messages, files, statistics, or the
caller record. Security changes affect menu, conference, file-area, private-
board, and traditional Sysop thresholds at their ordinary authorization
boundaries.

## Caller self-service

An authenticated caller can use:

- Main `Y` for caller statistics;
- Main `R` to view/edit enabled private profile groups;
- Main `U` for graphics/text, dimensions, paging, hot-key, and transfer
  preferences; and
- Main `X` for session-local expert-mode menu behavior.

The operator cannot see credentials. There is no host-side password reset,
caller rename, destructive delete/packing, public profile directory, or
arbitrary record editor in the Development Preview.

## Privacy rules

Profile contact values are private to that caller and the deliberate operator
profile commands. They do not appear in caller lists, node status, unrelated
sessions, or message/file presentation. Never include passwords, password
hashes, private messages, or contact data in support screenshots or public
logs.

For the complete model and policy evidence, see
[Native Caller and Authentication Model](../sfng-caller-authentication.md).
