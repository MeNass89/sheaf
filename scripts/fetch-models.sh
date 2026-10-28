#!/bin/sh
# Downloads the OCR models into vendor/models/ and checks their SHA-256: ocrs' detection and
# recognition (CC-BY-SA-4.0, see crates/sheaf-app/macos/ocr-models-NOTICE.txt) and PaddleOCR's
# PP-OCRv5 Latin recognition with its dictionary (Apache-2.0), which reads accented letters.
# Idempotent: models already present with the right checksum are kept.
set -eu
dir="$(cd "$(dirname "$0")/.." && pwd)/vendor/models"
mkdir -p "$dir"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

# fetch_url FILE URL SHA256
fetch_url() {
    file="$dir/$1"
    url="$2"
    sum="$3"
    if [ -f "$file" ] && [ "$(sha256 "$file")" = "$sum" ]; then
        return 0
    fi
    tmp="$(mktemp)"
    trap 'rm -f "$tmp"' EXIT INT TERM
    curl -fsSL -o "$tmp" "$url"
    got="$(sha256 "$tmp")"
    if [ "$got" != "$sum" ]; then
        rm -f "$tmp"
        echo "error: $1: checksum $got, expected $sum" >&2
        exit 1
    fi
    chmod 644 "$tmp"
    mv "$tmp" "$file"
    trap - EXIT INT TERM
}

fetch() {
    # ocrs' own bucket refuses downloads since 27/10/2026: our mirror (same files, checked below).
    fetch_url "$1.rten" "https://github.com/MeNass89/sheaf/releases/download/models-1/$1.rten" "$2"
}

fetch text-detection f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca
fetch text-recognition e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e
paddle="https://huggingface.co/monkt/paddleocr-onnx/resolve/7b02d0a30a07ba2b92ad1ff5a8941ae2c633de65/languages/latin"
fetch_url latin-recognition.onnx "$paddle/rec.onnx" 614ffc2d6d3902d360fad7f1b0dd455ee45e877069d14c4e51a99dc4ef144409
fetch_url latin-dict.txt "$paddle/dict.txt" 3c0a8a79b612653c25f765271714f71281e4e955962c153e272b7b8c1d2b13ff
