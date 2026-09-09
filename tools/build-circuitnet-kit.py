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

"""Build and validate the rights-safe Network Kit and its BBS text editions."""
import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import textwrap
import unicodedata
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "docs/circuitnet-ng"
NAME = "circuitnet-ng-network-kit-1.0"
EDITIONS = {
    "README.md": "README.TXT", "ABOUT.md": "ABOUT.TXT", "GOALS.md": "GOALS.TXT",
    "CHARTER.md": "CHARTER.TXT", "RULES.md": "RULES.TXT",
    "CONFERENCES.md": "CONFS.TXT", "CONFERENCE-CHANGES.md": "CHANGES.TXT",
    "FILES.md": "FILES.TXT", "JOINING.md": "JOINING.TXT",
    "JOINING-INFO.md": "JOININFO.TXT", "NODE-APPLICATION.md": "APPLICATION.TXT",
    "END-NODE.md": "ENDNODE.TXT", "HOST-NODE.md": "HOSTNODE.TXT",
    "ROOT-NODE.md": "ROOTNODE.TXT", "SECURITY.md": "SECURITY.TXT",
    "HISTORY.md": "HISTORY.TXT", "KEY-CUSTODY.md": "KEYS.TXT",
    "NODE-IDENTITY.md": "NODEKEY.TXT", "OPERATIONS.md": "OPERATE.TXT",
    "CATALOG-ADMIN.md": "CATALOG.TXT", "VERIFICATION.md": "VERIFY.TXT",
    "PROTOCOL.md": "PROTOCOL.TXT", "APPLICATION-FIELDS.md": "APPFIELDS.TXT",
    "NOTICE.md": "NOTICE.TXT", "PUBLIC-IDENTITY.md": "NETWORK.TXT",
    "ADDRESSING.md": "ADDRESSING.TXT", "DOSSIERS.md": "DOSSIERS.TXT",
    "SYSOP-ACCESS.md": "SYSOPACC.TXT",
}
HUMAN = list(EDITIONS)
CONFIG = ["catalog.json", "catalog-authority.json", "catalog-review.json",
          "network-profile.example.json", "catalog.schema.json", "release.json", "regions.json"]
TECHNICAL = ["circuitnet-catalog.md", "circuitnet-transport.md", "circuitnet.md",
             "files-custody.md", "events.md", "circuitnet-addressing.md"]
PRIVATE = [b"/Users/", b"/private/tmp/", b"BEGIN PRIVATE KEY",
           b"research/samples/", b".local-development/"]
INTERNAL = re.compile(r"\b(?:C[1-8](?:\.1)?|M0[0-9]{2}|Codex|Astra|CNTEST)\b|acceptance harness", re.I)


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
            raise ValueError("kit history must be consecutive from revision 1")
        if body["previous_hash"] != (previous["hash"] if previous else None):
            raise ValueError("kit catalog predecessor mismatch")
        previous = signed
    return objects, paths


def member_summary(value):
    # Presentation only: signed source text and approval references remain intact.
    value = value.replace("bootstrap-c7-charter-1.0-initial-catalog",
                          "Founding adoption under Charter 1.0")
    return re.sub(r"\bC[1-8](?:\.1)?\s+", "", value)


