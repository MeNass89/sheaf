#!/bin/sh
set -eu
dir="$(cd "$(dirname "$0")/.." && pwd)/vendor/pdfium"
[ -f "$dir/lib/libpdfium.dylib" ] && exit 0
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
curl -fsSL -o "$tmp" https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8100/pdfium-mac-arm64.tgz
mkdir -p "$dir"
tar xzf "$tmp" -C "$dir"
