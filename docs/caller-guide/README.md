# SPITFIRE NG Caller Guide

<!-- help-topic: caller.guide -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)
>
> **Latest downloadable release:** SPITFIRE NG 0.1.0 Development Preview
>
> Individual boards choose their own menus, access levels, and available
> features, so what you see may differ from the examples here.

This guide is for people calling a SPITFIRE NG board. It explains how to
connect, log in, move through menus, use messages and files, change caller
preferences, and log off cleanly.

If you run the board, use the [Sysop Reference Manual](../manual/README.md).
Implementation details are in the [Technical Reference](../technical/README.md).

## Contents

- [Connecting](#connecting)
- [Logging in](#logging-in)
- [Your account and handle](#your-account-and-handle)
- [Menus and commands](#menus-and-commands)
- [Messages and conferences](#messages-and-conferences)
- [Files and searching](#files-and-searching)
- [Tagging and batch downloads](#tagging-and-batch-downloads)
- [Uploading](#uploading)
- [Transfer protocols](#transfer-protocols)
- [Account preferences](#account-preferences)
- [Logging off](#logging-off)
- [Common problems](#common-problems)

## Connecting

<!-- help-topic: caller.connecting -->

Your Sysop should give you the board address, port, and connection type. A
terminal program normally uses ANSI-BBS presentation, an 80 by 25 screen, and
CP437 for a traditional BBS connection. The board can also provide a simpler
text presentation when needed.

SSH is encrypted. On its first connection, an SSH client shows the board's
host-key fingerprint; compare it with the value published by the Sysop before
accepting it. Telnet, RAW TCP, and RLogin are compatibility transports and do
not encrypt your password or session. Use them only on a network you trust or
through protection arranged by the Sysop.

## Logging in

<!-- help-topic: caller.login -->

An SSH call asks for your login identifier and password before the BBS screen
appears. After successful authentication, SPITFIRE NG does not ask for the same
password again.

On other caller transports, the BBS asks whether you are a new caller. A
returning caller answers No and enters the requested caller name and password.
If the board permits registration, a new caller answers Yes, chooses an
available caller name and password, and supplies only the profile information
required by that board.

Passwords do not appear as you type them. If a normal terminal displays your
password, disconnect and ask the Sysop whether the client and transport are
configured correctly.

## Your account and handle

<!-- help-topic: caller.account -->

Your login identifier is used to authenticate. Your display handle is the name
other callers normally see. A board may request optional private profile
information, but private information is not part of the public caller
directory unless the board provides a specific safe public field and you opt
in where required.

Account availability, security level, daily time, and subscription policy are
set by the board. A clear denial or warning should tell you when one of those
limits affects a call; contact the Sysop rather than creating a second account
to avoid a restriction.

## Menus and commands

<!-- help-topic: caller.menus -->

Enter the highlighted command letter. Depending on your terminal, you may
need to press Enter after it.

- `M` enters Messages from the Main menu.
- `F` enters Files.
- `?` shows help for the current menu.
- `Q` normally returns to the previous or Main menu.
- `G` displays Goodbye and logs off.
- `X` changes the amount of menu prompting for the current session.

Commands are filtered by your current access. A board may customize command
letters and presentation, so the menu on screen is the final guide.

When output pauses at `MORE`, press Enter for another page, `S` to stop that
output, or `N` to continue without more pauses for that output.

## Messages and conferences

<!-- help-topic: caller.messages -->

The Message section lets you change conference, read or browse messages, enter
a new message, reply, search, and review Your Messages. Conferences separate
different subjects and may have different access rules.

When entering a message, pressing Enter at the recipient prompt chooses All
Callers when the conference permits it. A blank body line opens the editor
commands. Save with `S` and confirm with `Y`; abort if you do not want to post.
Private messages are visible only to their allowed participants and authorized
Sysops.

## Files and searching

<!-- help-topic: caller.files -->

The File section lets you change file area, list files, find a filename, search
descriptions, view new files, read safe text, inspect supported archives,
download, and upload where the board allows it.

File areas can have different access, upload, Preview, no-charge, and storage
rules. A Preview area may allow listing and inspection while denying transfer.
An external drive that is temporarily unavailable does not erase the file from
the catalog; try later or contact the Sysop.

## Tagging and batch downloads

<!-- help-topic: files.batch-downloads -->

Tagging puts several files into a queue for one transfer. Review the queue's
file count, total bytes, and chargeable total before starting. SPITFIRE NG
checks access and allowance again before each file transfers.

YMODEM Batch, YMODEM-g Batch, and ZMODEM Batch can send more than one queued
file in one batch. ASCII, XMODEM variants, and TeLink are single-file choices.
If a tagged file changes or becomes unavailable, refresh or remove that item;
SPITFIRE NG does not silently send a different file under the old tag.

## Uploading

<!-- help-topic: caller.uploading -->

Choose Upload in an area that permits it, enter a safe filename and
description, then select a protocol supported by your terminal. The board may
warn about a similar filename, reject a prohibited name, or hold the upload
for Sysop review. A completed transfer is not necessarily visible to other
callers until board policy accepts it.

Never put a local directory path into a remote filename. SPITFIRE NG accepts a
filename, not permission to choose a location on the server.

## Transfer protocols

<!-- help-topic: caller.transfer-protocols -->

Current SPITFIRE NG provides:

- ASCII for bounded seven-bit text;
- XMODEM Checksum and XMODEM CRC;
- 1K-XMODEM and 1K-XMODEM-g;
- YMODEM Batch and YMODEM-g Batch;
- ZMODEM Batch; and
- TeLink.

Choose a protocol your terminal actually supports. The client normally uses
its upload or download command after the BBS begins protocol negotiation.
Failed and cancelled files are not counted as successful transfers.

## Account preferences

<!-- help-topic: caller.preferences -->

The Main-menu preferences command lets you choose supported terminal and
paging behavior for your account. Useful choices include graphics or text,
screen dimensions, page length, and whether long output pauses. Reconnect after
changing a presentation choice if the current screen is already difficult to
read.

## Logging off

An authorized operator may invite you to a Sysop chat. Answer the displayed
accept/decline prompt; there is no hidden takeover or screen observation.
During accepted operator-initiated chat your ordinary caller time pauses, then
resumes when chat ends and you return to your previous BBS context. Chat is
not recorded as a message or durable transcript. You may also use the existing
Page command when the Sysop is available; a page may be answered or declined.

An operator can end your session with a Sysop-disconnect notice or without
that notice. Both paths still finalize your session and any active transfer.
A board-wide shutdown has its own shutdown notice and closes the board after
bounded cleanup. Reconnect after the Sysop starts the board again.

<!-- help-topic: caller.logoff -->

Use `G` for Goodbye and wait for the board to close the connection. A clean
logoff lets SPITFIRE NG finish the caller record and release the node. Closing
the terminal window is an emergency disconnect, not the normal logoff method.

## Common problems

<!-- help-topic: caller.problems -->

- **The screen is garbled:** select ANSI-BBS and CP437 for a traditional
  connection, or ask the Sysop for the board's text settings.
- **Lines wrap or menus do not fit:** use the dimensions recommended by the
  Sysop, commonly 80 by 25.
- **A command is missing:** your account, conference, or file area may not have
  access, or the board may use a customized menu.
- **A transfer does not start:** confirm that the terminal supports the chosen
  protocol and that you invoked the client's matching send/receive action.
- **SSH reports a changed host key:** stop and ask the Sysop to verify the new
  fingerprint. Do not routinely bypass the warning.
- **You reached a daily or ratio limit:** failed transfers should not count as
  successes; ask the Sysop to review the account if the displayed result looks
  wrong.

For a problem specific to one board, contact that board's Sysop. Include the
connection type, terminal name/version, the menu you were using, and the exact
error text, but never send your password.