def generated(objects):
    body = objects[-1]["body"]
    lines = ["# CircuitNET NG conference catalog", "",
             f"Catalog revision {body['revision']}. Generated from the signed catalog.", "",
             "Catalog availability does not create a local conference or subscription.",
             "Local numbers and access rules remain each BBS's responsibility.", "",
             "SUPPORT is core and Sysop-only. SYSOP, SPITFIRE and DOORS are optional",
             "and Sysop-only. Verified visiting Sysops may receive locally granted access;",
             "ordinary callers receive no privileges from catalog membership.", ""]
    groups = [
        ("REQUIRED NETWORK CONFERENCES", lambda e: e["required"],
         "Required on every participating board. Preserve each area's access rules."),
        ("SYSOP / NETWORK OPERATOR CONFERENCES", lambda e: not e["required"] and e.get("access") == "sysops",
         "SUPPORT above is also Sysop-only. These additional operator areas are optional."),
        ("GENERAL CALLER CONFERENCES", lambda e: not e["required"] and e.get("access") != "sysops",
         "Optional areas for callers authorized by local conference rules.")]
    for title, predicate, introduction in groups:
        lines += [f"## {title}", "", introduction, ""]
        for entry in sorted(filter(predicate, body["entries"]),
                            key=lambda e: (not e["required"], e["codename"] != "SUPPORT",
                                           e["codename"], e["effective_revision"])):
            lines += [f"### {entry['codename']} - {entry['display_name']}", "",
                entry["description"], "",
                "Requirement: " + ("Required on every participating board." if entry["required"] else "Optional."),
                "Status: " + entry["status"] + ".",
                "Access: " + ("Sysops and verified visiting Sysops; granted locally." if entry.get("access") == "sysops"
                             else "Callers authorized by local conference rules."), ""]
    changes = ["# CircuitNET NG conference changes", "",
               "Generated from signed revision history.", ""]
    old = {}
    for signed in objects:
        b = signed["body"]
        changes += [f"## Revision {b['revision']}", "",
                    f"Decision: {member_summary(b['governance_reference'])}", "", member_summary(b["rationale"]), ""]
        for e in b["entries"]:
            before = old.get(e["id"])
            if before == e:
                continue
            action = "ADDED" if before is None else {"retired": "RETIRED", "deprecated": "DEPRECATED"}.get(e["status"], "UPDATED")
            if before and before["status"] == "retired" and e["status"] == "active":
                action = "REACTIVATED"
            if before is None and any(p["codename"] == e["codename"] for p in old.values()):
                action = "EXPLICITLY REUSED"
            changes.append(f"- {action}: {e['codename']} - {e['display_name']}")
        changes.append("")
        old = {e["id"]: e for e in b["entries"]}
    return {"CONFERENCES.md": "\n".join(lines),
            "CONFERENCE-CHANGES.md": "\n".join(changes).rstrip() + "\n"}


@dataclass(frozen=True)
class PublicIdentity:
    network_display_name: str
    domain: str
    roles: dict
    paths: dict

    @classmethod
    def parse(cls, metadata):
        raw = metadata["public_identity"]
        if set(raw) != {"network_display_name", "domain", "roles", "paths"}:
            raise ValueError("unexpected public identity fields")
        identity = cls(**raw)
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 .-]{0,79}", identity.network_display_name):
            raise ValueError("invalid network display name")
        if not re.fullmatch(r"(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}", identity.domain):
            raise ValueError("invalid official domain")
        if set(identity.roles) != {"joining", "founder"} or any(
                not isinstance(v, str) or not re.fullmatch(r"[a-z][a-z0-9-]{0,30}", v)
                for v in identity.roles.values()):
            raise ValueError("invalid public role addresses")
        if set(identity.paths) != {"website", "application", "kit", "catalog_authority",
                                  "catalog_json", "catalog_signature", "catalog_public_key"}:
            raise ValueError("missing public endpoint path")
        if any(not isinstance(v, str) or not re.fullmatch(r"/[a-z0-9/._-]*", v)
               or ".." in v or "//" in v for v in identity.paths.values()):
            raise ValueError("invalid public endpoint path")
        if type(metadata["applications_open"]) is not bool or metadata["service_status"] not in {
                "not-verified", "verified-open"}:
            raise ValueError("invalid application/deployment status")
        if metadata["applications_open"] and metadata["service_status"] != "verified-open":
            raise ValueError("opening applications requires explicit verified service status")
        return identity

    def url(self, name):
        return "https://" + self.domain + self.paths[name]

    def email(self, role):
        return self.roles[role] + "@" + self.domain

    def projection(self):
        return {"network_display_name": self.network_display_name, "official_domain": self.domain,
                **{name + "_url": self.url(name) for name in self.paths},
                **{role + "_email": self.email(role) for role in self.roles}}


def joining(metadata):
    identity = PublicIdentity.parse(metadata)
    lines = ["# Current joining information", "",
             "Applications are open." if metadata["applications_open"] else
             "Applications Closed. Applications are not yet open. Keep completed forms locally.", "",
             "The following are canonical public locations, not claims of deployed services.",
             "Web, mail and download availability has not been verified by this kit." if
             metadata["service_status"] == "not-verified" else
             "Release administration has explicitly recorded the application service as open.", ""]
    for label, value in [("Official network home", identity.url("website")),
                         ("Founding Network Administrator", identity.email("founder")),
                         ("Joining contact when opened", identity.email("joining")),
                         ("Application location when opened", identity.url("application")),
                         ("Official Network Kit location", identity.url("kit"))]:
        lines += [f"{label}: {value}", ""]
    lines += ["Do not submit an application until an opening notice is published and the",
              "submission destination is confirmed. Never send authentication secrets.", "",
              "Current joining information is maintained with the public kit source:",
              metadata["source_repository"], "", metadata["joining_information_path"], "",
              "See [PUBLIC-IDENTITY](PUBLIC-IDENTITY.md) for catalog publication locations.", ""]
    return "\n".join(lines)


