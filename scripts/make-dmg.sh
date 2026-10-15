#!/bin/sh
# Builds target/Sheaf-<version>-arm64.dmg from target/Sheaf.app (run bundle-macos.sh first).
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
app="$root/target/Sheaf.app"
[ -d "$app" ] || { echo "error: $app missing; run scripts/bundle-macos.sh" >&2; exit 1; }
version=$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$app/Contents/Info.plist")
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
out="$root/target/Sheaf-$version-arm64.dmg"
rm -f "$out"
hdiutil create -volname "Sheaf $version" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$out" >/dev/null
codesign --force -s - "$out"
echo "==> $out ($(du -h "$out" | cut -f1))"
