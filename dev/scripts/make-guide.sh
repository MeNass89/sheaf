#!/bin/bash
# Makes crates/sheaf-app/guide/Sheaf Guide.pdf from sheaf-guide.md (Help ▸ Sheaf Guide): Pandoc to
# HTML with the guide's style, then Chrome's print to PDF (A4, no header or footer). Run it after
# editing the Markdown; the unit test the_guide_pdf_holds_every_section_of_its_markdown checks it was.
set -eu
cd "$(dirname "$0")/../../crates/sheaf-app/guide"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
pandoc sheaf-guide.md -f gfm -t html5 -s --metadata pagetitle="Sheaf Guide" -H style.html -o "$tmp/guide.html"
chrome=$(command -v google-chrome || command -v chromium || command -v chromium-browser)
"$chrome" --headless --disable-gpu --no-pdf-header-footer --user-data-dir="$tmp/profile" --print-to-pdf="$tmp/guide.pdf" "file://$tmp/guide.html" 2>/dev/null
mv "$tmp/guide.pdf" "Sheaf Guide.pdf"
echo "made $(pwd)/Sheaf Guide.pdf ($(stat -c %s 'Sheaf Guide.pdf') bytes)"
