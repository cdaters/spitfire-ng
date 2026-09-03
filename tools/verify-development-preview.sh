#!/bin/sh
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

set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: $0 RELEASE-ARCHIVE.tar.gz" >&2
    exit 1
fi

archive=$(CDPATH= cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
checksum="$archive.sha256"
[ -f "$archive" ] || { echo "error: archive is missing: $archive" >&2; exit 1; }
[ -f "$checksum" ] || { echo "error: checksum is missing: $checksum" >&2; exit 1; }

archive_directory=$(dirname -- "$archive")
archive_name=$(basename -- "$archive")
(
    cd "$archive_directory"
    shasum -a 256 -c "${archive_name}.sha256"
)

if tar -tzf "$archive" | awk 'BEGIN { bad = 0 } /(^\/|(^|\/)\.\.($|\/))/ { bad = 1 } END { exit bad }'
then
    :
else
    echo "error: archive contains an unsafe path" >&2
    exit 1
fi

working_directory=$(mktemp -d "${TMPDIR:-/tmp}/spitfire-verify.XXXXXX")
trap 'rm -rf "$working_directory"' EXIT HUP INT TERM
COPYFILE_DISABLE=1 tar -xzf "$archive" -C "$working_directory"

root_count=$(find "$working_directory" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
[ "$root_count" = "1" ] || { echo "error: archive must contain exactly one root directory" >&2; exit 1; }
package_root=$(find "$working_directory" -mindepth 1 -maxdepth 1 -type d)
[ -z "$(find "$package_root" -type l -print -quit)" ] || { echo "error: archive contains a symlink" >&2; exit 1; }

for required in \
    bin/spitfire \
    README.md \
    RELEASE-NOTES.md \
    RELEASE.toml \
    MANIFEST.SHA256 \
    licenses/SPITFIRE-NG/LICENSE-MIT \
    licenses/SPITFIRE-NG/LICENSE-APACHE \
    licenses/third-party/THIRD-PARTY-NOTICES.md \
    docs/operator/custom-display-screens.md \
    docs/operator/getting-started.md \
    docs/operator/macos-first-run.md \
    docs/operator/support.md \
    docs/research/m038-1-display-authoring-compatibility.md
do
    [ -f "$package_root/$required" ] || { echo "error: package is missing $required" >&2; exit 1; }
done

if grep -R -I -E '/Users/[^/[:space:]]+|/home/[^/[:space:]]+|[A-Za-z]:\\Users\\' "$package_root"
then
    echo "error: package text contains a private build-home path" >&2
    exit 1
fi

if strings "$package_root/bin/spitfire" | grep -E '/Users/[^/[:space:]]+|/home/[^/[:space:]]+|[A-Za-z]:\\Users\\'
then
    echo "error: executable contains a private build-home path" >&2
    exit 1
fi

(
    cd "$package_root"
    shasum -a 256 -c MANIFEST.SHA256
)
"$package_root/bin/spitfire" --version
echo "Verified extracted Development Preview package: $package_root"
