#!/bin/sh
# Builds target/Sheaf-<version>-linux-x64.tar.gz: an FHS-style tree that runs from wherever it
# is unpacked (bin/ finds pdfium in ../lib and the OCR models in ../share/pdfcraft/models).
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
mac="crates/sheaf-app/macos"
pdfium="vendor/pdfium"

version="$(cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "sheaf-app"))')"
name="Sheaf-$version-linux-x64"
stage="target/$name"

sh scripts/fetch-pdfium.sh
sh scripts/fetch-models.sh
cargo build --release -p sheaf-app -p sheaf-cli

rm -rf "$stage"
mkdir -p "$stage/bin" "$stage/lib" "$stage/share/pdfcraft/models" "$stage/share/applications" \
    "$stage/share/icons/hicolor/scalable/apps" "$stage/share/doc/sheaf/licenses/pdfium"
cp target/release/sheaf-app target/release/sheaf "$stage/bin/"
cp "$pdfium/lib/libpdfium.so" "$stage/lib/"
cp vendor/models/*.rten vendor/models/latin-recognition.onnx vendor/models/latin-dict.txt "$stage/share/pdfcraft/models/"
cp "$mac/icon.svg" "$stage/share/icons/hicolor/scalable/apps/sheaf.svg"
cat > "$stage/share/applications/sheaf.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Sheaf
Comment=Private PDF toolbox
Exec=sheaf-app %F
Icon=sheaf
MimeType=application/pdf;
Categories=Office;Graphics;
EOF
sh scripts/collect-licenses.sh "$stage/share/doc/sheaf"

tar czf "target/$name.tar.gz" -C target "$name"
echo "==> target/$name.tar.gz ($(du -h "target/$name.tar.gz" | cut -f1))"
