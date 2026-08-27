# SPITFIRE Compatibility and Preservation Principles

## 1. Compatibility Is a Feature

Historical compatibility is not temporary migration scaffolding.

Where practical, compatibility with original SPITFIRE resources should remain a supported feature of the project.

Old files should not become second-class citizens merely because newer formats become available.

## 2. Original Resource Formats

The project should attempt direct compatibility with the following historical resources.

### Menus

Original menu files such as:

- `SFMAIN.MNU`
- `SFMSG.MNU`
- `SFFILE.MNU`
- `SFSYSOP.MNU`

should remain readable and usable.

Original command identifiers and security behavior should be preserved wherever known.

Extensions to the format should be backward-compatible.

### Help

`SPITFIRE.HLP` should remain a native supported format.

The original record structure should be documented and supported by modern editing tools.

### Display Files

The following should remain supported:

- `.BBS`
- `.CLR`
- `.RIP`

SPITFIRE display macros and caller substitution codes should remain functional.

New display capabilities may be added without invalidating original files.

### Caller Database

Original caller information should be importable and, where practical, directly readable.

Modern storage may contain additional information not present in the original caller records.

### Message Bases

The historical message system should remain a first-class supported backend if the file formats can be reliably documented.

Expected legacy components include:

- `SFMSGx.DAT`
- `SFMSGx.PTR`
- `SFMSGx.IDX`
- `SFMSGx.LMR`
- message conference configuration

The implementation should preserve the logical behavior of:

- message numbers
- conference numbers
- private messages
- received flags
- deleted messages
- threading
- last-message-read pointers
- netmail indicators

## 3. Multiple Message Backends

The modern SPITFIRE message engine should not be tied to a single storage implementation.

Potential backends include:

- native SPITFIRE message bases
- Synchronet SMB
- modern internal database storage
- gateway or virtual message bases

A caller should interact with these through the normal SPITFIRE message interface.

The underlying storage mechanism should not require a different caller experience.

## 4. QWK

QWK support should be native.

The historical LAKOTA experience should be preserved where practical, while the modern implementation may perform QWK processing internally.

Support should include:

- QWK packet creation
- REP import
- conference mapping
- last-message-read handling
- private messages
- QWK networking
- modern interoperability extensions where appropriate

## 5. DOVE-Net

DOVE-Net compatibility should be pursued through the appropriate QWK networking conventions.

DOVE-Net should appear to SPITFIRE callers as normal message conferences.

The system should not require conversion of the entire BBS to Synchronet architecture merely to participate in DOVE-Net.

## 6. FidoNet

FidoNet support should become a native networking capability.

Modern TCP/IP transport should be preferred while retaining compatibility with established FTN concepts including:

- NetMail
- EchoMail
- addresses
- zones
- nets
- nodes
- points
- message attributes
- routing
- duplicate detection

External mailers and tossers may also be supported.

## 7. CircuitNet

CircuitNet should be preserved as both a historical artifact and, if practical, a functional networking option.

The project should document:

- node identifiers
- hubs
- dependent nodes
- dossiers
- routing
- conference identifiers
- packet formats
- duplicate detection
- historical transport assumptions
- remote control behavior

Historical packet compatibility may be retained while modern transport and authentication are introduced separately.

A modern CircuitNet implementation should distinguish between:

### Legacy Mode

Maximum historical packet and workflow compatibility.

### Secure Mode

Original CircuitNet concepts carried over modern authenticated transport.

## 8. Doors

SPITFIRE should continue to support the concept of doors.

### Native Doors

Modern programs or extensions executed directly by the host operating system.

### Legacy DOS Doors

Original DOS software executed using a compatibility environment where necessary.

The BBS should generate historical drop files where practical, including:

- `DOOR.SYS`
- `SFDOORS.DAT`

Legacy doors should not require the entire SPITFIRE server to run under DOS emulation.

## 9. Configuration Philosophy

Original terminology should remain wherever possible.

Prefer:

- Caller
- Sysop
- Security Level
- Message Conference
- File Area
- Door
- Event
- Node

over generic enterprise terminology.

Configuration should remain readable and approachable.

Human-editable configuration should be preferred where reasonable.

## 10. Compatibility Testing

Historical files and utilities should serve as test fixtures.

Compatibility tests should include:

- reading original resources
- writing resources that original utilities can understand
- menu behavior
- help lookup
- message indexing
- caller record interpretation
- QWK round trips
- door drop files
- network packet round trips

Whenever possible, compatibility claims should be demonstrated rather than assumed.

## 11. Historical Documentation

Every reverse-engineered or reconstructed format should document:

- known fields
- field lengths
- data types
- byte order
- flags
- unused/reserved bytes
- version differences
- confidence level
- test files used
- unresolved questions

This documentation should become part of the permanent SPITFIRE preservation record.
