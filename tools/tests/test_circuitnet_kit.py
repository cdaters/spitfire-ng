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

"""Release journey: delivered bytes, signatures, text, references and reproducibility."""
import importlib.util
import json
import posixpath
from pathlib import Path
import re
import copy
import shutil
from unittest.mock import patch
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
        self.assertTrue(validator.is_file())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            first = KIT.build(root / "one", validator)
            second = KIT.build(root / "two", validator)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            with zipfile.ZipFile(first) as archive:
                names = set(archive.namelist())
                self.assertTrue({"README.TXT", "FILE_ID.DIZ", "CONFS.TXT",
                                 "JOINING.TXT", "APPLICATION.TXT", "ENDNODE.TXT"} <= names)
                manifest = json.loads(archive.read("MANIFEST.json"))
                self.assertEqual(names, {item["name"] for item in manifest["files"]}
                                 | {"MANIFEST.json", "MANIFEST.sha256"})
                for item in manifest["files"]:
                    data = archive.read(item["name"])
                    self.assertEqual(len(data), item["size"])
                    self.assertEqual(KIT.digest(data), item["sha256"])
                    self.assertNotIn("..", Path(item["name"]).parts)
                    for marker in KIT.PRIVATE:
                        self.assertNotIn(marker, data)
                    if item["name"].endswith(".TXT") or item["name"] == "FILE_ID.DIZ":
                        text = data.decode("ascii")
                        self.assertNotIn(b"\n", data.replace(b"\r\n", b""))
                        self.assertTrue(all(len(line) <= 79 for line in text.splitlines()))
                        self.assertNotRegex(text, r"(?m)^\|.*\|$")
                        self.assertNotRegex(text, KIT.INTERNAL)
                    if item["name"].startswith("markdown/"):
                        self.assertNotRegex(data.decode(), KIT.INTERNAL)
                        self.assertNotIn(b"beginner-friendly", data)
                checksum_lines = archive.read("MANIFEST.sha256").decode().splitlines()
                checked = set()
                for line in checksum_lines:
                    digest, name = line.split("  ", 1)
                    self.assertEqual(digest, KIT.digest(archive.read(name)))
                    checked.add(name)
                self.assertEqual(checked, names - {"MANIFEST.sha256"})
                diz = archive.read("FILE_ID.DIZ").decode().splitlines()
                self.assertLessEqual(len(diz), 10)
                self.assertTrue(all(len(line) <= 45 for line in diz))
                self.assertIn(b"ABOUT.TXT", archive.read("README.TXT"))
                self.assertIn(b"JOINING.TXT", archive.read("README.TXT"))
                self.assertIn(b"KEYS.TXT", archive.read("SECURITY.TXT"))
                self.assertIn(b"Applications are not yet open", archive.read("JOININFO.TXT"))
                application = archive.read("APPLICATION.TXT").decode()
                self.assertNotRegex(application, r"(?im)^(?:password|private key|token|secret)\s*:")
                self.assertIn("DO NOT INCLUDE passwords", application)
                self.assertIn("static IP", application)
                profile = json.loads(archive.read("config/network-profile.example.json"))
                self.assertFalse(profile["directly_importable"])
                self.assertEqual(profile["joining_document"], "../markdown/JOINING.md")
                self.assertIn("markdown/JOINING.md", names)
                for field in ["addressing_document", "region_table"]:
                    target = (Path("config") / profile[field]).as_posix()
                    self.assertIn(posixpath.normpath(target), names)
                # Review from a fresh extraction, not repository-relative files.
                extracted = root / "outsider"
                archive.extractall(extracted)
                for name in names:
                    if name.endswith(".md"):
                        text = (extracted / name).read_text()
                        for target in re.findall(r"\[[^]]+\]\(([^)]+)\)", text):
                            if "://" in target or target.startswith("#"):
                                continue
                            self.assertTrue((extracted / name).parent.joinpath(target.split("#")[0]).is_file(),
                                            f"broken kit link: {name}: {target}")
                transport = archive.read("technical/circuitnet-transport.md").decode()
                self.assertIn("supported minor range 0 through 4", transport)
                self.assertIn("catalog-access", transport)
                self.assertIn("technical/circuitnet-ng-specification.md", names)
                self.assertIn("technical/circuitnet-implementation.md", names)
                specification = archive.read("technical/circuitnet-ng-specification.md").decode()
                self.assertIn("accepting BBS", specification)
                self.assertIn("reference implementation", specification)
                self.assertNotIn("CARGO_MANIFEST_DIR", specification)
                self.assertNotIn("supported minor range 0 through 3", transport)

    def test_generated_documents_follow_signed_data_without_exposing_ids(self):
        objects, _ = KIT.checked_catalog(ROOT / "target/debug/examples/catalog-artifact")
        docs = KIT.generated(objects)
        for name, text in docs.items():
            self.assertEqual((KIT.SOURCE / name).read_text(), text)
        restricted = []
        for entry in objects[-1]["body"]["entries"]:
            self.assertIn(entry["codename"], docs["CONFERENCES.md"])
            self.assertIn(entry["description"], docs["CONFERENCES.md"])
            self.assertNotIn(entry["id"], docs["CONFERENCES.md"])
            self.assertNotIn(entry["id"], docs["CONFERENCE-CHANGES.md"])
            if entry.get("access") == "sysops":
                restricted.append(entry["codename"])
                self.assertEqual(entry["required"], entry["codename"] == "SUPPORT")
        self.assertEqual(set(restricted), {"SUPPORT", "SYSOP", "SPITFIRE", "DOORS"})

    def test_operator_journeys_grouping_consent_and_verified_catalog_flow(self):
        objects, _ = KIT.checked_catalog(ROOT / "target/debug/examples/catalog-artifact")
        catalog = KIT.generated(objects)["CONFERENCES.md"]
        required, operators, general = [catalog.index(title) for title in [
            "## REQUIRED NETWORK CONFERENCES", "## SYSOP / NETWORK OPERATOR CONFERENCES",
            "## GENERAL CALLER CONFERENCES"]]
        self.assertLess(required, operators)
        self.assertLess(operators, general)
        required_codes = {e["codename"] for e in objects[-1]["body"]["entries"] if e["required"]}
        self.assertEqual(required_codes, {"SUPPORT", "CHITCHAT"})
        for code in required_codes:
            self.assertIn("### " + code + " -", catalog[required:operators])
        for code in ["SYSOP", "SPITFIRE", "DOORS"]:
            self.assertIn("### " + code + " -", catalog[operators:general])
        self.assertNotIn("### SUPPORT", catalog[general:])
        application = (KIT.SOURCE / "NODE-APPLICATION.md").read_text()
        self.assertNotIn("Fields you consent to publish", application)
        for phrase in ["unchecked means NO", "Country (name)", "REQUIRED PUBLIC MEMBERSHIP RECORD",
                       "[ ] Publish my Sysop", "[ ] Publish my general", "DO NOT INCLUDE passwords"]:
            self.assertIn(phrase, application)
        dossier = (KIT.SOURCE / "DOSSIERS.md").read_text()
        codes = {e["codename"] for e in objects[-1]["body"]["entries"]}
        for code in ["RETRO", "SUPPORT", "CHITCHAT"]:
            self.assertIn(code, codes)
            self.assertIn(code, dossier)
        self.assertIn("directional", dossier)
        end = (KIT.SOURCE / "END-NODE.md").read_text()
        for phrase in ["900", "15 minutes", "Hybrid", "Scheduled", "Manual", "pending_sync",
                       "catalog-status", "sfmonitor", "same connection", "EVENTS conference"]:
            self.assertIn(phrase, end)
        files = (KIT.SOURCE / "FILES.md").read_text()
        for phrase in ["Remote file-subscription requests are not implemented", "byte zero",
                       "file-map", "file-subscribe", "required approval", "approve FILE_ID"]:
            self.assertIn(phrase, files)
        access = (KIT.SOURCE / "SYSOP-ACCESS.md").read_text()
        self.assertIn("catalog-access-levels", access)
        self.assertIn("SUPPORT -", access)
        self.assertIn("ordinary caller", access)
        self.assertIn("Signature", (KIT.SOURCE / "VERIFICATION.md").read_text())

    def test_renderer_structure_encoding_width_and_visible_destinations(self):
        source = "# Title\n\nA ‘quoted’ sentence — with [guide](END-NODE.md).\n\n" + (
            "Useful text " * 20) + "\n\n- One item\n  continued here.\n\n" + (
            "| Field | Purpose |\n| --- | --- |\n| name | Contact |\n\n"
            "https://example.org/join\n\n```\n   ROOT\n    |\n   HOST\n```\n")
        result = KIT.plain(source)
        self.assertIn("guide (ENDNODE.TXT)", result)
        self.assertIn("https://example.org/join", result)
        self.assertIn("Field: name", result)
        self.assertIn("Purpose: Contact", result)
        self.assertIn("   ROOT\r\n    |\r\n   HOST", result)
        self.assertNotIn("‘", result)
        self.assertNotIn("| Field", result)
        self.assertTrue(all(len(line) <= 79 for line in result.splitlines()))
        result.encode("ascii")
        with self.assertRaises(ValueError):
            KIT.plain("```\n" + "x" * 80 + "\n```\n")
        with self.assertRaises(ValueError):
            KIT.plain("injected\x1b[31m")

    def test_public_identity_validation_and_closed_service_boundary(self):
        metadata = json.loads((KIT.SOURCE / "config/release.json").read_text())
        identity = KIT.PublicIdentity.parse(metadata)
        self.assertEqual(identity.domain, "circuitnetng.org")
        expected = {
            "website": "https://circuitnetng.org/",
            "application": "https://circuitnetng.org/apply",
            "kit": "https://circuitnetng.org/downloads/infopack.zip",
            "catalog_authority": "https://circuitnetng.org/network/catalog",
            "catalog_json": "https://circuitnetng.org/catalog/catalog.json",
            "catalog_signature": "https://circuitnetng.org/catalog/catalog.sig",
            "catalog_public_key": "https://circuitnetng.org/catalog/catalog-authority.pub"}
        self.assertEqual({key: identity.url(key) for key in expected}, expected)
        self.assertEqual(identity.email("joining"), "join@circuitnetng.org")
        self.assertEqual(identity.email("founder"), "founder@circuitnetng.org")
        self.assertFalse(metadata["applications_open"])
        self.assertEqual(metadata["service_status"], "not-verified")
        for category, name, value in [("paths", "application", "https://elsewhere.invalid/apply"),
                                       ("paths", "kit", "/../secret"),
                                       ("roles", "founder", "personal@example.org")]:
            wrong = copy.deepcopy(metadata)
            wrong["public_identity"][category][name] = value
            with self.assertRaises(ValueError):
                KIT.PublicIdentity.parse(wrong)
        wrong = copy.deepcopy(metadata)
        wrong["applications_open"] = True
        with self.assertRaises(ValueError):
            KIT.PublicIdentity.parse(wrong)

    def test_domain_fixture_regenerates_actual_kit_without_drift(self):
        validator = ROOT / "target/debug/examples/catalog-artifact"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "docs/circuitnet-ng"
            shutil.copytree(KIT.SOURCE, source)
            shutil.copytree(ROOT / "docs/technical", root / "docs/technical")
            (root / "tools").mkdir()
            shutil.copyfile(ROOT / "tools/circuitnet-addressing.py", root / "tools/circuitnet-addressing.py")
            for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
                shutil.copyfile(ROOT / name, root / name)
            metadata_path = source / "config/release.json"
            metadata = json.loads(metadata_path.read_text())
            metadata["public_identity"]["domain"] = "fixture.example.org"
            metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")
            with patch.object(KIT, "ROOT", root), patch.object(KIT, "SOURCE", source):
                with self.assertRaisesRegex(ValueError, "generated"):
                    KIT.build(root / "stale", validator, check=True)
                KIT.build(root / "unused", validator, update=True, check=True)
                archive_path = KIT.build(root / "changed", validator)
                with zipfile.ZipFile(archive_path) as archive:
                    identity = KIT.PublicIdentity.parse(metadata)
                    for name in ["README.TXT", "ABOUT.TXT", "JOINING.TXT", "APPLICATION.TXT",
                                 "ENDNODE.TXT", "HOSTNODE.TXT", "ROOTNODE.TXT", "SECURITY.TXT",
                                 "JOININFO.TXT", "NETWORK.TXT", "RELEASE.TXT"]:
                        data = archive.read(name)
                        self.assertNotIn(b"circuitnetng.org", data, name)
                        self.assertIn(b"fixture.example.org", data, name)
                        self.assertNotIn(b"<!--", data, name)
                    for key in identity.paths:
                        self.assertIn(identity.url(key).encode(), archive.read("NETWORK.TXT"))
                    for role in identity.roles:
                        self.assertIn(identity.email(role).encode(), archive.read("NETWORK.TXT"))
                    self.assertIn(b"Applications Closed", archive.read("APPLICATION.TXT"))
                    self.assertIn("has not been verified", " ".join(archive.read("README.TXT").decode().split()))
                    profile = json.loads(archive.read("config/network-profile.example.json"))
                    self.assertEqual(profile["public_identity"], identity.projection())
                    self.assertEqual(profile["public_identity_source"], "release.json")
                    authority = json.loads(archive.read("config/catalog-authority.json"))
                    fingerprint = KIT.digest(bytes.fromhex(authority["public_key"]))
                    self.assertIn(fingerprint.encode(), archive.read("SECURITY.TXT"))
                    self.assertIn(fingerprint.encode(), archive.read("RELEASE.TXT"))
                    catalog = json.loads(archive.read("config/catalog.json"))
                    self.assertEqual(archive.read("config/catalog.sig"),
                                     (catalog["signature"] + "\n").encode())
                    # Only the two configured role addresses appear in member editions.
                    emails = set()
                    for name in archive.namelist():
                        if name.startswith("markdown/") or name.endswith(".TXT"):
                            text = archive.read(name).decode()
                            emails.update(re.findall(r"[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}", text))
                            self.assertNotRegex(text, r"localhost|127\.0\.0\.1|<domain>|TBD|TODO")
                    self.assertEqual(emails, {identity.email("joining"), identity.email("founder")})

    def test_charter_continuity_and_joining_release_source(self):
        charter = (KIT.SOURCE / "CHARTER.md").read_text()
        for phrase in ["nine", "60 days", "seven", "Five members", "interim Secretary",
                       "Bootstrap authority ends", "founding powers do not return",
                       "explicitly waived", "first quorate committee meeting"]:
            self.assertIn(phrase, charter)
        metadata = json.loads((KIT.SOURCE / "config/release.json").read_text())
        self.assertEqual(KIT.joining(metadata), (KIT.SOURCE / "JOINING-INFO.md").read_text())
        metadata["applications_open"] = True
        with self.assertRaises(ValueError):
            KIT.joining(metadata)
        keys = (KIT.SOURCE / "KEY-CUSTODY.md").read_text()
        for phrase in ["catalog-artifact rotate", "catalog-artifact recover",
                       "confirm-key-replacement", "EACH", "exact", "offline"]:
            self.assertIn(phrase, keys)


if __name__ == "__main__":
    unittest.main()
