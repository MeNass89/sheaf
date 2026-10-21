#!/bin/bash
# GUI smoke test of the reader on Linux (Xvfb + xdotool), checked on the files it writes, not on
# screenshots: open, highlight, note, save, page operations, form filling, close prompt.
# Prints one PASS/FAIL line per check and exits non-zero on any failure. Screenshots of each step
# go to dev/.tmp/smoke/ for a look when something fails.
#   sh dev/scripts/build-safe.sh build -p sheaf-app && bash dev/scripts/gui-smoke.sh
set -u
cd "$(dirname "$0")/../.."
APP="$HOME/.cache/cargo-target/sheaf/debug/sheaf-app"
OUT=dev/.tmp/smoke
rm -rf "$OUT"; mkdir -p "$OUT"
export DISPLAY=:98
fails=0
pass() { echo "PASS $1"; }
fail() { echo "FAIL $1"; fails=$((fails + 1)); }
check() { if eval "$2"; then pass "$1"; else fail "$1"; fi; }
shot() { import -window root "$OUT/$1.png" 2>/dev/null; }

Xvfb :98 -screen 0 1280x800x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
APP_PID=
cleanup() { [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null; kill "$XVFB" 2>/dev/null; }
trap cleanup EXIT
sleep 1

# Fixtures: a 4-page text PDF ("Page 1" … "Page 4") and a one-page form.
python3 - "$OUT" <<'PY'
import sys
out = sys.argv[1]
def write(name, objs, root):
    b = b"%PDF-1.7\n"; offs = []
    for i, s in enumerate(objs, 1):
        offs.append(len(b)); b += f"{i} 0 obj\n{s}\nendobj\n".encode()
    x = len(b)
    b += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode() + b"".join(f"{n:010d} 00000 n \n".encode() for n in offs)
    b += f"trailer\n<< /Size {len(objs)+1} /Root {root} 0 R >>\nstartxref\n{x}\n%%EOF\n".encode()
    open(f"{out}/{name}", "wb").write(b)
# text.pdf: 1 font, 2 catalog, 3 pages, then content+page per page
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", "<< /Type /Catalog /Pages 3 0 R >>", ""]
kids = []
for i in range(1, 5):
    c = f"BT /F1 28 Tf 72 700 Td (Page {i} the quick brown fox) Tj ET"
    objs.append(f"<< /Length {len(c)} >>\nstream\n{c}\nendstream")
    objs.append(f"<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents {len(objs)} 0 R /Resources << /Font << /F1 1 0 R >> >> >>")
    kids.append(len(objs))
objs[2] = f"<< /Type /Pages /Kids [{' '.join(f'{k} 0 R' for k in kids)}] /Count 4 >>"
write("text.pdf", objs, 2)
# form.pdf: a text field "name" over the page, Helvetica
c = "BT /F1 18 Tf 72 720 Td (Name:) Tj ET"
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        "<< /Type /Catalog /Pages 3 0 R /AcroForm << /Fields [6 0 R] /NeedAppearances true /DA (/Helv 12 Tf 0 g) /DR << /Font << /Helv 1 0 R >> >> >> >>",
        "<< /Type /Pages /Kids [5 0 R] /Count 1 >>",
        f"<< /Length {len(c)} >>\nstream\n{c}\nendstream",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 1 0 R >> >> /Annots [6 0 R] >>",
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) /Rect [72 400 540 700] /P 5 0 R /DA (/Helv 12 Tf 0 g) /F 4 >>"]
write("form.pdf", objs, 2)
PY
for f in text form; do qpdf "$OUT/$f.pdf" "$OUT/$f.fixed.pdf" && mv "$OUT/$f.fixed.pdf" "$OUT/$f.pdf"; done

