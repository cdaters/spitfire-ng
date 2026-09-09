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

"""Read-only assignment planning; no network enrollment or reservation mutation."""
import argparse
import json
from pathlib import Path
import re


def node_id(value):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9]{1,8}", value):
        raise ValueError("Node ID must contain 1-8 ASCII letters/digits")
    return value.upper()


class AddressBook:
    def __init__(self, table):
        if (set(table) != {"format", "version", "country_authority", "subdivision_authority", "scope", "regions"}
                or table["format"] != "circuitnet-ng-regions" or type(table["version"]) is not int
                or table["version"] < 1 or not 1 <= len(table["regions"]) <= 4096):
            raise ValueError("invalid versioned region table")
        self.regions = {}
        self.countries = {}
        names, subdivisions = set(), set()
        for row in table["regions"]:
            if set(row) != {"country", "country_name", "region", "region_name", "subdivision"}:
                raise ValueError("invalid region fields")
            cc, rr = row["country"], row["region"]
            if (not re.fullmatch(r"[A-Z]{2}", cc) or not re.fullmatch(r"[A-Z0-9]{2}", rr)
                    or not re.fullmatch(cc + r"-[A-Z0-9]{1,3}", row["subdivision"])):
                raise ValueError("invalid country/subdivision mapping")
            for key in ("country_name", "region_name"):
                if not isinstance(row[key], str) or not 1 <= len(row[key]) <= 120 or any(ord(c) < 32 for c in row[key]):
                    raise ValueError("invalid geographic name")
            country_name = row["country_name"].casefold()
            pair = (cc, row["region_name"].casefold())
            if (cc + rr in self.regions or pair in names or row["subdivision"] in subdivisions
                    or self.countries.get(country_name, cc) != cc
                    or any(k != country_name and v == cc for k, v in self.countries.items())):
                raise ValueError("conflicting geographic mapping")
            self.regions[cc + rr] = row
            self.countries[country_name] = cc
            names.add(pair)
            subdivisions.add(row["subdivision"])

    def region(self, country, region):
        cc = self.countries.get(country.casefold(), country.upper())
        found = [prefix for prefix, row in self.regions.items() if row["country"] == cc
                 and region.casefold() in (row["region"].casefold(), row["region_name"].casefold())]
        if len(found) != 1:
            raise ValueError("country/region is not in the approved table; request an extension")
        return found[0]

    def validate(self, value, role, legacy=False):
        value = node_id(value)
        if role not in ("END", "HOST", "ROOT"):
            raise ValueError("role must be END, HOST or ROOT")
        if value == "CNETROOT":
            if role != "ROOT":
                raise ValueError("CNETROOT is reserved for ROOT")
            return value
        if legacy:
            return value  # Explicit old assignment, not inferred geography.
        if not re.fullmatch(r"[A-Z]{2}[A-Z0-9]{2}[0-9]{3}", value) or value[:4] not in self.regions:
            raise ValueError("new assignment requires an approved CCRRNNN address")
        expected = "HOST" if int(value[4:]) == 0 or int(value[4:]) >= 900 else "END"
        if role != expected:
            raise ValueError("assignment range does not agree with explicit role")
        return value

    def reservations(self, data):
        if not isinstance(data, list) or len(data) > 10000:
            raise ValueError("reservation input must be a bounded list")
        seen = set()
        for row in data:
            if (not isinstance(row, dict) or set(row) != {"node", "role", "status", "legacy"}
                    or type(row["legacy"]) is not bool
                    or row["status"] not in ("reserved", "active", "retired")):
                raise ValueError("invalid reservation row")
            value = self.validate(row["node"], row["role"], row["legacy"])
            if value in seen:
                raise ValueError("duplicate Node ID in network reservation list")
            seen.add(value)
        return seen  # Retired addresses stay reserved; no silent reuse.

    def suggest(self, country, region, kind, reservations):
        used = self.reservations(reservations)
        if kind == "ROOT":
            if any(row["role"] == "ROOT" and row["status"] == "active" for row in reservations):
                raise ValueError("an active ROOT exists; a reviewed authority transition is required")
            candidates = ["CNETROOT"]
        else:
            if kind not in ("END", "PRIMARY-HOST", "ADDITIONAL-HOST"):
                raise ValueError("unknown assignment kind")
            prefix = self.region(country, region)
            numbers = range(1, 900) if kind == "END" else (range(1) if kind == "PRIMARY-HOST" else range(900, 1000))
            candidates = (prefix + f"{n:03d}" for n in numbers)
        for value in candidates:
            if value not in used:
                return value
        raise ValueError("assignment range exhausted; administrative review required")


def read_json(path):
    with path.open("rb") as source:
        data = source.read(2 * 1024 * 1024 + 1)
    if len(data) > 2 * 1024 * 1024:
        raise ValueError("assignment input exceeds 2 MiB")
    return json.loads(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--regions", type=Path, required=True)
    parser.add_argument("--reservations", type=Path, required=True)
    parser.add_argument("--country", default="")
    parser.add_argument("--region", default="")
    parser.add_argument("--kind", choices=["ROOT", "PRIMARY-HOST", "END", "ADDITIONAL-HOST"], required=True)
    parser.add_argument("--requested", help="check a requested address instead of suggesting")
    args = parser.parse_args()
    try:
        book = AddressBook(read_json(args.regions))
        reservations = read_json(args.reservations)
        suggested = book.suggest(args.country, args.region, args.kind, reservations)
        if args.requested:
            role = "HOST" if "HOST" in args.kind else args.kind
            suggested = book.validate(args.requested, role)
            if suggested in book.reservations(reservations):
                raise ValueError("requested address is already reserved")
            if args.kind != "ROOT":
                number = int(suggested[4:])
                if (not suggested.startswith(book.region(args.country, args.region))
                        or (args.kind == "PRIMARY-HOST" and number != 0)
                        or (args.kind == "ADDITIONAL-HOST" and number < 900)):
                    raise ValueError("requested address disagrees with location/assignment kind")
        print(json.dumps({"suggested_node_id": suggested,
                          "role": "HOST" if "HOST" in args.kind else args.kind,
                          "assigned": False, "notice": "Administrator approval and reservation required"}))
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(2, f"Assignment check failed: {error}\n")


if __name__ == "__main__":
    main()
