#!/bin/sh
# Fetches the openly licensed faces PdfCraft embeds at build time (craft-fonts, OFL-1.1) into
# vendor/craft-fonts/, with a manifest of the ones Sheaf ships: BIZ UDPGothic (Japanese, with kanji,
# Cyrillic and Greek), Noto Sans Arabic, Thai and Georgian, about 10 MB. They let Edit text and
# text boxes write what WinAnsi cannot (dev/DECISIONS.md, 25/10/2026). dev/scripts/cargo.sh passes
# the folder as CRAFT_FONTS_DIR when it exists. Idempotent: an existing checkout at the pin is kept.
set -eu
pin=fff559f5f038112a4135e95c720161d4d0a5b335
dir="$(cd "$(dirname "$0")/.." && pwd)/vendor/craft-fonts"
if [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)" != "$pin" ]; then
    rm -rf "$dir"
    git clone -q --filter=blob:none --no-checkout https://github.com/storytold/craft-fonts "$dir"
    git -C "$dir" sparse-checkout set --no-cone /fonts/biz-ud-pgothic/ /fonts/noto-sans-arabic/ /fonts/noto-sans-thai/ /fonts/noto-sans-georgian/ /fonts/manifest.txt /ATTRIBUTION.md
    git -C "$dir" checkout -q "$pin"
fi
# The manifest lists only the faces fetched (build.rs checks every line's file and checksum).
grep -E '^#|^BIZ UDPGothic|^Noto Sans (Arabic|Thai|Georgian) ' "$dir/fonts/manifest.txt" > "$dir/fonts/manifest.sheaf"
cp "$dir/fonts/manifest.sheaf" "$dir/fonts/manifest.txt"
echo "craft-fonts: $(grep -vc '^#' "$dir/fonts/manifest.txt") faces in $dir"
