# About CircuitNET NG

<!-- public-identity:start -->
Official network home: https://circuitnetng.org/

These are canonical locations; web, mail and download deployment has not
been verified. See [JOINING-INFO](JOINING-INFO.md) for opening status.
<!-- public-identity:end -->

CircuitNET NG is a modern BBS network. Participating boards share public message
conferences and, where configured, approved files. Each Sysop runs an independent
BBS and chooses its local conference numbers, access rules and optional areas.

## How the boards connect

```
                 ROOT
                  |
         +--------+--------+
         |                 |
       HOST              HOST
      /    \                \
    END    END              END
```

ROOT is the top routing node for the configured network tree. A HOST serves
boards below it and routes traffic toward or from ROOT. An END is a normal
participating BBS connected through a HOST.

These roles describe routing, not who opens a connection. An END can make its
own scheduled poll to send and receive traffic, including from behind a firewall.

## Terms used in this kit

- Conference: a message discussion area. Most are open to participating callers;
  four operator areas are restricted to Sysops and verified visiting Sysops.
- Codename: the network's name for a conference, such as CHITCHAT. One BBS may map
  it to conference 7 and another to conference 42.
- Dossier: the conferences a direct neighbor subscribes to receive. Being listed
  in the catalog does not subscribe a board.
- Catalog: the official signed list of message conferences and their status.
- File Area: a BBS area holding files. Separate file-area mappings and
  subscriptions control CircuitNET file distribution.
- Event: a scheduled board operation, such as exchanging queued network traffic.

## When traffic moves

An eligible saved message enters a durable queue. Exchange can happen promptly,
on a schedule, by manual Poll, or with a combination of prompt exchange and
scheduled catch-up. A held or unreachable neighbor does not erase queued work.

Node-to-node transport is encrypted and authenticated. Public conference messages
are not end-to-end encrypted. After arrival, access follows the receiving BBS's
conference rules. Directed routing selects a destination; it is not private mail.
See [SECURITY](SECURITY.md) and [FILES](FILES.md).

## Your first BBS network

Some Sysops already run other BBS networks; for others this is their first one.
CircuitNET connects the conferences you choose to matching conferences on other
boards. You continue to run your own users, local conferences and File Areas.
The guides take you from an assigned node through your first successful exchange.

Your local conference 12, Retro Computing, can map to RETRO. Another board maps
RETRO to its conference 4. CircuitNET uses RETRO, so local numbers need not match.
The official catalog lists available areas. You map the ones you carry, arrange
subscriptions with your HOST and choose when to Poll. The HOST forwards queued
messages to other subscribed boards, through ROOT when another branch is involved.
HOST and ROOT can also have callers and their own conferences and File Areas.
END means it does not serve downstream CircuitNET nodes.

## A normal message journey

A caller at one BBS posts in its local Retro Computing conference:

```
To: Craig Daters
Subject: Restoring a classic computer
```

That local area maps to RETRO. The message travels to subscribed BBSes, including
another board whose local RETRO mapping uses a different number. Craig can read
it there under that board's access rules. The To name does not choose a BBS or
make the message private. No destination Node ID is needed.

For deliberately directed conference traffic, the operator additionally selects
Destination Node USAZ017. That selects one BBS instead of normal conference
fanout. It still belongs to the conference and is not private or end-to-end
encrypted. [ADDRESSING](ADDRESSING.md) explains assignments and this distinction.
[DOSSIERS](DOSSIERS.md) shows both sides of an actual subscription setup.
