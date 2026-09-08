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

"""Focused kit reproducibility and manifest verification with the native validator."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("kit", ROOT / "tools/build-circuitnet-kit.py")
KIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(KIT)


class CircuitnetKitTests(unittest.TestCase):
    def test_reproducible_allowlisted_archive_and_manifest(self):
        validator = ROOT / "target/debug/examples/catalog-artifact"
        self.assertTrue(validator.is_file(), "build the documented native validator first")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = KIT.build(root / "one", validator)
            second = KIT.build(root / "two", validator)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            with zipfile.ZipFile(first) as archive:
                manifest = json.loads(archive.read(KIT.NAME + "/MANIFEST.json"))
                self.assertEqual(len(archive.namelist()), len(manifest["files"]) + 1)
                for item in manifest["files"]:
                    data = archive.read(KIT.NAME + "/" + item["name"])
                    self.assertEqual(len(data), item["size"])
                    self.assertEqual(KIT.digest(data), item["sha256"])
                    self.assertNotIn("..", Path(item["name"]).parts)
                    self.assertNotIn(b"/Users/", data)
                    self.assertNotIn(b"BEGIN PRIVATE KEY", data)
                    if item["name"].startswith("text/"):
                        self.assertTrue(all(len(line) <= 80 for line in data.decode().splitlines()))

    def test_generated_conference_and_change_documents_follow_signed_data(self):
        objects, _ = KIT.checked_catalog(ROOT / "target/debug/examples/catalog-artifact")
        docs = KIT.generated(objects)
        for name, text in docs.items():
            self.assertEqual((KIT.SOURCE / name).read_text(), text)
        for entry in objects[-1]["body"]["entries"]:
            self.assertIn(entry["id"], docs["CONFERENCES.md"])
            self.assertIn(entry["id"], docs["CONFERENCE-CHANGES.md"])


if __name__ == "__main__":
    unittest.main()
