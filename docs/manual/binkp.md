# BinkP mail transport

BinkP moves queued FTN packets between configured mailers over TCP. The
[native FTN core](ftn-core.md) still owns addresses, NetMail, EchoMail, scanning,
tossing, routes and duplicate protection. A successful BinkP acknowledgement
means that the peer took custody; it does not prove a recipient read the message.

N4 supports client polling and an inbound daemon listener, with controlled
independent interoperability. Live public FidoNet participation is not claimed or
authorized by this guide. Use an isolated peer and addresses agreed for that test.
FileEcho, TIC, FREQ, AreaFix and scheduled polling are not implemented.

## Configure an isolated link

First configure the N3 domain, local AKAs, FTN link, routes, private recipient
aliases and EchoMail mappings. BinkP refers to those existing IDs. It does not
create a route or give a remote sender a local caller identity.

Use `sfconfig --board BOARD --apply-binkp POLICY.json` while attached with
`change-sensitive-configuration`; add `--offline` only when the board is stopped.
The file is a bounded typed replacement for the BinkP section. For example, after
creating FTN IDs `node`, `point` and `peer`:

```json
{
  "listener": {
    "enabled": true,
    "bind": "127.0.0.1:24554",
    "akas": ["node", "point"]
  },
  "links": [{
    "link": "peer",
    "enabled": true,
    "outbound": true,
    "inbound": true,
    "endpoint": "127.0.0.1",
    "port": 24555,
    "directory": false,
    "akas": ["point", "node"],
    "remote_akas": [],
    "auth": "require-cram",
    "allow_domainless": false
  }]
}
```

The local AKA list is the only set advertised for that link. It must include the
N3 link's default AKA. Remote aliases, if needed for packets from a peer's other
AKAs, are explicitly configured typed endpoints and must also be presented during
that session. The expected primary remote identity must always be present.
Different domains remain separate even when their numeric addresses are identical.

An explicit endpoint overrides directory data. Set `endpoint` to `null` and
`directory` to `true` to use an effective, usable N3 directory generation: one
`IBN` service with its own host, or an `INA` host plus `IBN` port. Ambiguous or stale
data fails; system names never become guessed Internet hosts. There is no automatic
nodelist download or public DNS-derived FTN hostname fallback.

Listener enable/bind/AKA changes require daemon restart to take full effect.
Disabling an existing listener stops new admissions immediately. Link restrictions
and credential changes cancel affected active work; unacknowledged work stays safe.

## Credentials and explicit actions

Set a credential with `sfconfig --board BOARD --binkp-password peer`. The local
prompt hides input; the credential is not an argument or a returned configuration
value. Status is only Missing, Configured or Invalid. Empty input is rejected.
Clearing requires `sfconfig --board BOARD --clear-binkp-password peer`.
These credential operations require a running daemon and sensitive-configuration
permission. The credential is shared with the controlled peer through an appropriate
private channel. Never paste it into logs or examples.

Default `require-cram` requires FTS-1027 CRAM-MD5. `allow-plain` is an explicit
legacy compatibility policy, not an automatic downgrade from required CRAM. CRAM
protects the password exchange but does not encrypt mail, and the base protocol
does not cryptographically authenticate the answerer to the caller. TLS is deferred.

The daemon owns all sessions. Local commands request typed operations:

| Command | Required explicit capability | Effect |
| --- | --- | --- |
| `spitfire binkp-status CONFIG` | `network-status` | Safe policy digest, credential status and link health |
| `spitfire binkp-test CONFIG peer` | `network-test` plus status access | Resolve/connect/authenticate/check identity; no local mail custody |
| `spitfire binkp-poll CONFIG peer` | `network-run` plus status access | Exchange existing queued packets; inbound packets toss automatically |
| `spitfire ftn-queue CONFIG` | `network-status` | Existing N3 queue IDs, versions and safe state |
| `spitfire binkp-hold CONFIG QUEUE VERSION` | `network-queue` | Hold unclaimed work using its expected version |
| `spitfire binkp-release CONFIG QUEUE VERSION` | `network-queue` | Review/release eligible held, retry or failed work |

Test Link is limited to once a minute per link. An eager peer may offer mail during
a test; SPITFIRE declines it with M_SKIP. Poll Link returns a started session ID;
inspect status for completion. It does not start an independent shell command or
accept an arbitrary hostname. `sfmonitor` shows a small read-only Networks section
on its dashboard; the CLI owns the minimum typed requests, not a full Networks TUI.

## Queues, failures and recovery

Scan local native messages through N3 before polling. The mailer builds/reads the
same immutable queued artifacts, sends a batch, and records acceptance only after
matching M_GOT. A socket write or clean TCP close is insufficient. The listener
stages an entire packet privately and calls the N3 tosser before acknowledging it.
Unknown areas or malformed packets become N3 quarantine; private text is never a
status message. Partial files are held only in bounded memory and discarded.

Transient failures retain work with five-minute exponential backoff, capped at six
hours plus up to thirty seconds jitter. Twelve attempts or seven days end automatic
eligibility. No periodic polling scheduler exists. Authentication, address, protocol,
limit and custody failures hold the link for review. Fix configuration or credentials;
a successful Test Link can establish health again. Held queue work still needs an
explicit versioned release. Exhausted attempt histories require a future corrective
queue decision rather than reusing attempt numbers.

A hold cannot revoke custody in an active session. Wait for its bounded completion;
restricting the link cancels transfer, but an acknowledgement already received is
still true. Two total sessions and one per link prevent competing queue senders.

Graceful shutdown stops admissions and cancels active workers boundedly. Restart
clears stale session ownership and preserves accepted receipts. Backup includes
configuration, credentials, native FTN authority, directory generations, receipts,
health and immutable artifacts. Restore creates a new daemon generation, clears
active claims and holds unsent work. Review the restored route and peer history
before releasing that work. Origin serial allocation remains held: a backup cannot
prove which MSGIDs were used after its snapshot. Reconcile a verified serial floor
before new origination; automated serial-floor reconciliation remains a later
explicit operator operation. Existing frozen packets can be reviewed and released.
Never run the restored board concurrently under the original live identity.

See [Technical Reference](../technical/binkp.md),
[backup/restore](../sfng-backup-restore.md) and
[N4 acceptance report](../research/m048-networking-n4-binkp.md) for exact guarantees,
limits and demonstrated interoperability. Windows live BinkP and operator-surface
acceptance remains **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**.
