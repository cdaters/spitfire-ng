# About CircuitNET NG

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
