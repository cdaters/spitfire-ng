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

"""Package an already-built D1 runtime under a separate Ed25519 release authority.

Requires Python cryptography. Private keys and generated packages are local
release inputs, never repository/publication inputs. --generate-test-key is
explicitly for disposable acceptance, not an official signing ceremony.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

DOMAIN = b"SPITFIRE-NG-RELEASE-V1\n"


def file_hash(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build(binary, output, key_file, minimum, generate_test_key=False):
    binary, output, key_file = Path(binary), Path(output), Path(key_file)
    if binary.is_symlink() or not binary.is_file():
        raise ValueError("runtime must be a regular file")
    binary = binary.resolve()
    descriptor = json.loads(subprocess.check_output(
        [str(binary), "deployment-describe"], timeout=60
    ))
    if descriptor["product"] != "spitfire-ng" or descriptor["manager_protocol"] != 1:
        raise ValueError("unsupported runtime product/deployment protocol")
    version = descriptor["version"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("runtime version is not a portable SemVer release id")
    if generate_test_key:
        key = Ed25519PrivateKey.generate()
        encoded = key.private_bytes(serialization.Encoding.DER,
                                    serialization.PrivateFormat.PKCS8,
                                    serialization.NoEncryption())
        fd = os.open(key_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as target:
            target.write(encoded)
    if key_file.is_symlink():
        raise ValueError("private signing key must not be a symlink")
    key = serialization.load_der_private_key(key_file.read_bytes(), password=None)
    if not isinstance(key, Ed25519PrivateKey):
        raise ValueError("release authority must be Ed25519")
    public = key.public_key().public_bytes(serialization.Encoding.Raw,
                                         serialization.PublicFormat.Raw)
    public_path = key_file.with_name(key_file.name + ".public.hex")
    if public_path.exists():
        if public_path.read_text().strip() != public.hex():
            raise ValueError("existing public-key file differs; refusing replacement")
    else:
        with public_path.open("x", encoding="ascii") as target:
            target.write(public.hex() + "\n")
    output.mkdir(parents=True, exist_ok=True)
    destination = output / version
    if destination.exists():
        raise ValueError("release version already exists; refusing replacement")
    executable = "spitfire.exe" if descriptor["platform"] == "windows" else "spitfire"
    manifest = {
        "format": 1, "runtime": descriptor, "minimum_upgrade_version": minimum,
        "executable": executable, "size_bytes": binary.stat().st_size,
        "sha256": file_hash(binary),
    }
    encoded = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8")
    signature = key.sign(DOMAIN + encoded)
    key.public_key().verify(signature, DOMAIN + encoded)
    with tempfile.TemporaryDirectory(prefix=".runtime-package-", dir=output) as staging:
        stage = Path(staging)
        shutil.copy2(binary, stage / executable)
        if file_hash(stage / executable) != manifest["sha256"]:
            raise ValueError("runtime changed while packaging")
        (stage / "release.json").write_bytes(encoded)
        (stage / "release.sig").write_text(signature.hex(), encoding="ascii")
        stage.rename(destination)
    print(f"Created authenticated runtime {version}: {destination}")
    print(f"Separate release-authority fingerprint: {hashlib.sha256(public).hexdigest()}")
    print(f"Operator public pin file: {public_path}")
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--private-key", required=True)
    parser.add_argument("--minimum-upgrade-version", default="0.1.0")
    parser.add_argument("--generate-test-key", action="store_true")
    args = parser.parse_args()
    build(args.binary, args.output, args.private_key, args.minimum_upgrade_version,
          args.generate_test_key)


if __name__ == "__main__":
    main()