PUBLIC_BLOCK = re.compile(r"<!-- public-identity:start -->.*?<!-- public-identity:end -->", re.S)


def identity_sections(metadata, authority):
    identity = PublicIdentity.parse(metadata)
    home = f"Official network home: {identity.url('website')}"
    closed = ("Applications Closed. Keep your form locally until applications open." if
              not metadata["applications_open"] else "Applications are open.")
    apply = (f"{closed}\n\nWhen opened, the application location is {identity.url('application')}\n"
             f"and the joining contact is {identity.email('joining')}.\n"
             "Do not send passwords, private keys or authentication secrets.")
    status = ("These are canonical locations; web, mail and download deployment has not\n"
              "been verified. See [JOINING-INFO](JOINING-INFO.md) for opening status.")
    admin = f"Founding Network Administrator role contact: {identity.email('founder')}."
    catalog = (f"Catalog authority and verification information: {identity.url('catalog_authority')}\n\n"
               f"Catalog JSON: {identity.url('catalog_json')}\n\n"
               f"Catalog signature: {identity.url('catalog_signature')}\n\n"
               f"Catalog public key: {identity.url('catalog_public_key')}\n\n"
               "Current accepted signing-key fingerprint (SHA-256 of raw public key):\n"
               f"{digest(bytes.fromhex(authority['public_key']))}\n\n"
               "HTTPS provides delivery and discovery, not independent signing authority.\n"
               "Confirm the fingerprint through approved enrollment or an already trusted\n"
               "authority. Verify the signed catalog and retained revision chain.\n"
               "These publication locations are recorded; endpoint service is not verified.")
    return {"README.md": home + "\n\nOfficial kit location: " + identity.url('kit') + "\n\n" + status,
            "ABOUT.md": home + "\n\n" + status,
            "JOINING.md": apply + "\n\n" + admin + "\n\n" + status,
            "NODE-APPLICATION.md": apply,
            "APPLICATION-FIELDS.md": apply,
            "END-NODE.md": home + "\n\n" + apply + "\n\n" + status,
            "HOST-NODE.md": admin + "\n\n" + home + "\n\n" + status,
            "ROOT-NODE.md": admin + "\n\n" + home + "\n\n" + status,
            "SECURITY.md": catalog,
            "VERIFICATION.md": catalog}


