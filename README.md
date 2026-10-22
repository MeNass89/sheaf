<p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="crates/sheaf-app/macos/icon-dark.svg"><img src="crates/sheaf-app/macos/icon.svg" width="96" alt="Sheaf icon"></picture></p>

<h1 align="center">Sheaf</h1>

<p align="center"><b>A fast, private, open-source PDF toolbox for Mac, Windows and Linux.</b><br>
Merge, compress, convert, OCR, sign and redact PDFs — offline, in a native app.</p>

<p align="center"><a href="https://ko-fi.com/MeNass89"><img src="https://img.shields.io/badge/Ko--fi-support-FF5E5B?logo=ko-fi&logoColor=white" alt="Support on Ko-fi"></a> <a href="https://github.com/sponsors/MeNass89"><img src="https://img.shields.io/badge/GitHub-sponsor-EA4AAA?logo=githubsponsors&logoColor=white" alt="Sponsor on GitHub"></a></p>

---

Sheaf is a native desktop app (Rust + [Slint](https://slint.dev)) for everyday PDF work. Everything runs on your machine: no account, no upload, no paywall, no telemetry.

## Tools

All 28 tools run on your machine:

| Organize | Optimize | Convert to PDF | Convert from PDF | Edit | Security |
|---|---|---|---|---|---|
| Merge · Split · Remove pages · Extract pages · Organize · Rotate | Compress (3 levels) · Repair · OCR | JPG/PNG/WebP/TIFF → PDF · Word · PowerPoint · Excel · HTML | PDF → JPG · Word · PowerPoint · Excel · PDF/A | Add text and images · Page numbers · Watermark · Crop | Sign (drawn, typed or image; optional .p12 certificate) · Protect (AES-256) · Unlock · Redact (verified) · Compare |

PDF → Office keeps the text and an approximate layout. Redact checks the written file and refuses to finish if any searched text is left in it.

Office and HTML conversion use [LibreOffice](https://www.libreoffice.org/download/) if it is installed; every other tool is self-contained.

## Why

| | Sheaf 0.1 | A popular commercial PDF desktop app |
|---|---|---|
| App size | **29 MB** | 334 MB |
| Launch to first window | **0.35 s** | 1.12 s |
| Idle memory | **115 MB** | 617 MB |
| Compress (sample 3.2 MB guide, "Recommended") | **−42 %** | −29 % |

Sheaf 0.2 adds the PdfCraft engine and the OCR models: the app is 92 MB (50 MB download). Launch and memory figures for 0.2 are still to be measured.

Measured on an Apple Silicon Mac — method and raw numbers in [docs/BENCH.md](docs/BENCH.md).

## Install

Download from [Releases](../../releases):

- **macOS** (12+, Apple Silicon): `Sheaf-x.y.z-arm64.dmg`. Open it and drag **Sheaf** to Applications. The build is ad-hoc signed (not notarized), so on first launch macOS will refuse to open it: right-click **Sheaf → Open**, or run `xattr -dr com.apple.quarantine /Applications/Sheaf.app`.
- **Windows** (10+, x64): `Sheaf-x.y.z-windows-x64.zip`. Unzip it anywhere and run `Sheaf.exe`. The build is not code-signed, so SmartScreen may warn on first launch: **More info → Run anyway**.
- **Linux** (x64, glibc 2.35+): `Sheaf-x.y.z-linux-x64.tar.gz`. Unpack it anywhere and run `bin/sheaf-app`. `share/applications/sheaf.desktop` adds it to the app menu if copied to `~/.local/share/applications/` (adjust its `Exec=` to the full path).

The Windows and Linux builds are compiled and smoke-tested by CI on every release; day-to-day testing happens on macOS.

## Command line

The app ships a CLI: `Sheaf.app/Contents/Helpers/sheaf` on macOS, `sheaf-cli.exe` on Windows, `bin/sheaf` on Linux.

```sh
sheaf merge a.pdf b.pdf -o merged.pdf
sheaf compress big.pdf --level extreme -o small.pdf
sheaf split book.pdf --every 10 -o chapters/
sheaf protect report.pdf --user-password s3cret -o locked.pdf
sheaf office letter.docx -o letter.pdf
sheaf ocr scan.pdf -o searchable.pdf
sheaf redact contract.pdf --text "Jane Doe" --pattern email -o redacted.pdf
sheaf sign form.pdf --page 1 --at 400,700 --text "J. Doe" -o signed.pdf
sheaf edit form.pdf --text "Approved" --page 1 --at 72,72 -o edited.pdf
```

`sheaf --help` lists every subcommand. Exit code 2 means a password is missing or wrong.

### For agents and scripts

```sh
sheaf info doc.pdf                  # pages, sizes, encryption, metadata, form fields, bookmarks (JSON)
sheaf text doc.pdf --pages 1-2      # the text the tools read, pages separated by a form feed
sheaf render out.pdf --page 1 -o p1.png   # look at a result
sheaf --json compress big.pdf -o small.pdf
# {"ok":true,"outputs":["small.pdf"],"notes":["412345 -> 120456 bytes"]}
# on failure: {"ok":false,"error":"…","code":"password_required"} (codes: password_required,
# wrong_password, invalid_range, office_missing, unsupported, usage, io, engine)
sheaf tools                         # every command as a JSON schema
```

`sheaf mcp` serves every command as a [Model Context Protocol](https://modelcontextprotocol.io) tool over stdio, so an agent (Claude Code, Codex, Cursor…) can merge, redact, sign or edit PDFs directly. `render` also returns the page as an image, so the agent can check its own work. With Claude Code:

```sh
claude mcp add sheaf -- /Applications/Sheaf.app/Contents/Helpers/sheaf mcp
```

Tools never overwrite a file: when the output name is taken they write `name (1).pdf`, and the path actually written is in `outputs`.

## Build from source

```sh
sh scripts/fetch-pdfium.sh          # downloads the pdfium library for this platform
sh scripts/fetch-models.sh          # the OCR models
bash scripts/test.sh                # every test, one verdict (needs qpdf and poppler; Xvfb + xdotool for the GUI layer)
cargo run -p sheaf-app              # the GUI
sh scripts/bundle-macos.sh          # → target/Sheaf.app (needs: brew install librsvg; cargo install cargo-about --features cli)
sh scripts/package-linux.sh         # → target/Sheaf-x.y.z-linux-x64.tar.gz
sh scripts/package-windows.sh       # → target/Sheaf-x.y.z-windows-x64.zip (Git Bash)
```

The map of the code (layout, entry points, where to change what): [ARCHITECTURE.md](ARCHITECTURE.md).

## Support

Sheaf is free and stays free. If it saves you a subscription, you can [buy me a coffee on Ko-fi](https://ko-fi.com/MeNass89) or [sponsor on GitHub](https://github.com/sponsors/MeNass89).

## License

Sheaf's own source code is GPL-3.0-or-later. The distributed app links Slint under GPL-3.0-only and is therefore distributed under GPL-3.0. Third-party notices ship inside the app (`Contents/Resources/licenses/` on macOS, `licenses/` in the Windows zip, `share/doc/sheaf/` in the Linux package). Icons: [Lucide](https://lucide.dev) (ISC). PDF engine: [pdfium](https://pdfium.googlesource.com/pdfium/) (BSD-3-Clause) via [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries). Built on [PdfCraft](https://github.com/storytold/pdfcraft) (MIT OR Apache-2.0), which embeds the Dancing Script and Inter fonts (SIL OFL 1.1). Typed signatures also use Great Vibes and Caveat (SIL OFL 1.1) and Satisfy (Apache-2.0).

Sheaf is an independent project, not affiliated with any other PDF vendor.
