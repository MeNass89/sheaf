#!/bin/sh
# Runs a packaged command line (argument: its path) on a real PDF: pdfium loads from the
# package, a page renders, two files merge, and the OCR models are found.
set -eu
cli="$1"
root="$(cd "$(dirname "$0")/.." && pwd)"
pdf="$root/crates/sheaf-core/tests/fixtures/invoice.pdf"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
check() { printf '%s\n' "$2" | grep -q '"ok":true' || { echo "error: $1: $2" >&2; exit 1; }; echo "ok: $1"; }

check info "$("$cli" --json info "$pdf")"
check render "$("$cli" --json render "$pdf" -o "$tmp/p1.png")"
[ -s "$tmp/p1.png" ] || { echo "error: render wrote nothing" >&2; exit 1; }
check merge "$("$cli" --json merge "$pdf" "$pdf" -o "$tmp/m.pdf")"
"$cli" --json info "$tmp/m.pdf" | grep -q '"page_count":2' || { echo "error: merged page count" >&2; exit 1; }
check ocr "$("$cli" --json ocr "$pdf" --all-pages -o "$tmp/o.pdf")"
