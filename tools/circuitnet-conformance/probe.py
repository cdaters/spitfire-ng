#!/usr/bin/env python3
# SPITFIRE NG
# Preservation-driven modern cross-platform reimplementation of
# Buffalo Creek Software's SPITFIRE Bulletin Board System
#
# Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
# Licensed under MIT OR Apache-2.0
#
# This file is part of the SPITFIRE NG project.
# See the repository documentation for architecture, provenance,
# compatibility research, security, and contribution guidelines.


"""Explicit malformed-peer probe, separate from conforming peer send behavior."""
import argparse
from pathlib import Path
from peer import Peer, Store, run_connection
from protocol import CAPS, Failure, canonical, loads, read_frame, write_frame

class Probe(Peer):
    def send_work(self, io):
        # Deliberately violate capability gating; never used by the reference peer.
        write_frame(io, dict(type='offer',batch=self.work['batch']))
        read_frame(io,4096)
        raise Failure('unexpected-acceptance')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config',required=True,type=Path)
    parser.add_argument('--state',required=True,type=Path)
    parser.add_argument('--work',required=True,type=Path)
    parser.add_argument('--port',required=True,type=int)
    parser.add_argument('--report',required=True,type=Path)
    parser.add_argument('--catalog-without-access',action='store_true')
    args=parser.parse_args()
    config=loads(args.config.read_bytes(),65536)
    config.update(capabilities=CAPS[:2]+(['catalog-sync'] if args.catalog_without_access else []),
                  maximum_minor=4 if args.catalog_without_access else 1)
    peer=Probe(config,Store(args.state),loads(args.work.read_bytes()))
    try:
        run_connection(peer,args.port)
        result='unexpected-acceptance'
    except Failure as e:
        result=e.code
    args.report.write_bytes(canonical(dict(result=result))+b'\n')
    return 0 if result=='unsupported-version' else 1

if __name__=='__main__':
    raise SystemExit(main())
