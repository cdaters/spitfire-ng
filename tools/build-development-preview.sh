#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_directory=${1:-"$repository_root/dist"}
host_target=$(rustc -vV | awk '/^host:/ { print $2 }')
build_home=${HOME:?HOME must identify the release build account}
source_commit=$(git -C "$repository_root" rev-parse HEAD)

if [ -n "$(git -C "$repository_root" status --porcelain --untracked-files=no)" ]; then
    echo "error: tracked release inputs must be committed before packaging" >&2
    exit 1
fi

case "$host_target" in
    aarch64-apple-darwin) ;;
    *)
        echo "error: $host_target is not an accepted Development Preview binary target" >&2
        exit 1
        ;;
esac

cd "$repository_root"
release_rustflags=${RUSTFLAGS:-}
release_rustflags="${release_rustflags:+$release_rustflags }--remap-path-prefix=$repository_root=/usr/src/spitfire-ng"
release_rustflags="$release_rustflags --remap-path-prefix=$build_home=/build"
RUSTFLAGS="$release_rustflags" cargo build --release --locked -p sf-bbs

binary="$repository_root/target/release/spitfire"
version=$($binary --version | awk '{ print $NF }')
package_name="spitfire-ng-${version}-development-preview-${host_target}"
archive_name="${package_name}.tar.gz"
mkdir -p "$output_directory"
output_directory=$(CDPATH= cd -- "$output_directory" && pwd)
archive="$output_directory/$archive_name"
archive_checksum="$archive.sha256"

if [ -e "$archive" ] || [ -e "$archive_checksum" ]; then
    echo "error: release output already exists: $archive" >&2
    exit 1
fi

working_directory=$(mktemp -d "${TMPDIR:-/tmp}/spitfire-release.XXXXXX")
trap 'rm -rf "$working_directory"' EXIT HUP INT TERM
package_root="$working_directory/$package_name"
mkdir -p "$package_root/bin" "$package_root/docs" "$package_root/licenses/SPITFIRE-NG"

cp "$binary" "$package_root/bin/spitfire"
chmod 755 "$package_root/bin/spitfire"
cp "$repository_root/release/DEVELOPMENT-PREVIEW.md" "$package_root/README.md"
cp "$repository_root/SECURITY.md" "$package_root/docs/SECURITY.md"
for technical_document in \
    localization.md \
    presentation-profiles.md \
    sfng-backup-restore.md \
    sfng-caller-authentication.md \
    sfng-file-system.md \
    sfng-file-transfers.md \
    sfng-message-system.md \
    sfng-setup-configuration.md
do
    cp "$repository_root/release/TECHNICAL-DOCS-NOTICE.md" "$package_root/docs/$technical_document"
done
mkdir -p "$package_root/docs/operator"
cp "$repository_root/release/OPERATOR-INDEX.md" "$package_root/docs/operator/README.md"
for operator_document in \
    backup-restore.md \
    caller-management.md \
    classic-presentation.md \
    configuration.md \
    custom-display-screens.md \
    development-preview-package.md \
    files.md \
    getting-started.md \
    installation.md \
    localization.md \
    macos-first-run.md \
    messages.md \
    support.md \
    sysop-guide.md \
    terminal-clients.md \
    transfers.md \
    troubleshooting.md \
    upgrades.md
do
    cp "$repository_root/docs/operator/$operator_document" "$package_root/docs/operator/$operator_document"
done
mkdir -p "$package_root/docs/research"
cp "$repository_root/release/TECHNICAL-DOCS-NOTICE.md" \
    "$package_root/docs/research/m038-1-display-authoring-compatibility.md"
cp "$repository_root/release/RELEASE-NOTES-0.1.0-DEVELOPMENT-PREVIEW.md" \
    "$package_root/RELEASE-NOTES.md"
cp "$repository_root/LICENSE-MIT" "$package_root/licenses/SPITFIRE-NG/LICENSE-MIT"
cp "$repository_root/LICENSE-APACHE" "$package_root/licenses/SPITFIRE-NG/LICENSE-APACHE"
cp "$repository_root/Cargo.lock" "$package_root/licenses/Cargo.lock"

metadata="$working_directory/cargo-metadata.json"
cargo metadata --format-version 1 --locked --filter-platform "$host_target" > "$metadata"
ruby "$repository_root/tools/collect-third-party-licenses.rb" \
    "$metadata" "$package_root/licenses/third-party" \
    "$repository_root/LICENSE-MIT" "$repository_root/LICENSE-APACHE"

rustc_version=$(rustc -V)
cargo_version=$(cargo -V)
cargo_lock_sha256=$(shasum -a 256 "$repository_root/Cargo.lock" | awk '{ print $1 }')

cat > "$package_root/RELEASE.toml" <<EOF
format_version = 1
product = "SPITFIRE NG Bulletin Board System"
version = "$version"
channel = "development-preview"
source_commit = "$source_commit"
target = "$host_target"
archive = "$archive_name"
license = "MIT OR Apache-2.0"
presentation_profiles = ["modern-ng 1.0.1", "classic-spitfire 1.1.1", "minimal-terminal 1.0.1"]
language_packages = ["en-US 1.0.1"]
resources = "embedded; materialized board-locally by spitfire setup"
signed = false
notarized = false
rustc = "$rustc_version"
cargo = "$cargo_version"
cargo_lock_sha256 = "$cargo_lock_sha256"
build_command = "RUSTFLAGS=<source-path-remapping> cargo build --release --locked -p sf-bbs"
archive_command = "COPYFILE_DISABLE=1 tar -czf"
bit_for_bit_reproducible = false
EOF

(
    cd "$package_root"
    find . -type f ! -name MANIFEST.SHA256 -print | LC_ALL=C sort | while IFS= read -r path
    do
        shasum -a 256 "$path"
    done > MANIFEST.SHA256
)

COPYFILE_DISABLE=1 tar -czf "$archive" -C "$working_directory" "$package_name"
(
    cd "$output_directory"
    shasum -a 256 "$archive_name" > "${archive_name}.sha256"
)

echo "Created $archive"
echo "Created $archive_checksum"
