#!/bin/sh
# Copies Sheaf's licence and every third-party notice into <dir> (LICENSE, THIRD_PARTY.md,
# licenses/…), for the Linux and Windows packages.
set -eu
out="$1"
mac="crates/sheaf-app/macos"
pdfium="vendor/pdfium"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"

mkdir -p "$out/licenses/pdfium" "$out/licenses/pdfcraft" "$out/licenses/fonts"
cp LICENSE "$out/LICENSE"
cp "$mac/THIRD_PARTY.md" "$out/THIRD_PARTY.md"
cp "$mac/ocr-models-NOTICE.txt" "$out/licenses/ocr-models-NOTICE.txt"
cp "$mac/ocr-latin-NOTICE.txt" "$out/licenses/ocr-latin-NOTICE.txt"
cp third_party/pdfcraft-ocr/LICENSE-APACHE "$out/licenses/ocr-latin-LICENSE-APACHE.txt"
cp "$mac/onnxruntime-LICENSE.txt" "$out/licenses/onnxruntime-LICENSE.txt"
cp crates/sheaf-app/ui/icons/LICENSE "$out/licenses/lucide-ISC.txt"
cp "$pdfium"/licenses/* "$out/licenses/pdfium/"
cp "$pdfium/LICENSE" "$out/licenses/pdfium/pdfium-binaries-MIT.txt"

pdfcraft_rev="$(sed -n 's/^pdfcraft-engine = .*rev = "\([0-9a-f]*\)".*/\1/p' Cargo.toml)"
pdfcraft_src=""
for d in "$cargo_home"/git/checkouts/pdfcraft-*/"$(printf %.7s "$pdfcraft_rev")"; do
    [ -f "$d/NOTICE" ] && pdfcraft_src="$d"
done
[ -n "$pdfcraft_src" ] || { echo "error: PdfCraft checkout for rev $pdfcraft_rev not found under $cargo_home/git/checkouts" >&2; exit 1; }
for f in LICENSE-MIT LICENSE-APACHE NOTICE; do
    cp "$pdfcraft_src/$f" "$out/licenses/pdfcraft/$f"
done
for f in OFL-DancingScript.txt OFL-Inter.txt; do
    cp "$pdfcraft_src/assets/fonts/$f" "$out/licenses/fonts/$f"
done
# Sheaf's own handwriting fonts (typed signatures).
cp crates/sheaf-core/assets/fonts/OFL-GreatVibes.txt crates/sheaf-core/assets/fonts/OFL-Caveat.txt \
    crates/sheaf-core/assets/fonts/LICENSE-Satisfy.txt "$out/licenses/fonts/"
cargo about generate --workspace -c about.toml about/template.hbs -o "$out/licenses/THIRD_PARTY_LICENSES.html"