def public_identity_document(metadata, authority):
    identity = PublicIdentity.parse(metadata)
    lines = ["# Official network identity and publication locations", "",
             f"{identity.network_display_name} uses {identity.domain} as its official home.", "",
             ("Applications Closed." if not metadata["applications_open"] else "Applications are open."),
             ("Web, email and download services have not been verified." if
              metadata["service_status"] == "not-verified" else
              "Application service opening is recorded by release administration."),
             "These are the intended publication locations; no deployment is claimed.", ""]
    purposes = {"website": "Network introduction", "application": "Node application when opened",
                "kit": "Current documentation/configuration Network Kit",
                "catalog_authority": "Human catalog authority and verification page",
                "catalog_json": "Signed machine catalog", "catalog_signature": "Signature sidecar",
                "catalog_public_key": "Catalog signing public key"}
    for name, purpose in purposes.items():
        lines += [f"## {purpose}", "", identity.url(name), ""]
    lines += ["## Public role contacts", "", f"Joining: {identity.email('joining')}", "",
              f"Founding Network Administrator: {identity.email('founder')}", "",
              "Do not submit applications until opening is announced. Credential enrollment",
              "occurs after membership approval; no application includes authentication secrets.", "",
              "## Catalog publication contract", "",
              "The human authority page should show the current revision, catalog body hash,",
              "signing-key fingerprint, publisher, protocol version, publication date and",
              "previous revision, with artifact links and verification instructions.", "",
              "catalog.json is the unchanged signed wrapper: body, hash and signature.",
              "Its catalog SHA-256 covers the canonical body, not the entire JSON file.",
              "catalog.sig contains the same 64-byte signature as 128 lowercase hexadecimal",
              "characters followed by LF. It is a convenience copy, not a separate signature",
              "of the JSON wrapper. Reject disagreement with the wrapper signature.",
              "catalog-authority.pub contains the 32-byte public key as 64 lowercase",
              "hexadecimal characters followed by LF. It contains no private signing material.", "",
              "HTTPS discovery does not establish trust in a replacement signing key.",
              "Use an independently accepted authority and verify the catalog signature and",
              "revision chain as described in [VERIFICATION](VERIFICATION.md).",
              "An online page must generate its current values from the actual published",
              "catalog and authority, not copy the values from this older kit indefinitely.", "",
              "Current kit authority fingerprint (SHA-256 of raw public key):",
              digest(bytes.fromhex(authority['public_key'])), "",
              "## Future site sections", "",
              "Conference information, public node listings, Files, standards and downloads",
              "may receive separate pages. Their publication requires separate work.",
              "The planned public Node Directory is " + identity.url("website") + "nodes.",
              "It will show Node ID, BBS name and role plus only explicitly consented",
              "optional fields, with BBS name/Node ID search and consented location search.",
              "The directory is not deployed; see the application publication choices.",
              "A future independent CircuitNet Technical Standards identity may be considered;",
              "it is not an active organization, site or dependency. This network home remains",
              "the canonical location for both network and technical documentation.", ""]
    return "\n".join(lines)


def ascii_text(value):
    value = value.translate(str.maketrans({"—": " - ", "–": "-", "’": "'", "‘": "'",
        "“": '\"', "”": '\"', "→": "->", "↔": "<->", "…": "...", "≤": "<=",
        "≠": "!=", "×": "x", "•": "*", " ": " "}))
    value = unicodedata.normalize("NFKD", value)
    value = "".join(c for c in value if not unicodedata.combining(c))
    value.encode("ascii")  # Unknown characters must be deliberately handled.
    if any(ord(c) < 32 and c not in "\n\r\t" for c in value):
        raise ValueError("control character in BBS edition")
    return value


def plain(markdown):
    """Structural BBS renderer: paragraphs, lists, tables, links and literal blocks."""
    def link(match):
        label, target = match.groups()
        base, _, anchor = target.partition("#")
        local = EDITIONS.get(Path(base).name)
        destination = local if local else target
        return label if label == destination else f"{label} ({destination})"
    markdown = re.sub(r"<!-- public-identity:(?:start|end) -->\n?", "", markdown)
    text = ascii_text(re.sub(r"\[([^]]+)\]\(([^)]+)\)", link, markdown))
    text = text.replace("**", "").replace("`", "") if "```" not in text else text
    result, paragraph, code = [], [], False
    def wrapped(value, initial="", subsequent=""):
        lines = textwrap.wrap(value, 79, initial_indent=initial, subsequent_indent=subsequent,
                              break_long_words=False, break_on_hyphens=False)
        if any(len(line) > 79 for line in lines):
            raise ValueError(f"unbreakable text exceeds 79 columns: {value}")
        # Avoid a short orphan word in prose while preserving literal blocks.
        if len(lines) > 1 and len(lines[-1].strip().split()) == 1:
            prefix, _, word = lines[-2].rpartition(" ")
            if prefix.strip() and len(subsequent + word + " " + lines[-1].strip()) <= 79:
                lines[-2] = prefix
                lines[-1] = subsequent + word + " " + lines[-1].strip()
        return lines
    def flush():
        if paragraph:
            result.extend(wrapped(" ".join(paragraph).replace("**", "").replace("`", "")))
            paragraph.clear()
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i].rstrip()
        if line.startswith("```"):
            flush(); code = not code; i += 1; continue
        if code:
            if len(line) > 79:
                raise ValueError(f"literal line exceeds 79 columns: {line}")
            result.append(line); i += 1; continue
        if line.startswith("|"):
            flush()
            headers = [x.strip() for x in line.strip("|").split("|")]
            i += 1
            if i < len(lines) and re.match(r"^[| :\-]+$", lines[i]):
                i += 1
            while i < len(lines) and lines[i].startswith("|"):
                values = [x.strip() for x in lines[i].strip("|").split("|")]
                if len(values) != len(headers):
                    raise ValueError("malformed Markdown table")
                for h, v in zip(headers, values):
                    result.extend(wrapped(f"{h}: {v}"))
                result.append(""); i += 1
            continue
        if not line:
            flush(); result.append(""); i += 1; continue
        if line.startswith("#"):
            flush(); title = re.sub(r"^#{1,6}\s+", "", line)
            result.extend(wrapped(title)); result.append("-" * min(79, len(title)))
        elif re.match(r"^(?:[-*]|[0-9]+\.)\s", line):
            flush()
            match = re.match(r"^([-*]|[0-9]+\.)\s+(.*)", line)
            marker, value = match.groups()
            while i + 1 < len(lines) and lines[i + 1].startswith("  "):
                i += 1; value += " " + lines[i].strip()
            result.extend(wrapped(value.replace("`", "").replace("**", ""), marker + " ", " " * (len(marker) + 1)))
        elif line.startswith(("Status:", "Access:", "Requirement:")):
            flush(); result.extend(wrapped(line))
        else:
            paragraph.append(line.strip())
        i += 1
    flush()
    if code:
        raise ValueError("unclosed literal block")
    text = re.sub(r"\n{3,}", "\n\n", "\n".join(result)).strip() + "\n"
    return text.replace("\n", "\r\n")


