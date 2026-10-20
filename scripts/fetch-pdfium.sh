#!/bin/sh
# Downloads the prebuilt pdfium for this platform into vendor/pdfium/ (bblanchon/pdfium-binaries).
# PDFIUM_PLATFORM overrides the detected one (mac-arm64, linux-x64, win-x64, …).
set -eu
dir="$(cd "$(dirname "$0")/.." && pwd)/vendor/pdfium"
if [ -z "${PDFIUM_PLATFORM:-}" ]; then
    case "$(uname -s)-$(uname -m)" in
        Darwin-arm64) PDFIUM_PLATFORM=mac-arm64 ;;
        Darwin-x86_64) PDFIUM_PLATFORM=mac-x64 ;;
        Linux-x86_64) PDFIUM_PLATFORM=linux-x64 ;;
        Linux-aarch64) PDFIUM_PLATFORM=linux-arm64 ;;
        MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64) PDFIUM_PLATFORM=win-x64 ;;
        *) echo "error: no pdfium build known for $(uname -s)-$(uname -m); set PDFIUM_PLATFORM" >&2; exit 1 ;;
    esac
fi
for lib in lib/libpdfium.dylib lib/libpdfium.so bin/pdfium.dll; do
    [ -f "$dir/$lib" ] && exit 0
done
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
curl -fsSL -o "$tmp" "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8100/pdfium-$PDFIUM_PLATFORM.tgz"
mkdir -p "$dir"
tar xzf "$tmp" -C "$dir"
