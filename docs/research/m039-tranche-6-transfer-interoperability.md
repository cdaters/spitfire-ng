# M039 Tranche 6 — Independent Transfer Interoperability

Status: **RIGHTS-SAFE PUBLIC EVIDENCE SUMMARY**

SPITFIRE NG B-024 interoperability was tested against independently
implemented peers:

| Protocol boundary | Independent peer | Result |
|---|---|---|
| XMODEM/YMODEM/ZMODEM caller paths | Qodem 1.0.1 and SyncTERM 1.9rc4 | Representative current-schema transfers passed |
| 1K-XMODEM-g | Original DOS Qmodem 4.6 Test-Drive | Both supported directions passed |
| YMODEM-g Batch | Original DOS Qmodem 4.6 Test-Drive | Two-member batches passed in both directions |
| ZMODEM Batch upload | lrzsz 0.12.20 | One real three-member upload passed |
| TeLink | Original DOS BinkleyTerm 2.59 | Both directions passed |
| SSH carrier | macOS OpenSSH | Completed binary transfer and bounded disconnect passed |

Each accepted transfer verified generated member names, exact logical lengths,
SHA-256 values, clean termination, one settlement per successful item, and no
residual quota reservation or active-use lease.

The historical programs and lrzsz were external test peers only. This project
does not redistribute their binaries, source, packages, captures, emulator
images, or test payloads. Committed automated tests use deterministic
project-authored data.

TeLink remains an ordinary member of B-024's required protocol set. SEAlink is
a distinct protocol and was not substituted for TeLink.