def build(output, validator, check=False, update=False, source_commit=None):
    if source_commit and not re.fullmatch(r"[a-f0-9]{40}", source_commit):
        raise ValueError("source commit must be a full public commit")
    objects, history_paths = checked_catalog(validator)
    metadata = json.loads((SOURCE / "config/release.json").read_text())
    docs = generated(objects)
    identity = PublicIdentity.parse(metadata)
    authority = json.loads((SOURCE / "config/catalog-authority.json").read_text())
    docs["JOINING-INFO.md"] = joining(metadata)
    docs["PUBLIC-IDENTITY.md"] = public_identity_document(metadata, authority)
    for name, section in identity_sections(metadata, authority).items():
        original = (SOURCE / name).read_text()
        block = "<!-- public-identity:start -->\n" + section + "\n<!-- public-identity:end -->"
        if PUBLIC_BLOCK.search(original):
            docs[name] = PUBLIC_BLOCK.sub(lambda _: block, original)
        else:
            heading, rest = original.split("\n", 1)
            docs[name] = heading + "\n\n" + block + "\n" + rest
    example_path = SOURCE / "config/network-profile.example.json"
    example = json.loads(example_path.read_text())
    example["display_name"] = identity.network_display_name
    example["public_identity"] = identity.projection()
    example["public_identity_source"] = "release.json"
    example_text = json.dumps(example, indent=2) + "\n"
    if update:
        example_path.write_text(example_text)
    elif example_path.read_text() != example_text:
        raise ValueError("generated example identity differs; use --update-docs")
    for name, text in docs.items():
        path = SOURCE / name
        if update:
            path.write_text(text)
        elif not path.exists() or path.read_text() != text:
            raise ValueError(f"generated document differs: {name}; use --update-docs")
    application = plain((SOURCE / "NODE-APPLICATION.md").read_text())
    if update:
        (SOURCE / "NODE-APPLICATION.txt").write_bytes(application.replace("\r\n", "\n").encode("ascii"))
    elif (SOURCE / "NODE-APPLICATION.txt").read_bytes() != application.replace("\r\n", "\n").encode("ascii"):
        raise ValueError("generated application differs; use --update-docs")
    if check:
        return None
    sources = {"markdown/" + name: SOURCE / name for name in HUMAN}
    sources.update({"config/" + name: SOURCE / "config" / name for name in CONFIG})
    sources.update({"technical/" + name: ROOT / "docs/technical" / name for name in TECHNICAL})
    sources["technical/circuitnet-addressing.py"] = ROOT / "tools/circuitnet-addressing.py"
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
                    if name.startswith("markdown/"):
                        raise ValueError(f"member link outside kit: {name}: {target}")
                    # Technical references outside this docs-only kit retain a public
                    # repository URL instead of silently dropping the destination.
                    repository_path = (path.parent / relative).resolve().relative_to(ROOT)
                    if repository_path.parts[:2] == ("docs", "research"):
                        return f"[{label}: public project documentation]({metadata['source_repository']}/tree/main/docs)"
                    return f"[{label}]({metadata['source_repository']}/blob/main/{repository_path}{'#' + anchor if anchor else ''})"
                link = os.path.relpath(destination, str(Path(name).parent))
                return f"[{label}]({link}{'#' + anchor if anchor else ''})"
            data = re.sub(r"\[([^]]+)\]\(([^)]+)\)", rewrite, data.decode()).encode()
            data = re.sub(rb"<!-- public-identity:(?:start|end) -->\n?", b"", data)
        if any(marker in data for marker in PRIVATE):
            raise ValueError(f"private marker in {name}")
        contents[name] = data
        if name.startswith("markdown/"):
            if INTERNAL.search(data.decode()):
                raise ValueError(f"development terminology in member document {name}")
            contents[EDITIONS[Path(name).name]] = plain(data.decode()).encode("ascii")
    for name in ["CHARTER.md", "RULES.md"]:
        if b"Version 1.0" not in contents["markdown/" + name]:
            raise ValueError("unexpected Charter/Rules version")
    authority = json.loads(contents["config/catalog-authority.json"])
    fingerprint = digest(bytes.fromhex(authority["public_key"]))
    contents["config/catalog-authority.pub"] = (authority["public_key"] + "\n").encode()
    contents["config/catalog.sig"] = (objects[-1]["signature"] + "\n").encode("ascii")
    contents["FILE_ID.DIZ"] = ("CircuitNET NG Network Kit 1.0\r\n"
        "Charter, rules, conference list,\r\n"
        "joining information and setup guides\r\n"
        "for the CircuitNET NG BBS network.\r\n"
        "Documentation and configuration kit.\r\n").encode("ascii")
    source_digest = digest(b"".join(name.encode() + b"\0" + bytes.fromhex(digest(data))
                                  for name, data in sorted(contents.items())))
    release = (f"CircuitNET NG Network Kit 1.0 - revised build {metadata['build_revision']}\n\n"
        f"Official home: {identity.url('website')}\n"
        f"Official kit location: {identity.url('kit')}\n"
        "Canonical locations only; deployment is not verified by this kit.\n"
        f"Source: {metadata['source_repository']}\n"
        f"Source commit: {source_commit or 'Uncommitted candidate; identify by source digest.'}\n"
        f"Source-content SHA-256:\n{source_digest}\n"
        f"Catalog authority fingerprint (SHA-256):\n{fingerprint}\n\n"
        "Fixed ZIP timestamps support reproducible builds; no wall-clock timestamp is embedded.\n"
        "This candidate supersedes the earlier pre-release kit of the same version.\n"
        "Only the revised artifact should be offered as the current Network Kit 1.0.\n"
        "See VERIFY.TXT, MANIFEST.json and MANIFEST.sha256.\n")
    contents["RELEASE.TXT"] = plain(release).encode("ascii")
    manifest = {"format": "circuitnet-ng-kit-manifest", "version": "1.0",
                "scope": "documentation-and-configuration-only",
                "excluded": ["MANIFEST.json", "MANIFEST.sha256"], "files": [
                    {"name": name, "size": len(data), "sha256": digest(data)}
                    for name, data in sorted(contents.items())]}
    contents["MANIFEST.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    contents["MANIFEST.sha256"] = "".join(f"{digest(data)}  {name}\n"
        for name, data in sorted(contents.items())).encode()
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
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            package.writestr(info, data)
    with zipfile.ZipFile(archive) as package:
        if set(package.namelist()) != set(contents):
            raise ValueError("archive inventory differs")
        for name, data in contents.items():
            if package.read(name) != data:
                raise ValueError("archive payload differs")
    checksum = digest(archive.read_bytes())
    archive.with_suffix(".zip.sha256").write_text(f"{checksum}  {archive.name}\n")
    print(f"{NAME}: {len(contents)} files; archive SHA-256 {checksum}")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--validator", type=Path, default=ROOT / "target/debug/examples/catalog-artifact")
    parser.add_argument("--source-commit", help="public source commit, never a private checkpoint")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--update-docs", action="store_true")
    args = parser.parse_args()
    build(args.output, args.validator.resolve(), args.check, args.update_docs, args.source_commit)


if __name__ == "__main__":
    main()
