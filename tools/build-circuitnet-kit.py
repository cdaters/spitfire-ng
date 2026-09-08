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

"""Build the rights-safe, reproducible documentation/configuration infopack."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import textwrap
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "docs/circuitnet-ng"
NAME = "circuitnet-ng-network-kit-1.0"
HUMAN = ["README.md", "CHARTER.md", "RULES.md", "CONFERENCES.md",
         "CONFERENCE-CHANGES.md", "JOINING.md", "END-NODE.md", "HOST-NODE.md",
         "ROOT-NODE.md", "SECURITY.md", "PROTOCOL.md", "CATALOG-ADMIN.md",
         "APPLICATION-FIELDS.md", "NOTICE.md", "NODE-APPLICATION.txt"]
CONFIG = ["catalog.json", "catalog-authority.json", "catalog-review.json",
          "network-profile.example.json", "catalog.schema.json"]
TECHNICAL = ["circuitnet-catalog.md", "circuitnet-transport.md", "circuitnet.md"]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked_catalog(validator):
    paths = sorted((SOURCE / "config/catalog-history").glob("*.json"))
    paths.append(SOURCE / "config/catalog.json")
    objects = [json.loads(p.read_text()) for p in paths]
    previous = None
    for path, signed in zip(paths, objects):
        subprocess.run([str(validator), "validate", str(path),
                        str(SOURCE / "config/catalog-authority.json")],
                       check=True, stdout=subprocess.PIPE)
        body = signed["body"]
        if body["revision"] != (previous["body"]["revision"] + 1 if previous else 1):
            raise ValueError("kit catalog history must be consecutive from revision 1")
        if body["previous_hash"] != (previous["hash"] if previous else None):
            raise ValueError("kit catalog predecessor mismatch")
        previous = signed
    return objects, paths


def cell(value):
    return str(value).replace("|", "\\|").replace("<", "&lt;").replace(">", "&gt;")


def generated(objects):
    current = objects[-1]
    body = current["body"]
    lines = ["# CircuitNET NG Conference Catalog", "",
             f"Generated from signed catalog revision {body['revision']}. Do not hand edit.", "",
             "Network identity is independent of local conference numbers. Catalog presence",
             "does not create mappings or subscriptions. Retired identities remain historical.", "",
             "| Codename | Name / purpose | Status | Core | Immutable identity |",
             "| --- | --- | --- | --- | --- |"]
    for e in sorted(body["entries"], key=lambda e: (e["codename"], e["id"])):
        lines.append(f"| {e['codename']} | {cell(e['display_name'])}: {cell(e['description'])} | "
                     f"{e['status']} | {'Required' if e['required'] else 'Optional'} | `{e['id']}` |")
    changes = ["# CircuitNET NG Conference Changes", "",
               "Generated from actual signed revision history. Do not hand edit.", ""]
    old = {}
    for signed in objects:
        b = signed["body"]
        changes += [f"## Revision {b['revision']}", "", f"Governance: {cell(b['governance_reference'])}",
                    "", cell(b["rationale"]), ""]
        for e in b["entries"]:
            before = old.get(e["id"])
            if before == e:
                continue
            action = "ADDED" if before is None else {"retired": "RETIRED", "deprecated": "DEPRECATED"}.get(e["status"], "UPDATED")
            if before and before["status"] == "retired" and e["status"] == "active":
                action = "REACTIVATED"
            changes.append(f"- {action}: {e['codename']} — {cell(e['display_name'])} (identity {e['id']})")
        changes.append("")
        old = {e["id"]: e for e in b["entries"]}
    return {"CONFERENCES.md": "\n".join(lines) + "\n",
            "CONFERENCE-CHANGES.md": "\n".join(changes).rstrip() + "\n"}


def plain(markdown):
    text = re.sub(r"\[([^]]+)\]\([^)]+\)", r"\1", markdown)
    text = re.sub(r"(?m)^#{1,6}\s+", "", text).replace("**", "").replace("`", "")
    lines = []
    for line in text.splitlines():
        lines.extend(textwrap.wrap(line, 80, replace_whitespace=False) if line else [""])
    return "\n".join(lines) + "\n"


def build(output, validator, check=False, update=False):
    objects, history_paths = checked_catalog(validator)
    docs = generated(objects)
    for name, text in docs.items():
        path = SOURCE / name
        if update:
            path.write_text(text)
        elif not path.exists() or path.read_text() != text:
            raise ValueError(f"generated document differs: {name}; use --update-docs")
    if check:
        return None
    sources = {name: SOURCE / name for name in HUMAN}
    sources.update({"config/" + name: SOURCE / "config" / name for name in CONFIG})
    sources.update({"technical/" + name: ROOT / "docs/technical" / name for name in TECHNICAL})
    sources.update({name: ROOT / name for name in ["LICENSE-MIT", "LICENSE-APACHE"]})
    sources.update({"config/catalog-history/" + p.name: p for p in history_paths[:-1]})
    reverse = {path.resolve(): name for name, path in sources.items()}
    contents = {}
    for name, path in sorted(sources.items()):
        if path.is_symlink() or not path.is_file():
            raise ValueError("kit sources must be regular allowlisted files")
        data = path.read_bytes()
        if name.endswith(".md"):
            def rewrite(match):
                label, target = match.groups()
                if "://" in target or target.startswith("#"):
                    return match.group(0)
                relative, _, anchor = target.partition("#")
                destination = reverse.get((path.parent / relative).resolve())
                if destination is None:
                    # Keep the explanation, without private research paths or broken
                    # repository-only links in a standalone offline kit.
                    return label
                link = os.path.relpath(destination, str(Path(name).parent))
                return f"[{label}]({link}{'#' + anchor if anchor else ''})"
            data = re.sub(r"\[([^]]+)\]\(([^)]+)\)", rewrite, data.decode()).encode()
        if any(marker in data for marker in [b"/Users/", b"/private/tmp/", b"BEGIN PRIVATE KEY",
                                             b"research/samples/", b".local-development/"]):
            raise ValueError(f"private source marker in allowlisted file {name}")
        contents[name] = data
        if name in HUMAN and name.endswith(".md"):
            contents["text/" + name.removesuffix(".md") + ".txt"] = plain(data.decode()).encode()
    for name in ["CHARTER.md", "RULES.md"]:
        if b"Version 1.0" not in contents[name]:
            raise ValueError("unexpected Charter/Rules version")
    manifest = {"format": "circuitnet-ng-kit-manifest", "version": "1.0",
                "scope": "documentation-and-configuration-only",
                "self_excluded": "MANIFEST.json", "files": [
                    {"name": name, "size": len(data), "sha256": digest(data)}
                    for name, data in sorted(contents.items())]}
    contents["MANIFEST.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    output.mkdir(parents=True, exist_ok=True)
    expanded = output / NAME
    if expanded.exists():
        raise ValueError("expanded kit destination already exists")
    expanded.mkdir()
    for name, data in sorted(contents.items()):
        destination = expanded / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    archive = output / (NAME + ".zip")
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_STORED) as package:
        for name, data in sorted(contents.items()):
            info = zipfile.ZipInfo(NAME + "/" + name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            package.writestr(info, data)
    # Re-open and verify every inventory entry against the delivered archive.
    with zipfile.ZipFile(archive) as package:
        for item in manifest["files"]:
            data = package.read(NAME + "/" + item["name"])
            assert len(data) == item["size"] and digest(data) == item["sha256"]
    print(f"{NAME}: {len(contents)} files; archive SHA-256 {digest(archive.read_bytes())}")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--validator", type=Path, default=ROOT / "target/debug/examples/catalog-artifact")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--update-docs", action="store_true")
    args = parser.parse_args()
    build(args.output, args.validator.resolve(), args.check, args.update_docs)


if __name__ == "__main__":
    main()
