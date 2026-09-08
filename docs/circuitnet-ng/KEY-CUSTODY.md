# Catalog signing-key custody and recovery

The catalog signing key authorizes network conference definitions. It is separate
from every node's live TLS key. The designated publication custodian holds it
under the Administrator's documented authority. The Secretary records custody,
public fingerprint, activation checkpoint and approved changes, never the secret.

## Normal custody

Keep the signing key on a protected signing system or encrypted removable medium,
with access limited to the designated custodian. It need not be continuously online
on ROOT. Route messages and files with the public catalog authority only.

If the network chooses a recovery copy, document why recovery is preferable to
fresh enrollment. Keep one encrypted, access-controlled copy separately from the
signing system, with its unlock material held separately. Record who can recover
it. Do not put the key in ordinary downloadable backups, this kit or a File Area.
Minimize copies; a signature key backup also creates another way to impersonate
the publisher. Public keys, signatures, revision history and transition records
should be retained for long-term verification.

Test recovery on an isolated protected system: recover the key, derive and compare
its public fingerprint, sign a disposable draft and verify it without publishing.
Record the result, remove the working recovery copy, and plan replacement after
actual emergency recovery. Loss of access does not prove the key was not copied.

## Protected signing

The offline `catalog-artifact` utility is built from the SPITFIRE NG source:

```
cargo build -p sf-net --example catalog-artifact
```

Use the resulting executable on the protected signing system. In these examples
it is available as `catalog-artifact`. It operates on local files and does not
contact a BBS. With a restrictive file-creation mask or equivalent OS ACLs:

```
umask 077
catalog-artifact sign draft.json signing.key signed.json
catalog-artifact validate signed.json authority.json
```

Review the draft, approval reference, current predecessor and intended changes
before signing. Return only signed.json to the stopped ROOT board:

```
sfconfig circuitnet spitfire.toml catalog-import circuitnet-ng \
  signed.json
```

The ordinary local `catalog-publish` command also signs and commits, but requires
the private key at that console. Neither option changes the need for governance
approval. Signed import still enforces the pin, lifecycle and consecutive chain.

## Planned replacement

Freeze catalog publication at an agreed revision/hash. Keep normal routing running.
Bring participating boards to that exact checkpoint and record the approval and
new public fingerprint by an independent administrative channel.

Generate a new key on the protected system. The utility prints only its public key:

```
catalog-artifact key replacement.key
catalog-artifact rotate catalog.json authority.json signing.key \
  replacement.key transition.json APPROVAL "Scheduled key replacement"
```

The transition is signed by both keys and binds their public identities to the
exact current catalog checkpoint. It does not change the network, catalog identity
or publisher Node ID. Confirm the printed new fingerprint independently; copying
it only from an untrusted transition file is not verification.

On EACH stopped participating board, an authorized local operator runs:

```
sfconfig circuitnet spitfire.toml catalog-replace-key circuitnet-ng \
  transition.json NEW_FINGERPRINT confirm-key-replacement
```

This sensitive-configuration operation validates the signatures, current pin and
exact revision/hash, stores the transition, audits it and changes only future
signing authority. Repeating that exact transition is harmless. Wrong fingerprints,
wrong signatures, reused old keys and mismatched checkpoints fail closed.
There is no automatic trust replacement through Poll or ordinary pin configuration.

Confirm adoption at all participating nodes before publishing the next revision
with the new key. Lagging nodes retain ordinary service under the previous catalog
but cannot accept newly signed revisions until their operator completes enrollment.
Keep the old PUBLIC key and history. Retire the old private key from use and dispose
of secret copies under the recorded custody policy after the transition is verified.

## Lost key

If there is a known protected recovery copy and no evidence of compromise, recover
and verify it using the procedure above, then make a planned replacement. If no
usable old key remains, use the emergency procedure. Do not reset the catalog,
re-sign historical revisions or edit the database pin.

## Compromise or unrecoverable loss

Notify the Administrator, Secretary and direct neighbors through established
contacts. Suspend catalog publication and acceptance of unverified new artifacts.
Hold affected links where necessary while investigating; this does not delete
queued work. Preserve the last independently agreed catalog and evidence.

The governance authority records an emergency replacement decision and the trusted
checkpoint. On a protected system generate a fresh key, then:

```
catalog-artifact recover catalog.json authority.json emergency \
  replacement.key transition.json APPROVAL "Emergency key replacement"
```

This proves possession of the new key; it cannot prove consent of the unavailable
or compromised old key. Each local Sysop must independently verify the emergency
decision and fingerprint, then use the same explicit `catalog-replace-key` command.
An arriving file is never sufficient authority. If a newly enrolled replacement is
lost before publishing anything, another explicit emergency transition may use the
same catalog checkpoint; no publication with the unavailable key is required.
The operation is audited as an
emergency rather than a dual-signed planned rotation.

If a board has already accepted a conflicting or malicious later catalog, STOP:
this recovery command will not roll it back or force a mismatched checkpoint.
Preserve that board and reconcile the incident with the Administrator. A reviewed
replacement/recovery from agreed trusted history is necessary before reconnecting;
never erase the only evidence to make enrollment succeed. This is a deliberate
limit of the recovery path, not permission for silent rollback.

## Trust history, restore and continuity

Existing catalog revisions continue to verify under their original public keys.
The next revision binds to the same previous hash and uses the replacement key.
Keep transitions with the catalog history in protected board backups. Restoring
an older pin over known newer key state is rejected.

For a fresh replacement node, enroll the original trusted authority, import its
consecutive catalog revisions through each replacement checkpoint, apply each
verified transition locally, then continue with later revisions. A current-key
file alone cannot verify history signed by earlier keys. The administrator must
supply the retained public trust history, not private keys. Future kits carrying
a rotated history must include those public transition artifacts and instructions.

Message/file routing continues under the last accepted catalog while signing is
unavailable. Announce that catalog changes are paused, rather than leaving ROOT
online with an unprotected emergency secret. Publisher-node or network identity
migration is separate from same-publisher key replacement and requires a reviewed
migration plan.
