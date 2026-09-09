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

"""Assignment policy remains distinct from runtime routing and enrollment."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('addressing', ROOT / 'tools/circuitnet-addressing.py')
ADDRESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADDRESS)
TABLE = ROOT / 'docs/circuitnet-ng/config/regions.json'


class AddressingTests(unittest.TestCase):
    def setUp(self):
        self.table = json.loads(TABLE.read_text())
        self.book = ADDRESS.AddressBook(self.table)

    def test_wire_compatibility_and_reserved_root(self):
        for value in ['END00001', 'HOST0001', 'ROOT0001', 'CNETROOT', 'usaz017', 'A']:
            self.assertEqual(ADDRESS.node_id(value), value.upper())
        for value in ['', 'TOOLOOOONG', 'US/AZ001', '1:2/3', 'USAZ00é']:
            with self.assertRaises(ValueError):
                ADDRESS.node_id(value)
        self.assertEqual(self.book.validate('CNETROOT', 'ROOT'), 'CNETROOT')
        for role in ['END', 'HOST']:
            with self.assertRaises(ValueError):
                self.book.validate('CNETROOT', role, legacy=True)
        self.assertEqual(self.book.validate('ROOT', 'ROOT', legacy=True), 'ROOT')
        with self.assertRaises(ValueError):
            self.book.validate('ROOT', 'ROOT')

    def test_ranges_and_role_mismatches(self):
        for value, role in [('USAZ000','HOST'), ('USAZ001','END'), ('USAZ899','END'),
                            ('USAZ900','HOST'), ('USAZ999','HOST'), ('CAON001','END'),
                            ('AUNS012','END'), ('GBEN021','END')]:
            self.assertEqual(self.book.validate(value, role), value)
            for wrong in {'ROOT','END','HOST'} - {role}:
                with self.assertRaises(ValueError):
                    self.book.validate(value, wrong)

    def test_named_region_authority_and_extensions(self):
        self.assertEqual(self.book.region('United States', 'Arizona'), 'USAZ')
        self.assertEqual(self.book.region('Australia', 'New South Wales'), 'AUNS')
        for country, region in [('XX','AZ'), ('US','ZZ'), ('DE','BE')]:
            with self.assertRaises(ValueError):
                self.book.region(country, region)
        self.table['regions'].append(copy.deepcopy(self.table['regions'][0]))
        with self.assertRaises(ValueError):
            ADDRESS.AddressBook(self.table)

    def test_unique_reservations_retirement_and_deterministic_suggestions(self):
        rows = [{'node':'USAZ001','role':'END','status':'retired','legacy':False}]
        self.assertEqual(self.book.suggest('US','AZ','END',rows), 'USAZ002')
        self.assertEqual(self.book.suggest('US','AZ','PRIMARY-HOST',rows), 'USAZ000')
        self.assertEqual(self.book.suggest('US','AZ','ADDITIONAL-HOST',rows), 'USAZ900')
        self.assertEqual(self.book.suggest('','','ROOT',rows), 'CNETROOT')
        with self.assertRaisesRegex(ValueError, 'active ROOT'):
            self.book.suggest('', '', 'ROOT',
                [dict(node='ROOT', role='ROOT', status='active', legacy=True)])
        duplicate = dict(rows[0], node='usaz001', status='active')
        with self.assertRaises(ValueError):
            self.book.reservations(rows + [duplicate])
        rows = [dict(node=f'USAZ{n:03}',role='END',status='reserved',legacy=False) for n in range(1,900)]
        with self.assertRaisesRegex(ValueError, 'exhausted'):
            self.book.suggest('US','AZ','END',rows)

    def test_cli_does_not_assign_and_rejects_wrong_requested_region_kind(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'reservations.json'
            path.write_text('[]\n')
            cmd = ['python3',str(ROOT/'tools/circuitnet-addressing.py'),'--regions',str(TABLE),
                   '--reservations',str(path),'--country','United States','--region','Arizona',
                   '--kind','END']
            result = subprocess.run(cmd,check=True,capture_output=True,text=True)
            self.assertEqual(json.loads(result.stdout)['suggested_node_id'],'USAZ001')
            self.assertFalse(json.loads(result.stdout)['assigned'])
            self.assertEqual(path.read_text(),'[]\n')
            for request in ['USAZ000','CAON001','USAZ900','CNETROOT']:
                self.assertNotEqual(subprocess.run(cmd+['--requested',request],capture_output=True).returncode,0)


if __name__ == '__main__':
    unittest.main()
