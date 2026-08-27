# SPITFIRE Web and Embedded Terminal Architecture

## 1. Purpose

This document defines the web architecture for the modern SPITFIRE Bulletin Board System.

The web layer should make SPITFIRE easier to discover, access, and administer without replacing the traditional BBS interface.

The web interface is an additional doorway into SPITFIRE.

It is not the BBS itself.

## 2. Design Principle

The traditional terminal interface remains the canonical SPITFIRE experience.

The web system should complement it through:

- an embedded BBS terminal
- public system information
- caller account functions
- optional message access
- optional file access
- Sysop administration
- system monitoring

A Sysop should be able to disable most web functionality while continuing to operate the BBS normally.

## 3. Deployment Modes

The web component should support several practical deployment styles.

### Local Only

Example:

    http://127.0.0.1:8080

Intended for:

- local administration
- testing
- preservation
- offline BBS installations

No public Internet access is required.

### LAN

Example:

    http://192.168.1.50:8080

Useful for:

- home networks
- private systems
- Raspberry Pi installations
- local retro-computing environments

### Internet

Example:

    https://bbs.example.com

Recommended for public systems.

Internet deployment should support HTTPS and secure WebSocket connections.

## 4. Web Architecture

Conceptually:

    Browser
       |
       | HTTPS
       v
    SPITFIRE Web Service
       |
       +---- Public Web Interface
       |
       +---- Caller Account Interface
       |
       +---- Sysop Administration
       |
       +---- WebSocket Terminal Gateway
                    |
                    v
             Session Engine
                    |
                    v
              SPITFIRE Core

The web service should communicate with the SPITFIRE core through defined application interfaces rather than directly manipulating BBS database files.

## 5. Public Web Interface

The public site may expose information such as:

- BBS name
- Sysop name or handle
- welcome message
- system description
- online status
- available connection methods
- active nodes
- public message conference list
- public file-area list
- historical information
- SPITFIRE version
- system registration number

Example:

    SPITFIRE Bulletin Board System

    The Dragon's Den

    Sysop: Alex

    Online Since: 1993
    SPITFIRE NG System: #0001

    Connections:
        Web Terminal
        Telnet
        SSH

    Nodes Online: 3

The public site should be customizable.

## 6. Embedded Terminal

A primary feature of the web interface should be an embedded terminal capable of presenting the normal SPITFIRE session.

The terminal should support:

- ANSI
- CP437
- keyboard input
- cursor control
- screen resizing
- copy/paste where appropriate
- full-screen operation
- mobile-friendly display where practical
- configurable fonts
- downloadable screen captures where appropriate

The terminal should display the same menus and screens presented to Telnet or SSH callers.

## 7. WebSocket Connection

The browser terminal should use a secure WebSocket connection when accessed through HTTPS.

Conceptually:

    Browser Terminal
          |
          | WSS
          v
    Terminal Gateway
          |
          v
    SPITFIRE Session

The browser should not connect indirectly to the public Telnet listener.

The WebSocket connection is its own SPITFIRE transport.

## 8. Terminal Transport Interface

The SPITFIRE session engine should receive terminal input through a generic interface.

For example:

    TerminalTransport

with implementations such as:

    TelnetTransport
    SSHTransport
    WebSocketTransport
    LocalConsoleTransport

The BBS should not care which transport produced a keypress.

## 9. Guest Access

A Sysop may choose to permit guest access through the browser terminal.

Guest access may:

- enter the BBS normally
- view welcome screens
- create a caller account
- browse guest-accessible areas

This should remain configurable.

## 10. Web Authentication

Web authentication may support:

- caller name or handle
- password
- optional TOTP
- optional passkeys

Ordinary caller accounts should not be forced to use MFA.

Sysop accounts may optionally require stronger authentication for administrative functions.

## 11. Web-to-BBS Session Handoff

A logged-in web caller should optionally be able to enter the terminal without typing credentials again.

Possible flow:

    Caller logs into website
            |
            v
    Clicks "Enter BBS"
            |
            v
    Server creates short-lived session token
            |
            v
    WebSocket terminal connects
            |
            v
    Token validated and consumed
            |
            v
    Normal SPITFIRE session begins

The token should be:

- short-lived
- single-use
- cryptographically random
- tied to the intended purpose

## 12. Traditional Terminal Login

Telnet and SSH callers should retain the familiar SPITFIRE login process.

Example:

    Enter Your Name:

    Enter Your Password:

The project should not require web authentication before using traditional BBS clients.

## 13. Web Messages

The web interface may optionally expose SPITFIRE message conferences.

Possible functionality:

- list conferences
- read messages
- post messages
- reply
- private messages
- thread view
- new-message view
- search

