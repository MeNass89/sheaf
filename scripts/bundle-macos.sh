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
# Shipped builds must embed the non-Latin faces (.cargo/config.toml): fail rather than build without.
sh scripts/fetch-fonts.sh
export CRAFT_FONTS_REQUIRED=1
for m in text-detection.rten text-recognition.rten latin-recognition.onnx latin-dict.txt; do
    [ -f "vendor/models/$m" ] || { echo "error: vendor/models/$m missing" >&2; exit 1; }
done

echo "==> cargo build --release"
# Panic locations embed source paths: map the builder's home and the checkout to neutral
# prefixes so no local path ships in the binaries (checked after assembly).
# env!("CARGO_MANIFEST_DIR") in dependencies is not remapped, so cargo also builds through a
# neutral CARGO_HOME link.
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
neutral="/tmp/sheaf-cargo"
ln -sfn "$cargo_home" "$neutral"
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$neutral=/cargo --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$root=/sheaf --remap-path-prefix=$HOME=/home"
export RUSTFLAGS
CARGO_HOME="$neutral" cargo build --release -p sheaf-app -p sheaf-cli

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
# macOS 26+ reads Assets.car, whose icon follows the light or dark appearance;
# older systems fall back to AppIcon.icns. actool ships with full Xcode only and
# resolves relative paths against its own daemon, hence the absolute paths.
rm -rf target/icon-car
if xcrun --find actool >/dev/null 2>&1; then
    mkdir -p target/icon-car
    xcrun actool "$root/$mac/AppIcon.icon" --compile "$root/target/icon-car" --platform macosx \
        --minimum-deployment-target 11.0 --app-icon AppIcon --include-all-app-icons \
        --output-partial-info-plist "$root/target/icon-car/partial.plist" >/dev/null
    [ -f target/icon-car/Assets.car ] || { echo "error: actool produced no Assets.car" >&2; exit 1; }
else
    echo "warning: actool not found (needs Xcode): the icon will not follow dark mode" >&2
fi

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
cp vendor/models/*.rten vendor/models/latin-recognition.onnx vendor/models/latin-dict.txt "$app/Contents/Resources/models/"
cp "$mac/ocr-models-NOTICE.txt" "$app/Contents/Resources/licenses/ocr-models-NOTICE.txt"
cp "$mac/ocr-latin-NOTICE.txt" "$app/Contents/Resources/licenses/ocr-latin-NOTICE.txt"
cp third_party/pdfcraft-ocr/LICENSE-APACHE "$app/Contents/Resources/licenses/ocr-latin-LICENSE-APACHE.txt"
cp "$mac/onnxruntime-LICENSE.txt" "$app/Contents/Resources/licenses/onnxruntime-LICENSE.txt"
cp target/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
# AppleScript's dictionary (Info.plist: OSAScriptingDefinition), handled by src/scripting_mac.rs.
cp "$mac/Sheaf.sdef" "$app/Contents/Resources/Sheaf.sdef"
[ -f target/icon-car/Assets.car ] && cp target/icon-car/Assets.car "$app/Contents/Resources/Assets.car"
sed "s/@VERSION@/$version/g" "$mac/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist" >/dev/null
printf 'APPL????' > "$app/Contents/PkgInfo"
cp LICENSE "$app/Contents/Resources/LICENSE"
cp "$mac/THIRD_PARTY.md" "$app/Contents/Resources/THIRD_PARTY.md"
cp crates/sheaf-app/ui/icons/LICENSE "$app/Contents/Resources/licenses/lucide-ISC.txt"
cp "$pdfium"/licenses/* "$app/Contents/Resources/licenses/pdfium/"
cp "$pdfium/LICENSE" "$app/Contents/Resources/licenses/pdfium/pdfium-binaries-MIT.txt"
# PdfCraft's licences and the OFL texts of the fonts it embeds, from the pinned git checkout.
pdfcraft_rev="$(sed -n 's/^pdfcraft-engine = .*rev = "\([0-9a-f]*\)".*/\1/p' Cargo.toml)"
pdfcraft_src=""
for d in "$cargo_home"/git/checkouts/pdfcraft-*/"$(printf %.7s "$pdfcraft_rev")"; do
    [ -f "$d/NOTICE" ] && pdfcraft_src="$d"
done
[ -n "$pdfcraft_src" ] || { echo "error: PdfCraft checkout for rev $pdfcraft_rev not found under $cargo_home/git/checkouts" >&2; exit 1; }
mkdir -p "$app/Contents/Resources/licenses/pdfcraft" "$app/Contents/Resources/licenses/fonts"
for f in LICENSE-MIT LICENSE-APACHE NOTICE; do
    cp "$pdfcraft_src/$f" "$app/Contents/Resources/licenses/pdfcraft/$f"
done
for f in OFL-DancingScript.txt OFL-Inter.txt; do
    cp "$pdfcraft_src/assets/fonts/$f" "$app/Contents/Resources/licenses/fonts/$f"
done
# Sheaf's own handwriting fonts (typed signatures).
cp crates/sheaf-core/assets/fonts/OFL-GreatVibes.txt crates/sheaf-core/assets/fonts/OFL-Caveat.txt \
    crates/sheaf-core/assets/fonts/LICENSE-Satisfy.txt "$app/Contents/Resources/licenses/fonts/"
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
