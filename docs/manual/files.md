# Files

Existing caller Files menus and transfer protocols use the native File Area and
publication rules described here. C6 acceptance status is recorded in the report below.

A File Area is a named collection with its own access and upload rules. A filename
is a label, not the identity of its contents. SPITFIRE keeps the original bytes and
uses a SHA-256 hash to detect identical content, even under different names.

## Import and review

Operator imports and policy changes require a stopped board and an authorized local operator:

```text
sfconfig files <config> areas
sfconfig files <config> area 1 utilities "Utilities"
sfconfig files <config> edit-area 1 "Utilities" "Approved utilities" 10 10
sfconfig files <config> enable-area 1 yes
sfconfig files <config> policy 1 required approval
sfconfig files <config> scanner 1 127.0.0.1:3310
sfconfig files <config> import 1 <source-path> UTILITY.ZIP "Uploader description"
sfconfig files <config> status
sfconfig files <config> approve <file-id>
sfconfig files <config> reject <file-id>
sfconfig files <config> rescan <file-id>
```

A newly created area requires scanning and operator approval. ClamAV is not bundled;
configure an already available local ClamD service. If required scanning cannot run,
the file stays unpublished. `optional` or `disabled` scanning is an explicit operator
policy choice. `automatic` replaces `approval` when safe files may publish directly.
Existing areas retain their earlier scan-disabled policy until configured.

Caller transfers use the existing upload handoff. A completed transfer does not
necessarily mean a published file: inspection, scanning and review come next.
Quarantined, pending and rejected files do not appear in ordinary caller listings
and cannot be exported through CircuitNET. A scanner failure does not mean clean.
Rescan uses current policy; approving a quarantined file first requires a satisfactory
new inspection. Rejection retains evidence rather than silently deleting it.

## Descriptions and archives

SPITFIRE recognizes actual content rather than trusting an extension. ZIP, TAR and
GZIP/TGZ inspection checks member paths and resource limits. Recognized formats
without a supported inspector remain unpublished. Encrypted archives cannot be
claimed fully inspected. No archive is rewritten during inspection or scanning.

A root FILE_ID.DIZ can suggest a description. The original bytes and detected text
encoding are retained, while display text is filtered for terminal safety. The
uploader's description remains until you choose:

```text
sfconfig files <config> description <file-id> use
sfconfig files <config> description <file-id> edit "Reviewed description"
sfconfig files <config> description <file-id> ignore
```

Long suggestions may need editing to fit the native description limits. Embedded
terminal commands never run as part of this workflow. Archive branding or repacking
must be a separate derivative; C6 admission preserves the original artifact.

## Duplicates, downloads and recovery

`limits AREA SOURCE EXPANDED MEMBER MEMBERS NESTING RATIO` adjusts bounded archive
limits in bytes/counts. Defaults and supported formats are listed in the
[technical matrix](../technical/files-custody.md#archive-operation-matrix).

An occupied filename with changed content requires a different name. Identical
bytes under another filename can reuse the payload. A repeated import of the same
filename and content returns the existing file. Matching DIZ suggestions are reported separately. No duplicate is silently deleted.

Published files remain subject to File Area access rules and the established caller
transfer engines. Check `list <area-number>`, `status` and `integrity` to distinguish
pending review, quarantine, missing payload and corrupt payload. Integrity reporting
does not remove or repair files automatically. Removing a listing is separate from
deleting shared content; physical cleanup is deferred.

Cold backup includes managed file bytes and native admission decisions. Restore
rebuilds content sharing after checking hashes. Missing or corrupt content prevents
a successful complete backup. Unfinished admission remains unpublished after restart.

[CircuitNET file distribution](circuitnet.md#file-distribution-c6) uses the same approved native
files and local receiving safety policy. TLS protects the transfer in transit; it does
not make published downloadable files confidential after arrival.

See the [technical contract](../technical/files-custody.md) and
[C6 report](../research/m069-files-circuitnet-distribution.md) for current acceptance.

C6 safety settings apply to new admissions and rescans. Review or rescan older files
explicitly when changing policy. Existing FTN receipt workflows cannot publish into
an area with explicit C6 safety configuration; those adapters retain their legacy
areas until a separate staged-admission integration is designed. This prevents an
older import path from bypassing the new policy.
