# SPITFIRE Security and Accessibility Philosophy

## 1. Purpose

SPITFIRE is a hobbyist BBS platform, not an online banking system.

Security must therefore be appropriate to the real risks while preserving ease of use, compatibility, experimentation, and the character of traditional BBS operation.

The system should be secure enough to protect callers, Sysops, and host computers without requiring casual users to navigate excessive authentication or configuration.

## 2. Guiding Principle

The preferred model is:

> Safe by default, stronger when needed, compatible when reasonable.

Security mechanisms should reduce meaningful risk rather than merely increase complexity.

## 3. Caller Authentication

Traditional caller accounts should remain simple.

A normal caller should generally need:

- caller name or handle
- password

Password rules should encourage reasonable strength without requiring absurd complexity.

The system should prefer minimum password length and detection of obviously weak passwords over rules such as mandatory punctuation, capitalization, or frequent forced password changes.

Optional stronger authentication may include:

- TOTP
- passkeys
- recovery codes

These should normally remain optional for ordinary callers.

## 4. Sysop Authentication

Sysop administration presents greater risk and should receive stronger protection.

A Sysop may still log into the BBS in the traditional manner.

Administrative functions exposed through the web interface may optionally or by default require stronger authentication.

Possible protections include:

- stronger password requirements
- TOTP
- passkeys
- session expiration
- administrative reauthentication for sensitive changes

The goal is to protect the system without making routine Sysop work cumbersome.

## 5. Telnet

Telnet remains important to BBS culture and should be supported.

It should not be removed merely because stronger transports exist.

The system should:

- clearly identify Telnet as an unencrypted connection
- allow Sysops to enable or disable it
- support traditional terminal clients
- avoid transmitting highly sensitive administrative secrets through it when practical

SSH and browser-based encrypted access should be available as alternatives.

## 6. Web Access

SPITFIRE should include a web component.

The web interface may provide:

- BBS information
- current system status
- file listings
- message access where enabled
- account functions
- Sysop administration
- an embedded BBS terminal

The embedded terminal should connect directly to the SPITFIRE session engine through a controlled gateway rather than acting as an unrestricted proxy to the public Telnet port.

HTTPS should be the normal deployment mode for Internet-accessible systems.

## 7. Local Operation

SPITFIRE must remain usable as a local application.

A Sysop should be able to run and administer a board without requiring:

- a public domain name
- cloud services
- external identity providers
- Internet connectivity

The web interface should be capable of operating on localhost or a private LAN.

Internet-facing deployment should be optional.

## 8. Personally Identifiable Information

SPITFIRE should collect only information that is useful to the operation of a BBS.

Historical fields that no longer make sense should not automatically be mandatory merely because they existed in earlier versions.

Examples of information that should generally be optional include:

- street address
- telephone number
- full legal name
- birth date

Where historical file compatibility requires these fields, empty or privacy-preserving values should be supported.

Passwords must never be stored as plaintext.

Sensitive account information should not be unnecessarily exposed through APIs, logs, display macros, or network packets.

## 9. Legacy Compatibility

Legacy support inherently carries some risk.

The project should therefore distinguish between:

- compatibility
- trust

An old QWK packet, CircuitNet packet, RIP file, archive, door program, or imported data file should be treated as untrusted input even when its format is supported.

This does not mean preventing people from using historical material.

It means parsing it defensively.

## 10. DOS Doors

Legacy DOS doors should remain supported where practical.

They should run inside a restricted compatibility environment so that a vulnerability in an old door does not automatically compromise the SPITFIRE server or host operating system.

The compatibility environment should have access only to the files and resources it actually requires.

Network access may be granted when a particular door requires it.

Security should not unnecessarily prevent known legitimate door behavior.

## 11. Network Services

Each network service should expose only the functionality it requires.

Potential services include:

- Telnet
- SSH
- HTTPS
- WebSocket terminal access
- FidoNet/BinkP
- CircuitNet
- QWK networking

A vulnerability in one service should not automatically grant unrestricted access to the others.

## 12. CircuitNet

Historical CircuitNet behavior may be preserved even when its original authentication assumptions are no longer acceptable.

Legacy packet formats may remain supported.

Modern CircuitNet operation should be capable of authenticating nodes using cryptographic identity rather than trusting message fields alone.

Strict modern authentication should not prevent offline preservation, packet inspection, or historical experimentation.

## 13. File Uploads

File uploads are fundamental to BBS operation and must remain supported.

The server should protect itself against:

- invalid filenames
- path traversal
- oversized uploads
- malformed archives
- accidental overwrite of system files

Optional virus or malware scanning may be provided but should not become an unavoidable requirement for running a private hobby board.

## 14. Logging

Logging should help the Sysop understand what the board is doing.

Useful events include:

- caller logins
- failed authentication
- network connections
- door launches
- message-network activity
- administrative changes
- unusual errors

Logs should avoid recording passwords, authentication secrets, or unnecessary personal information.

## 15. Security Levels

Traditional SPITFIRE security levels should remain part of the BBS authorization model.

They control caller access to BBS functionality.

Operating-system or server-administration privileges should remain separate from caller security levels.

A caller reaching the highest SPITFIRE security level should not automatically gain host operating-system access.

## 16. Defaults

A fresh installation should be reasonably safe without extensive configuration.

At the same time, advanced security should not be mandatory for a Sysop running a private BBS on localhost or a home LAN.

Deployment context matters.

The software should support sensible presets such as:

### Local / Preservation

Optimized for experimentation and historical compatibility.

### Private Network

Suitable for LAN or VPN operation.

### Internet BBS

Enables recommended public-facing protections.

These should remain presets rather than artificial product editions.

## 17. Philosophy

SPITFIRE should never punish someone for wanting to explore a thirty-year-old BBS format.

Nor should nostalgia require exposing passwords, personal information, or the host computer unnecessarily.

The project will therefore modernize security primarily where the world around SPITFIRE has changed, while leaving the BBS experience itself familiar.

The desired result is not:

> maximum possible security at any cost.

It is:

> enough security that a SPITFIRE Sysop can confidently put the board online, without making the software miserable to use.
