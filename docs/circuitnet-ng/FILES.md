# Files on CircuitNET NG

CircuitNET can distribute approved files between participating boards. SPITFIRE
NG owns the local File Areas, file records and original contents; CircuitNET
carries eligible files through configured file-area mappings and subscriptions.

A message conference subscription does not subscribe to a file area. File-area
codenames and subscriptions are configured separately with your HOST. There is
not yet an official governed network file-area catalog. Do not assume a message
codename names an authorized file distribution area.

Receiving boards apply their own archive inspection, scanning and approval rules.
A file may be quarantined instead of published. A remote clean-scan claim does
not bypass required local scanning. Scanner failure is not a clean result.

Transfers use authenticated, encrypted links. After publication, a file is
available according to the receiving BBS's File Area access policy. Transport
security does not make a downloadable file confidential.

Normal CircuitNET Poll and Events can exchange files alongside messages and
subscription controls. File requests are not currently supported. The network
may later use approved file areas to distribute this kit, catalogs and utilities;
installing this kit does not enable that distribution automatically.

## One file from your BBS to your HOST

Agree a file-area codename and safety/access rules with your HOST first. KITDOCS
below is an agreed example, not an official governed network file area. Your
board is USAZ017, your HOST USAZ000. Use unused local File Area numbers: the example
uses 7 on your board and 9 at the HOST. Local numbers do not travel with the file.

Remote file-subscription requests are not implemented. Each operator configures
their own direct-neighbor file subscription on a stopped board. Message Dossier
requests cannot subscribe a File Area.

On your stopped board, create an area, set its description/access and required
scan/review policy. Configure an already operating scanner; SCANNER_ADDRESS is
your approved local ClamD endpoint, including its port, not a network HOST address:

```
sfconfig files spitfire.toml area 7 network-docs "Network documents"
sfconfig files spitfire.toml edit-area 7 "Network documents" "Documents" 10 10
sfconfig files spitfire.toml policy 7 required approval
sfconfig files spitfire.toml scanner 7 SCANNER_ADDRESS
sfconfig circuitnet spitfire.toml file-map circuitnet-ng KITDOCS 7 \
  yes yes 1048576
sfconfig circuitnet spitfire.toml file-subscribe circuitnet-ng \
  USAZ000 KITDOCS yes
```

The mapping enables send/receive with a 1 MiB network limit. At USAZ000, the HOST
creates its own area 9 and local scanner/policy, maps KITDOCS to 9 with the same
agreed limit, and runs file-subscribe for USAZ017, KITDOCS, yes. These separate
file subscriptions are directional just like message Dossiers. Never put a
scanner credential or private endpoint into the public network application.

Import a small safe file you intend to distribute:

```
sfconfig files spitfire.toml import 7 notes.txt NOTES.TXT "Network notes"
sfconfig files spitfire.toml status
```

Inspection and scanning run before approval. Use the returned file ID with
`sfconfig files spitfire.toml approve FILE_ID` only after status permits approval.
If FILE_ID.DIZ is found in a supported archive, choose description FILE_ID use,
edit TEXT, or ignore; the original archive bytes remain unchanged.

Start both boards and Poll USAZ000 from USAZ017, or let its normal CircuitNET
Exchange Event run. The same authenticated connection can carry messages, controls
and approved file work. At the receiver, hash verification and its own inspection,
scan and approval policy run. Remote clean claims never override required local
scanning. With manual approval, the HOST stops its board and reviews/approves the
received object before it appears in area 9. A received or quarantined receipt is
not a claim that the file was published. Check file status and native Files listings.

## Limits to expect

- ZIP, TAR and GZIP/TGZ have bounded inspection. Recognized unsupported formats,
  encrypted archives, unsafe paths and resource-limit failures stay unpublished
  or quarantined; a familiar extension does not make an archive safe.
- Required scanning fails closed when the scanner is absent or errors. It is not
  bundled or installed by the kit. Quarantine retains evidence; rescan with current
  policy before approval. Do not disable scanning just to clear an error.
- Interrupted payload transfers restart from byte zero. Final SHA-256 verification
  is mandatory; already-owned content can avoid retransmission through hash-have.
- There is no file request service, automatic kit publication, remote file Dossier
  editor or official file-area catalog. Agree each mapping and subscription locally.

The [Files specification](../technical/files-custody.md) lists archive limits,
formats, backup and reference-safe storage behavior. It is the deeper reference
when troubleshooting; routine file publication uses the commands above.
