#!/bin/sh
# Build a local installer from the already-built, signed application bundle.
# No downloads, security-setting changes, or quarantine removal.
set -eu
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app_name="Polymath"
app_bundle="$repo_root/dist/$app_name.app"
output_dir=${1:-"$repo_root/dist"}
if [ ! -d "$app_bundle" ]; then
    echo "Build dist/$app_name.app before creating the installer." >&2
    exit 1
fi
/usr/bin/codesign --verify --strict --deep "$app_bundle"
mkdir -p "$output_dir"
staging_dir=$(mktemp -d "${TMPDIR:-/tmp}/polymath-dmg.XXXXXX")
trap 'rm -rf "$staging_dir"' EXIT HUP INT TERM
mkdir "$staging_dir/contents"
/usr/bin/ditto "$app_bundle" "$staging_dir/contents/$app_name.app"
ln -s /Applications "$staging_dir/contents/Applications"
# Build the HFS image without attaching a temporary device. This also works in
# build sessions that cannot access DiskImages' device helper.
/usr/bin/hdiutil makehybrid -hfs -hfs-volume-name "$app_name" -o "$staging_dir/source.dmg" "$staging_dir/contents"
/usr/bin/hdiutil convert "$staging_dir/source.dmg" -format UDZO -o "$staging_dir/installer.dmg"
/usr/bin/hdiutil verify "$staging_dir/installer.dmg"
mv "$staging_dir/installer.dmg" "$output_dir/$app_name.dmg"
printf '%s\n' "$output_dir/$app_name.dmg"
