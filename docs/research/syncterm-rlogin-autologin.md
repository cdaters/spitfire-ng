# SyncTERM / Synchronet RLogin Auto-Login

## Purpose

This note records the exact credential-field convention implemented as an
optional SPITFIRE NG compatibility mode in Stock Core Increment 2. It is not a
claim that RLogin is secure or that standard RLogin authenticates a SPITFIRE
caller.

Research was performed on 2026-08-21 against current upstream SyncTERM and
Synchronet source at commit
`dc5eb88e3852dfa673c7c72ab5df955b89a21dbc` plus SyncTERM's published manual
and RFC 1282.

## Standard RLogin framing

[RFC 1282](https://www.rfc-editor.org/rfc/rfc1282) defines the initial stream
as four NUL-terminated fields:

```text
NUL
client-user-name NUL
server-user-name NUL
terminal-type/speed NUL
```

The RFC assigns usernames—not a password—to the first two nonempty fields. It
also describes historical host/source trust behavior. SPITFIRE NG does not
adopt those trust semantics.

## SyncTERM convention

The current [SyncTERM manual](https://syncterm.bbsdev.net/Manual.html) states
that its RLogin mode puts the user's password in place of the local/client
username. Its dialing-directory Username and Password fields are configurable.
The separate `RLogin Reversed` connection type exists for servers that swap
the two username fields.

Current `src/syncterm/rlogin.c` confirms normal SyncTERM RLogin writes:

| Wire field | SyncTERM value |
|---|---|
| client-user-name | configured BBS password |
| server-user-name | configured BBS username |
| terminal-type/speed | selected emulation, `/`, configured speed; speed defaults to 115200 |

The values are ordinary C byte strings terminated by NUL. There is no escape
layer; embedded NUL is therefore impossible. SyncTERM sends the handshake
before enabling its later RLogin XON/XOFF processing, so credential bytes are
not filtered as terminal flow control. The first two fields are bounded by
SyncTERM's configured username/password storage; Synchronet currently stores
the received name in its alias-sized field (25 characters) and the password in
its password-sized field.

The SPITFIRE NG adapter bounds every incoming NUL-terminated RLogin field to
256 bytes before session creation. Even within that protocol bound, an
auto-login password longer than the board's configured maximum is rejected as
an invalid automatic attempt and falls back to the ordinary bounded login
flow.

For `RLogin Reversed`, SyncTERM swaps its chosen username and password
pointers. SPITFIRE NG Increment 2 intentionally implements only the normal
Synchronet-compatible order. A client must select normal `RLogin`, not
`RLogin Reversed`.

## Synchronet server behavior

Current `src/sbbs3/answer.cpp` parses the first field into `rlogin_pass`, the
second into `rlogin_name`, and retains the complete terminal/speed string while
also splitting the terminal type. It uses the name to find the ordinary BBS
user and compares the supplied password against that user's credential.

For a recognized user with an invalid supplied password, Synchronet performs
bounded password handling and terminates after failure. An unknown or absent
RLogin name is not an authenticated identity; the surrounding Synchronet
logon/new-user path decides what follows. Current `newuser.cpp` also shows that
a supplied RLogin password can participate in its new-user path, subject to
its password policy.

These details establish a Synchronet/SyncTERM BBS compatibility convention,
not standard RFC 1282 password semantics.

## SPITFIRE NG policy and implementation

The listener option is explicit and defaults off:

```toml
[[transports]]
type = "rlogin"
listen = "127.0.0.1:2513"
auto_login = true
```

When enabled:

1. the first field is held as sensitive password bytes;
2. the second is treated as the claimed caller name;
3. terminal type and decimal speed remain terminal metadata;
4. both credential fields must be nonempty before an automatic attempt;
5. the pair is consumed by the same Argon2id caller verifier used by
   interactive login;
6. a valid pair enters the ordinary authenticated SPITFIRE session;
7. missing or invalid credentials fall back to normal bounded SPITFIRE login;
8. disabled/deleted callers remain denied; and
9. neither the password nor the credential-bearing handshake is logged or
   included in diagnostics.

Regardless of the option, only the second (requested server/BBS username)
field may be retained as untrusted transport identity metadata. The first
field is never placed in `TerminalInfo`, because a SyncTERM connection can put
a password there even when auto-login is disabled. No declared identity grants
caller access. SPITFIRE NG does not use privileged source ports, host trust,
remote/local username assertions, Unix identity, or terminal identity as BBS
authentication.

## Security warning

RLogin is plaintext. The password can be observed by anyone able to inspect
the network path. Auto-login should be limited to a trusted LAN, localhost,
VPN, or similarly controlled environment and is not recommended across the
untrusted Internet. Argon2id storage protects a stolen database; it cannot
protect a plaintext credential in transit.

Future SSH public-key mapping is a separate policy. The default SSH model
remains secure transport followed by normal SPITFIRE login.

## Verification

Synthetic coverage proves:

- bounded NUL framing and terminal/speed parsing;
- disabled-by-default credential extraction;
- normal SyncTERM field order;
- valid supplied credentials reach the shared caller/session engine;
- invalid supplied credentials do not bypass interactive authentication;
- declared RLogin identity alone is insufficient; and
- debug/transcript output does not contain supplied secrets.

No physical modem, historical executable, or proprietary resource is needed
for these tests.
