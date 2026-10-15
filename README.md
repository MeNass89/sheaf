<p align="center"><img src="crates/sheaf-app/macos/icon.svg" width="96" alt="Sheaf icon"></p>

<h1 align="center">Sheaf</h1>

<p align="center"><b>A fast, private, open-source PDF toolbox for the Mac.</b><br>
Merge, split, compress, protect and convert PDFs — offline, in a 29 MB native app.</p>

---

Sheaf is a native macOS app (Rust + [Slint](https://slint.dev)) for everyday PDF work. Everything runs on your machine: no account, no upload, no paywall, no telemetry.

## Tools

| Organize | Optimize | Convert to PDF | Convert from PDF | Security |
|---|---|---|---|---|
| Merge · Split · Remove pages · Extract pages · Organize · Rotate | Compress (3 levels) · Repair | JPG/PNG/WebP/TIFF → PDF · Word · PowerPoint · Excel · HTML | PDF → JPG | Protect (AES-256) · Unlock |

Coming next: OCR, page numbers, watermark, crop, PDF/A, editing, signing, redaction, compare, PDF → Office.

Office and HTML conversion use [LibreOffice](https://www.libreoffice.org/download/) if it is installed; every other tool is self-contained.

## Why

| | Sheaf 0.1 | A popular commercial PDF desktop app |
|---|---|---|
| App size | **29 MB** | 334 MB |
| Launch to first window | **0.35 s** | 1.12 s |
| Idle memory | **115 MB** | 617 MB |
| Compress (sample 3.2 MB guide, "Recommended") | **−42 %** | −29 % |

Measured on an Apple Silicon Mac — method and raw numbers in [docs/BENCH.md](docs/BENCH.md).

## Install

Download the latest `Sheaf-x.y.z-arm64.dmg` from [Releases](../../releases), open it and drag **Sheaf** to Applications.

The build is ad-hoc signed (not notarized), so on first launch macOS will refuse to open it: right-click **Sheaf → Open**, or run `xattr -dr com.apple.quarantine /Applications/Sheaf.app`.

Requires macOS 12+ on Apple Silicon.

## Command line

The app ships a CLI at `Sheaf.app/Contents/Helpers/sheaf`:

```sh
sheaf merge a.pdf b.pdf -o merged.pdf
sheaf compress big.pdf --level extreme -o small.pdf
sheaf split book.pdf --every 10 -o chapters/
sheaf protect report.pdf --user-password s3cret -o locked.pdf
sheaf office letter.docx -o letter.pdf
```

`sheaf --help` lists every subcommand. Exit code 2 means a password is missing or wrong.

## Build from source

```sh
sh scripts/fetch-pdfium.sh          # downloads the pdfium library (arm64)
cargo test --workspace
cargo run -p sheaf-app              # the GUI
sh scripts/bundle-macos.sh          # → target/Sheaf.app (needs: brew install librsvg; cargo install cargo-about --features cli)
```

Workspace layout:

- `crates/sheaf-core` — the engine: pdfium (rendering, page copy), lopdf (structure, AES-256, rotation, image recompression), LibreOffice headless (Office → PDF). Every write goes through a temp file and never overwrites an existing file.
- `crates/sheaf-cli` — the `sheaf` command.
- `crates/sheaf-app` — the Slint GUI.

## License

Sheaf's own source code is GPL-3.0-or-later. The distributed app links Slint under GPL-3.0-only and is therefore distributed under GPL-3.0. Third-party notices ship inside the app (`Contents/Resources/licenses/`). Icons: [Lucide](https://lucide.dev) (ISC). PDF engine: [pdfium](https://pdfium.googlesource.com/pdfium/) (BSD-3-Clause) via [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries). Built on [PdfCraft](https://github.com/storytold/pdfcraft) (Apache-2.0).

Sheaf is an independent project, not affiliated with any other PDF vendor.
