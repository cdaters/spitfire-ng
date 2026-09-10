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

"""Build disposable native D1 acceptance runtimes without editing the workspace.

Usage: python3 tools/build-d1-fixtures.py /absolute/disposable/output
Requires cached Cargo dependencies and Python cryptography. No remote service.
The generated private key, PEM and binaries must never be published.
"""

import argparse
import datetime
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent
    output = args.output.resolve()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location("package", repository / "tools/build-runtime-release.py")
    package = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(package)
    cert_key = ec.generate_private_key(ec.SECP256R1())
    subject = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "D1 disposable acceptance only")])
    now = datetime.datetime.now(datetime.timezone.utc)
    cert = (x509.CertificateBuilder().subject_name(subject).issuer_name(subject)
            .public_key(cert_key.public_key()).serial_number(x509.random_serial_number())
            .not_valid_before(now - datetime.timedelta(minutes=1))
            .not_valid_after(now + datetime.timedelta(days=1)).sign(cert_key, hashes.SHA256()))
    (output / "synthetic-certificate.pem").write_bytes(cert.public_bytes(serialization.Encoding.PEM))
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(repository / "target/d1-native")
    # No production version override exists. Each synthetic release is compiled
    # from a disposable source tree with its actual Cargo package version.
    with tempfile.TemporaryDirectory(prefix="spitfire-d1-source-") as directory:
        source = Path(directory)
        shutil.copytree(repository / "crates", source / "crates")
        shutil.copy2(repository / "Cargo.lock", source / "Cargo.lock")
        original = (repository / "Cargo.toml").read_text()
        model_path = source / "crates/sf-bbs/src/deployment/model.rs"
        worker_path = source / "crates/sf-bbs/src/deployment/worker.rs"
        original_model, original_worker = model_path.read_text(), worker_path.read_text()
        for version in ("0.1.0", "0.1.1", "0.1.5", "0.1.6", "0.1.9"):
            (source / "Cargo.toml").write_text(original.replace('version = "0.1.0"', f'version = "{version}"', 1))
            model_path.write_text(original_model)
            worker_path.write_text(original_worker)
            if version == "0.1.6":
                # A synthetic future required feature tests incompatible runtime
                # refusal. It does not change any network protocol or semantics.
                needle = '"circuitnet-catalog-history",'
                assert needle in original_model
                model_path.write_text(original_model.replace(needle, '"d1-synthetic-future-feature", ' + needle, 1))
            if version == "0.1.9":
                needle = "        db.migrate()?;"
                assert needle in original_worker
                worker_path.write_text(original_worker.replace(needle, needle + '\n        return Err(Error::Rejected("synthetic failure after staged migration".into()));', 1))
            subprocess.run(["cargo", "build", "--offline", "-p", "sf-bbs", "--bin", "spitfire"],
                           cwd=source, env=env, check=True)
            binary = Path(env["CARGO_TARGET_DIR"]) / "debug" / ("spitfire.exe" if os.name == "nt" else "spitfire")
            package.build(binary, output / "packages", output / "test-release-key.der", "0.1.0", version == "0.1.0")
    print(f"Run: SPITFIRE_D1_FIXTURES={output} cargo test -p sf-bbs --lib deployment::tests::native_cli_acceptance -- --ignored --nocapture")


if __name__ == "__main__":
    main()
