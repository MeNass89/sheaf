#!/bin/sh
# Downloads the OCR models (ocrs) into vendor/models/ and checks their SHA-256.
# Licence: CC-BY-SA-4.0 (see crates/sheaf-app/macos/ocr-models-NOTICE.txt).
# Idempotent: models already present with the right checksum are kept.
set -eu
dir="$(cd "$(dirname "$0")/.." && pwd)/vendor/models"
mkdir -p "$dir"

fetch() {
    name="$1"
    sum="$2"
    file="$dir/$name.rten"
    if [ -f "$file" ] && [ "$(shasum -a 256 "$file" | cut -d' ' -f1)" = "$sum" ]; then
        return 0
    fi
    tmp="$(mktemp)"
    trap 'rm -f "$tmp"' EXIT INT TERM
    curl -fsSL -o "$tmp" "https://ocrs-models.s3-accelerate.amazonaws.com/$name.rten"
    got="$(shasum -a 256 "$tmp" | cut -d' ' -f1)"
    if [ "$got" != "$sum" ]; then
        rm -f "$tmp"
        echo "error: $name.rten: checksum $got, expected $sum" >&2
        exit 1
    fi
    chmod 644 "$tmp"
    mv "$tmp" "$file"
    trap - EXIT INT TERM
}

fetch text-detection f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca
fetch text-recognition e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e
