#!/bin/sh
# Builds target/Sheaf-<version>-windows-x64.zip (run from Git Bash). Portable: Sheaf.exe, the
# command-line sheaf-cli.exe (Windows names are case-insensitive, so it cannot be sheaf.exe),
# pdfium.dll, DirectML.dll and models\ side by side.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
pdfium="vendor/pdfium"

version="$(cargo metadata --no-deps --format-version 1 |
    python -c 'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "sheaf-app"))')"
name="Sheaf-$version-windows-x64"
stage="target/$name"

sh scripts/fetch-pdfium.sh
sh scripts/fetch-models.sh
# Shipped builds must embed the non-Latin faces (.cargo/config.toml): fail rather than build without.
sh scripts/fetch-fonts.sh
export CRAFT_FONTS_REQUIRED=1
cargo build --release -p sheaf-app -p sheaf-cli

rm -rf "$stage"
mkdir -p "$stage/models" "$stage/licenses/pdfium"
cp target/release/sheaf-app.exe "$stage/Sheaf.exe"
cp target/release/sheaf.exe "$stage/sheaf-cli.exe"
cp "$pdfium/bin/pdfium.dll" "$stage/"
# onnxruntime (OCR) is linked statically, but pyke's Windows build imports DirectML; ort's
# copy-dylibs puts the DLL beside the executables it builds.
cp target/release/DirectML.dll "$stage/"
cp vendor/models/*.rten vendor/models/latin-recognition.onnx vendor/models/latin-dict.txt "$stage/models/"
sh scripts/collect-licenses.sh "$stage"

rm -f "target/$name.zip"
(cd target && 7z a -tzip -bso0 "$name.zip" "$name")
echo "==> target/$name.zip ($(du -h "target/$name.zip" | cut -f1))"