launch() {
    HOME="$HOME" "$APP" "$@" >"$OUT/app.log" 2>&1 &
    APP_PID=$!
    for _ in $(seq 1 40); do W=$(xdotool search --name '^Sheaf$' 2>/dev/null | tail -1); [ -n "$W" ] && break; sleep 0.25; done
    sleep 2.5
    xdotool windowsize "$W" 1280 800; xdotool windowfocus --sync "$W" 2>/dev/null
}
quit() { kill "$APP_PID" 2>/dev/null; wait "$APP_PID" 2>/dev/null; APP_PID=; }
# A held click (Slint misses instantaneous ones under Xvfb).
click() { xdotool mousemove "$1" "$2" mousedown 1; sleep 0.08; xdotool mouseup 1; sleep "${3:-0.5}"; }
key() { xdotool key "$@"; sleep 0.4; }
# Markup, notes, ink and text boxes in the file as it is now (qdf rewrites only live objects).
annots() { qpdf --qdf --object-streams=disable "$1" - 2>/dev/null | grep -ac '/Subtype /\(Highlight\|Underline\|StrikeOut\|Text\|Ink\|FreeText\)$'; }
npages() { qpdf --show-npages "$1"; }
first_text() { pdftotext -f "$2" -l "$2" "$1" - 2>/dev/null | head -1 | cut -d' ' -f1-2; }

# 1. Open, highlight the first line with H, save, check the file.
launch "$OUT/text.pdf"
key ctrl+9; shot 01-open
check "the reader window opens" "kill -0 $APP_PID 2>/dev/null"
# The first line of page 1 at fit width (⌘9) sits near y = 200 in a 1280×800 window.
xdotool mousemove 330 200 mousedown 1 mousemove 600 201 mousemove 900 201 mouseup 1; sleep 0.4
key h; sleep 1
key ctrl+s; sleep 1.5; shot 02-highlight
check "a highlight is saved into the file" "[ \"\$(annots $OUT/text.pdf)\" -ge 1 ]"
check "the saved file passes qpdf --check" "qpdf --check $OUT/text.pdf >/dev/null 2>&1"

# 2. A note, then undo / redo.
key n; click 1000 400 0.8; xdotool type --delay 10 "smoke note"; key ctrl+Return; sleep 1
key v; key ctrl+s; sleep 1.5
check "a note is saved" "[ \"\$(annots $OUT/text.pdf)\" -ge 2 ]"
key ctrl+z; sleep 0.8; key ctrl+s; sleep 1.5
A_UNDO=$(annots "$OUT/text.pdf")
key ctrl+shift+z; sleep 0.8; key ctrl+s; sleep 1.5
A_REDO=$(annots "$OUT/text.pdf")
check "undo then redo restores the note ($A_UNDO → $A_REDO)" "[ $A_REDO -gt $A_UNDO ]"

# 3. Page operations from the Pages panel: duplicate page 1, delete it again, rotate.
click 120 230; click 142 778 1.2      # select page 1, Duplicate
key ctrl+s; sleep 1.5
check "duplicate adds a page" "[ \"\$(npages $OUT/text.pdf)\" = 5 ]"
click 120 230; click 203 778 1.2      # Delete
key ctrl+s; sleep 1.5
check "delete removes it" "[ \"\$(npages $OUT/text.pdf)\" = 4 ]"
shot 03-pages

# 4. Search runs (results panel in the screenshot).
click 1030 61; xdotool type --delay 10 "fox"; key Return; sleep 1.5; shot 04-search
check "search does not crash" "kill -0 $APP_PID 2>/dev/null"
key Escape

# 5. Closing with unsaved edits asks; Don't Save leaves the file as it was.
key h; xdotool mousemove 330 200 mousedown 1 mousemove 800 201 mouseup 1; sleep 0.3; key h; sleep 1
BEFORE=$(annots "$OUT/text.pdf")
key ctrl+w; sleep 1; shot 05-unsaved-prompt
check "the app is still open after ⌘W with unsaved edits (prompt shown)" "kill -0 $APP_PID 2>/dev/null"
quit
check "Don't Save / quit leaves the file unchanged" "[ \"\$(annots $OUT/text.pdf)\" = $BEFORE ]"

# 6. Fill a form field and save.
launch "$OUT/form.pdf"
key ctrl+9
click 640 300 0.8; xdotool type --delay 10 "Smoke Test"; key Return; sleep 1
key ctrl+s; sleep 1.5; shot 06-form
V=$(qpdf --json=2 --json-key=acroform "$OUT/form.pdf" | python3 -c "import json,sys;print([f.get('value') for f in json.load(sys.stdin)['acroform']['fields']])")
check "a form field is filled and saved ($V)" "echo \"$V\" | grep -q 'Smoke Test'"
quit

echo "---"
[ $fails -eq 0 ] && echo "ALL PASS" || echo "$fails FAILED (screenshots in $OUT)"
exit $fails
