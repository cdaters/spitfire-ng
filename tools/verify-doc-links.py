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

"""Verify inline local Markdown path links; remote URLs/anchors are not fetched."""

from pathlib import Path
import re
import subprocess
from urllib.parse import unquote

root = Path(__file__).resolve().parent.parent
files = subprocess.check_output(["rg", "--files", "-g", "*.md"], cwd=root, text=True).splitlines()
checked = 0
missing = []
for name in files:
    path = root / name
    text = path.read_text(encoding="utf-8")
    text = re.sub(r"```.*?```", "", text, flags=re.S)
    for match in re.finditer(r"!?\[[^\]\n]*\]\((<[^>]+>|[^\s)]+)(?:[ \t]+[\"'][^\n]*?[\"'])?\)", text):
        link = match.group(1).strip("<>")
        if re.match(r"[A-Za-z][A-Za-z0-9+.-]*:", link) or link.startswith(("#", "//")):
            continue
        target = unquote(link.split("#", 1)[0].split("?", 1)[0])
        if not target:
            continue
        checked += 1
        if not (path.parent / target).exists():
            missing.append(f"{name}: {target}")
for item in missing:
    print(item)
print(f"Checked {len(files)} Markdown files, {checked} inline local path links, {len(missing)} missing targets. Remote URLs and fragment anchors excluded.")
raise SystemExit(bool(missing))