Web activity should operate through the same message engine used by terminal callers.

A message posted through the website becomes an ordinary SPITFIRE message.

## 14. Web File Areas

Optional web file-area access may include:

- browse file areas
- search
- view descriptions
- download
- upload
- show file statistics
- view new files

Existing SPITFIRE access controls must still apply.

A web caller should not gain access to a file merely because the web server can physically read it.

## 15. Caller Account Page

A caller may manage appropriate account information through the web interface.

Possible settings include:

    Password
    Handle
    Display name
    Location
    Terminal preferences
    Email address
    QWK preferences
    Privacy settings
    Optional MFA
    Passkeys

Sensitive historical fields should not be exposed unnecessarily.

## 16. Sysop Dashboard

The Sysop web interface may provide:

    System Status
    Active Nodes
    Caller Activity
    Message Statistics
    File Statistics
    Network Status
    Event Status
    Doors
    Logs
    Configuration
    Backups
    Updates

The dashboard should not replace the terminal Sysop menu.

Both should remain valid ways to administer the system.

## 17. Node Monitor

A modern equivalent of the classic multi-node status display may show:

    Node 1   Alex        Sysop Menu       SSH
    Node 2   RetroFan    Messages         Telnet
    Node 3   PixelMage   LORD             Web
    Node 4   Available

The web interface may update this view in real time.

## 18. Sysop Chat

The web dashboard may allow the Sysop to request chat with a caller.

This should integrate with the traditional SPITFIRE chat system rather than creating an unrelated chat platform.

## 19. API

The web service may expose an internal API.

The API should be versioned.

Example:

    /api/v1/system
    /api/v1/nodes
    /api/v1/messages
    /api/v1/files
    /api/v1/account

Not every API should necessarily be public.

Administrative APIs should require appropriate authorization.

## 20. Web Customization

Sysops should be able to customize the public website without modifying the SPITFIRE server source.

Possible customization:

    site title
    logo
    colors
    home-page content
    connection information
    system history
    custom CSS
    optional custom templates

The default appearance may take inspiration from classic SPITFIRE presentation while remaining usable on modern screens.

## 21. Embedded Terminal Customization

Possible terminal options include:

    CP437 font
    phosphor-style themes
    CRT-style visual effects
    modern clean terminal mode
    screen scaling
    80x25
    80x50
    custom dimensions

Visual effects should remain optional.

The terminal should prioritize accurate ANSI rendering over decorative effects.

## 22. RIP Graphics

If practical, the browser terminal may eventually become an especially useful environment for RIP graphics.

A browser implementation could interpret RIP commands and display vector graphics without requiring historical RIPterm software.

This should be considered a later compatibility feature rather than an initial requirement.

## 23. Sessions

Web sessions should use normal secure session practices.

Requirements should include:

- secure random identifiers
- expiration
- logout invalidation
- protection against session fixation
- secure cookies when HTTPS is enabled

Remembering a caller between visits may be optional.

## 24. Browser Security

Internet deployments should support protections such as:

- HTTPS
- secure WebSocket connections
- appropriate Content Security Policy
- origin validation
- CSRF protection
- output escaping
- input validation
- secure cookies

These should be configured by default where practical rather than requiring the Sysop to become a web-security specialist.

## 25. Reverse Proxy Support

The web server should operate correctly behind common reverse proxies.

Examples:

    Caddy
    nginx
    Apache
    Traefik
    Cloudflare Tunnel

SPITFIRE should also be capable of serving HTTP directly for simple installations.

## 26. TLS

SPITFIRE may support direct TLS termination.

However, use of a reverse proxy should remain a supported and documented deployment option.

Local-only installations should not require TLS.

## 27. Web Accessibility

The conventional web pages should follow modern accessibility principles.

The terminal itself is inherently specialized, but surrounding controls should support:

- keyboard navigation
- reasonable contrast
- readable labels
- screen-reader-friendly navigation where practical

## 28. Mobile Access

The website should be responsive.

The terminal should offer a usable mobile mode, potentially including:

- virtual function keys
- Escape key
- Ctrl key
- arrow controls
- Tab
- configurable BBS shortcut keys

The goal is not to make every DOS door perfect on a phone.

The goal is to make basic BBS access practical.

## 29. Local Web Administration

A default installation may bind administrative web access to localhost initially.

Example:

    127.0.0.1:8080

The Sysop may then explicitly enable LAN or Internet administration.

This provides a sensible default without complicating basic setup.

## 30. Guiding Principle

The web interface should make SPITFIRE easier to reach without replacing what makes a BBS a BBS.

Someone clicking:

    Enter BBS

should not receive a web recreation of SPITFIRE.

They should enter SPITFIRE itself.
