#!/bin/sh
# Builds target/Sheaf.app (arm64, ad-hoc signed). Idempotent: the previous bundle is removed.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export PATH="$HOME/.cargo/bin:$PATH"

mac="crates/sheaf-app/macos"
pdfium="vendor/pdfium"
app="target/Sheaf.app"
# Required tools, checked before anything is built.
missing=""
for tool in cargo rsvg-convert iconutil codesign plutil install_name_tool otool python3; do
    command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
cargo about --version >/dev/null 2>&1 || missing="$missing cargo-about"
if [ -n "$missing" ]; then
    echo "error: missing tools:$missing" >&2
    echo "  rsvg-convert: brew install librsvg" >&2
    echo "  cargo-about:  cargo install cargo-about --locked --features cli" >&2
    echo "  iconutil, codesign, plutil, install_name_tool, otool: xcode-select --install" >&2
    exit 1
fi

version="$(cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "sheaf-app"))')"
[ -n "$version" ] || { echo "error: could not read the sheaf-app version" >&2; exit 1; }

sh scripts/fetch-pdfium.sh
[ -f "$pdfium/lib/libpdfium.dylib" ] || { echo "error: $pdfium/lib/libpdfium.dylib missing" >&2; exit 1; }
sh scripts/fetch-models.sh
for m in text-detection text-recognition; do
    [ -f "vendor/models/$m.rten" ] || { echo "error: vendor/models/$m.rten missing" >&2; exit 1; }
done

echo "==> cargo build --release"
# Panic locations embed source paths: map the builder's home and the checkout to neutral
# prefixes so no local path ships in the binaries (checked after assembly).
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$root=/sheaf --remap-path-prefix=$HOME=/home"
export RUSTFLAGS
cargo build --release -p sheaf-app -p sheaf-cli

echo "==> icon"
iconset="target/AppIcon.iconset"
rm -rf "$iconset"
mkdir -p "$iconset"
for s in 16 32 128 256 512; do
    rsvg-convert -w "$s" -h "$s" "$mac/icon.svg" -o "$iconset/icon_${s}x${s}.png"
    rsvg-convert -w $((s * 2)) -h $((s * 2)) "$mac/icon.svg" -o "$iconset/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$iconset" -o "target/AppIcon.icns"
rm -rf "$iconset"

echo "==> assemble $app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Helpers" "$app/Contents/Frameworks" "$app/Contents/Resources/licenses/pdfium" \
    "$app/Contents/Resources/models"
cp target/release/sheaf-app "$app/Contents/MacOS/Sheaf"
# The CLI lives in Helpers: MacOS/sheaf would collide with MacOS/Sheaf on a
# case-insensitive volume. ../Frameworks resolves to the same folder from both.
cp target/release/sheaf "$app/Contents/Helpers/sheaf"
cp "$pdfium/lib/libpdfium.dylib" "$app/Contents/Frameworks/"
# OCR models: PdfCraft looks in <exe>/../Resources/models (the CLI in Helpers finds them too).
cp vendor/models/*.rten "$app/Contents/Resources/models/"
cp "$mac/ocr-models-NOTICE.txt" "$app/Contents/Resources/licenses/ocr-models-NOTICE.txt"
cp target/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
sed "s/@VERSION@/$version/g" "$mac/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist" >/dev/null
printf 'APPL????' > "$app/Contents/PkgInfo"
cp LICENSE "$app/Contents/Resources/LICENSE"
cp "$mac/THIRD_PARTY.md" "$app/Contents/Resources/THIRD_PARTY.md"
cp crates/sheaf-app/ui/icons/LICENSE "$app/Contents/Resources/licenses/lucide-ISC.txt"
cp "$pdfium"/licenses/* "$app/Contents/Resources/licenses/pdfium/"
cp "$pdfium/LICENSE" "$app/Contents/Resources/licenses/pdfium/pdfium-binaries-MIT.txt"
echo "==> third-party licences (cargo-about)"
cargo about generate --workspace -c about.toml about/template.hbs \
    -o "$app/Contents/Resources/licenses/THIRD_PARTY_LICENSES.html"

echo "==> no local paths in the bundle"
leaks=$(for f in "$app/Contents/MacOS/Sheaf" "$app/Contents/Helpers/sheaf"; do strings "$f" | grep -c -F "$HOME" || true; done | paste -sd+ - | bc)
[ "$leaks" = 0 ] || { echo "error: $leaks occurrences of the builder's home path in the binaries" >&2; exit 1; }

echo "==> rpath + sign"
for bin in "$app/Contents/MacOS/Sheaf" "$app/Contents/Helpers/sheaf"; do
    otool -l "$bin" | grep -q "@executable_path/../Frameworks" ||
        install_name_tool -add_rpath @executable_path/../Frameworks "$bin"
done
codesign --force --deep -s - "$app"
codesign --verify --deep --strict "$app"

echo "==> done: $app ($(du -sh "$app" | cut -f1))"
