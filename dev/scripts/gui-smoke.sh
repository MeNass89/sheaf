#!/bin/bash
# GUI smoke test of the reader on Linux (Xvfb + xdotool), checked on the files it writes, not on
# screenshots: open, highlight, note, save, page operations, form filling, close prompt.
# Prints one PASS/FAIL line per check and exits non-zero on any failure. Screenshots of each step
# go to dev/.tmp/smoke/, kept only when something fails. It is the `gui` layer of the one suite:
#   bash scripts/test.sh gui
set -u
cd "$(dirname "$0")/../.."
APP="${SHEAF_APP:-$HOME/.cache/cargo-target/sheaf/debug/sheaf-app}"   # set by scripts/test.sh
OUT=dev/.tmp/smoke
rm -rf "$OUT"; mkdir -p "$OUT"
export DISPLAY=:98
fails=0
pass() { echo "PASS $1"; }
fail() { echo "FAIL $1"; fails=$((fails + 1)); }
check() { if eval "$2"; then pass "$1"; else fail "$1"; fi; }
shot() { import -window root "$OUT/$1.png" 2>/dev/null; }

# A display left by an interrupted run is stopped first.
[ -f /tmp/.X98-lock ] && kill "$(tr -d ' ' </tmp/.X98-lock)" 2>/dev/null && sleep 0.5
rm -f /tmp/.X98-lock /tmp/.X11-unix/X98
Xvfb :98 -screen 0 1280x800x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
APP_PID=
cleanup() { [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null; kill "$XVFB" 2>/dev/null; }
trap cleanup EXIT
# The checks mean nothing without a display: wait for it, or stop here.
for _ in $(seq 1 40); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.25; done
xdpyinfo >/dev/null 2>&1 || { echo "FAIL no X display :98 (Xvfb did not start)"; exit 1; }

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
cp "$OUT/text.pdf" "$OUT/shapes.pdf"

# The user's own Sheaf data, as it is before the run (the check at the end compares).
real_profile() { ls -la --time-style=full-iso "${XDG_DATA_HOME:-$HOME/.local/share}/Sheaf" 2>/dev/null | md5sum; }
REAL0=$(real_profile)

launch() {
    # A fresh profile: no kept signature, history or preferences from earlier runs.
    # Absolute paths: the dirs crate ignores relative XDG paths and would fall back to the real
    # profile (it did until 21/10: smoke runs wrote into ~/.local/share/Sheaf).
    local p="$PWD/$OUT/profile"
    XDG_CONFIG_HOME="$p/config" XDG_DATA_HOME="$p/data" XDG_CACHE_HOME="$p/cache" "$APP" "$@" >"$OUT/app.log" 2>&1 &
    APP_PID=$!
    for _ in $(seq 1 40); do W=$(xdotool search --name '^Sheaf$' 2>/dev/null | tail -1); [ -n "$W" ] && break; sleep 0.25; done
    [ -n "$W" ] || { echo "FAIL the app opened no window:"; tail -5 "$OUT/app.log"; exit 1; }
    xdotool windowsize "$W" 1280 800
    # Drawn and loaded: wait until the screen is no longer black, then a moment for the pages.
    for _ in $(seq 1 40); do
        m=$(import -window root -crop 600x300+400+300 png:- 2>/dev/null | convert - -format "%[fx:mean]" info: 2>/dev/null)
        python3 -c "import sys; sys.exit(0 if float('${m:-0}') > 0.2 else 1)" && break
        sleep 0.5
    done
    sleep 1
    xdotool windowfocus --sync "$W" 2>/dev/null
}
quit() { kill "$APP_PID" 2>/dev/null; wait "$APP_PID" 2>/dev/null; APP_PID=; }
# A held click (Slint misses instantaneous ones under Xvfb).
click() { xdotool mousemove "$1" "$2" mousedown 1; sleep 0.08; xdotool mouseup 1; sleep "${3:-0.5}"; }
# Keys go to the app window: focus it first (Xvfb has no window manager to keep it).
key() { xdotool windowfocus --sync "$W" 2>/dev/null; xdotool key "$@"; sleep 0.4; }
# ⇧ + a letter, ⇧ held first: xdotool's "shift+t" can deliver the letter before the app has seen ⇧
# (flaky ⇧T on 21/10); a hand on a keyboard never does.
shifted() { xdotool windowfocus --sync "$W" 2>/dev/null; xdotool keydown shift; sleep 0.1; xdotool key "$1"; xdotool keyup shift; sleep 0.4; }
# Markup, notes, ink and text boxes in the file as it is now (qdf rewrites only live objects).
annots() { qpdf --qdf --object-streams=disable "$1" - 2>/dev/null | grep -ac '/Subtype /\(Highlight\|Underline\|StrikeOut\|Text\|Ink\|FreeText\)$'; }
npages() { qpdf --show-npages "$1"; }
# The Rects of the ink annotations (signatures are drawn as ink), "x0 y0 x1 y1" joined by ";".
inks() { qpdf --qdf --object-streams=disable "$1" - 2>/dev/null | python3 -c '
import re, sys
d = sys.stdin.buffer.read().decode("latin1")
print(";".join(" ".join(m.group(1).split()) for m in re.finditer(r"/Rect \[([-\d.\s]+)\][^>]*?/Subtype /Ink", d)))'; }
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
key n; click 1000 400 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "smoke note"; key ctrl+Return; sleep 1
key v; key ctrl+s; sleep 1.5
check "a note is saved" "[ \"\$(annots $OUT/text.pdf)\" -ge 2 ]"
check "annotations carry the author (login name by default)" "qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -aq \"/T ($USER)\""
key ctrl+z; sleep 0.8; key ctrl+s; sleep 1.5
A_UNDO=$(annots "$OUT/text.pdf")
key ctrl+shift+z; sleep 0.8; key ctrl+s; sleep 1.5
A_REDO=$(annots "$OUT/text.pdf")
check "undo then redo restores the note ($A_UNDO → $A_REDO)" "[ $A_REDO -gt $A_UNDO ]"

# 2a. Drag the note down 100 px: its /Rect moves in the saved file.
note_y() { qpdf --qdf --object-streams=disable "$1" - 2>/dev/null | python3 -c "
import re,sys
t=sys.stdin.buffer.read().decode('latin-1')
m=[x for x in re.finditer(r'/Rect \[\s*([0-9.-]+)\s+([0-9.-]+)', t) if '/Subtype /Text\n' in t[x.end():x.end()+200]]
print(m[0].group(2) if m else '')"; }
Y0=$(note_y "$OUT/text.pdf")
key Escape; key v
xdotool mousemove 1006 406 mousedown 1; sleep 0.1; for y in 420 450 480 506; do xdotool mousemove 1006 $y; sleep 0.05; done; xdotool mouseup 1; sleep 1
key ctrl+s; sleep 1.5
Y1=$(note_y "$OUT/text.pdf")
check "dragging a note moves it ($Y0 → $Y1)" "[ -n \"$Y1\" ] && python3 -c 'import sys; sys.exit(0 if float(\"$Y0\") - float(\"$Y1\") > 30 else 1)'"

# 2b. Signature: S, click → the pad opens; draw, Use → placed; the next click places it again.
key Escape; key s; click 700 500 1
xdotool mousemove 520 420 mousedown 1; for i in $(seq 0 20); do xdotool mousemove $((520 + i * 12)) $((420 + (i % 3) * 8)); done; xdotool mouseup 1; sleep 0.3
shot 02b-signature-pad
A0=$(annots "$OUT/text.pdf")
# The pad's Use button: bottom right of the centred dialog.
click 818 536 1.2
key ctrl+s; sleep 1.5
A1=$(annots "$OUT/text.pdf")
check "a drawn signature is placed and saved ($A0 → $A1)" "[ $A1 -gt $A0 ]"
click 700 650 1.2; key ctrl+s; sleep 1.5
A2=$(annots "$OUT/text.pdf")
check "the kept signature is placed again with one click ($A1 → $A2)" "[ $A2 -gt $A1 ]"
key Escape; key v

# 2b'. Select that signature, pull its corner handle: it grows, keeping its proportions and top-left.
# Page 1 at ⌘9 in the 1280×800 window: screen = (258.5 + 1.651·x, 1401 − 1.651·y) for PDF (x, y).
R0=$(inks "$OUT/text.pdf"); click 700 651 0.8
set -- $(python3 -c '
rs = [list(map(float, r.split())) for r in "'"$R0"'".split(";")]
sx = lambda x: 258.5 + 1.651 * x; sy = lambda y: 1401 - 1.651 * y
r = min(rs, key=lambda r: abs(sx((r[0] + r[2]) / 2) - 700) + abs(sy((r[1] + r[3]) / 2) - 651))
print(round(sx(r[2])), round(sy(r[1])))')
HX=$1; HY=$2
xdotool mousemove $HX $HY mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $((HX + i * 20)) $((HY + i * 8)); sleep 0.05; done; xdotool mouseup 1; sleep 1.5
key ctrl+s; sleep 1.5; shot 02c-signature-resized
R1=$(inks "$OUT/text.pdf")
check "resizing a signature scales it from its top-left" "python3 -c '
before, after = [set(s.split(\";\")) for s in (\"$R0\", \"$R1\")]
gone, new = before - after, after - before
assert len(gone) == 1 and len(new) == 1, (gone, new)
a, b = [list(map(float, r.pop().split())) for r in (gone, new)]
print(\"   \", a, \"→\", b)
wa, wb, ha, hb = a[2] - a[0], b[2] - b[0], a[3] - a[1], b[3] - b[1]
# The ink scales; its pen width does not, so the height grows a little less than the width.
ok = wb > wa * 1.2 and 1.1 < hb / ha <= wb / wa + 0.01 and abs(a[0] - b[0]) < 3 and abs(a[3] - b[3]) < 3
raise SystemExit(0 if ok else 1)'"
# 2b''. Click that signature again, Delete: it is gone from the saved file.
set -- $(python3 -c '
new = set("'"$R1"'".split(";")) - set("'"$R0"'".split(";"))
r = list(map(float, new.pop().split()))
print(round(258.5 + 1.651 * (r[0] + 12)), round(1401 - 1.651 * (r[1] + r[3]) / 2))')
key Escape; click $1 $2 0.8; key Delete; sleep 1; key ctrl+s; sleep 1.5
R2=$(inks "$OUT/text.pdf")
check "a selected signature is deleted with the Delete key" "[ \$(echo \"$R2\" | tr ';' '\\n' | grep -c .) -eq \$(( \$(echo \"$R1\" | tr ';' '\\n' | grep -c .) - 1 )) ]"
key Escape

# 2c. Edit text: E, click empty space, type, ⌘↩ → new text in the page content.
key e; sleep 0.8; click 640 600 0.8
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Added by smoke"; key ctrl+Return; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5
check "text added with the Edit tool is in the page content" "pdftotext -f 1 -l 1 $OUT/text.pdf - | grep -q 'Added by smoke'"
# The format row: Serif, bold, two sizes up → the page gains Times-Bold.
key e; sleep 0.8; click 360 110 0.3; click 470 110 0.3; click 578 110 0.3; click 578 110 0.3
click 640 680 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Serif bold"; key ctrl+Return; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5; shot 02c-text-format
check "added text takes the chosen font (Times-Bold)" "pdffonts -f 1 -l 1 $OUT/text.pdf 2>/dev/null | grep -q 'Times-Bold'"

# 2d. Edit tool: drag the first paragraph of page 1 down; its words move down in the saved file.
word_y() { pdftotext -f 1 -l 1 -bbox "$1" - 2>/dev/null | grep -m1 '>quick<' | sed 's/.*yMin="\([0-9.]*\)".*/\1/'; }
QY0=$(word_y "$OUT/text.pdf")
key e; sleep 1.2
xdotool mousemove 500 200 mousedown 1; sleep 0.1; for y in 220 260 300 340; do xdotool mousemove 500 $y; sleep 0.05; done; xdotool mouseup 1; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5; shot 02d-paragraph-moved
QY1=$(word_y "$OUT/text.pdf")
check "dragging a paragraph with the Edit tool moves it ($QY0 → $QY1)" "[ -n \"$QY1\" ] && python3 -c 'import sys; sys.exit(0 if float(\"$QY1\") - float(\"$QY0\") > 30 else 1)'"

# 3. Page operations from the Pages panel: duplicate page 1, delete it again, rotate.
click 120 230; click 142 778 1.2      # select page 1, Duplicate
key ctrl+s; sleep 1.5
check "duplicate adds a page" "[ \"\$(npages $OUT/text.pdf)\" = 5 ]"
click 120 230; click 203 778 1.2      # Delete
key ctrl+s; sleep 1.5
check "delete removes it" "[ \"\$(npages $OUT/text.pdf)\" = 4 ]"
click 120 230; click 108 778 1.2      # Insert blank page after page 1
key ctrl+s; sleep 1.5
check "insert blank adds an empty page 2" "[ \"\$(npages $OUT/text.pdf)\" = 5 ] && [ -z \"\$(pdftotext -f 2 -l 2 $OUT/text.pdf - | tr -d '[:space:]')\" ]"
key ctrl+z; key ctrl+s; sleep 1.5
check "undo removes the blank page" "[ \"\$(npages $OUT/text.pdf)\" = 4 ]"
key ctrl+z; key ctrl+s; sleep 1.5
check "undo brings the deleted page back" "[ \"\$(npages $OUT/text.pdf)\" = 5 ]"
key ctrl+z; key ctrl+s; sleep 1.5
check "undo again removes the duplicate" "[ \"\$(npages $OUT/text.pdf)\" = 4 ]"
# ⌘A selects the page's text, ⌘C copies it.
key Escape; key v; echo -n | xclip -selection clipboard >/dev/null 2>&1; key ctrl+a; key ctrl+c; sleep 0.3
check "⌘A then ⌘C copies the page's text" "xclip -o -selection clipboard 2>/dev/null | grep -q 'the quick brown fox'"
# ⇧T hides the toolbar for reading and shows it again.
bar() { import -window root -crop 1280x36+0+44 png:- 2>/dev/null | convert - -format "%[fx:standard_deviation]" info:; }
# The redraw can lag behind renders still running: wait for the bar to change (or come back), up to 3 s.
bar_until() { local v; for _ in $(seq 1 12); do v=$(bar); python3 -c "import sys; sys.exit(0 if ($1) else 1)" "$v" 2>/dev/null && break; sleep 0.25; done; echo "$v"; }
B0=$(bar); shifted t; B1=$(bar_until "abs(float(sys.argv[1]) - $B0) > 0.01"); shifted t; B2=$(bar_until "abs(float(sys.argv[1]) - $B0) < 0.005")
check "⇧T hides the toolbar and brings it back ($B0 → $B1 → $B2)" "python3 -c 'import sys; a, b, c = $B0, $B1, $B2; sys.exit(0 if abs(a - b) > 0.01 and abs(a - c) < 0.005 else 1)'"
shot 03-pages

# 4. Search runs (results panel in the screenshot).
click 1030 61; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "fox"; key Return; sleep 1.5; shot 04-search
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
click 640 300 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Smoke Test"; key Return; sleep 1
# 6b. Print before saving (⌘P, Return): the printed copy has the unsaved value. Needs a CUPS
# printer named PDF (printer-driver-cups-pdf, output in ~/PDF); skipped without one.
if lpstat -e 2>/dev/null | grep -qx PDF && lpstat -d 2>/dev/null | grep -q ': PDF$'; then
    N0=$(ls ~/PDF/*.pdf 2>/dev/null | wc -l)
    key ctrl+p; sleep 0.8; shot 06b-print-sheet; key Return
    for _ in $(seq 1 20); do [ "$(ls ~/PDF/*.pdf 2>/dev/null | wc -l)" -gt "$N0" ] && break; sleep 0.5; done; sleep 0.5
    P=$(ls -t ~/PDF/*.pdf 2>/dev/null | head -1)
    check "printing sends the document with its unsaved edits" "[ -n \"$P\" ] && [ \"\$(ls ~/PDF/*.pdf | wc -l)\" -gt $N0 ] && pdftotext \"$P\" - | grep -q 'Smoke Test'"
    [ -n "$P" ] && rm -f "$P"
else
    echo "SKIP printing (no default CUPS printer named PDF)"
fi
key ctrl+s; sleep 1.5; shot 06-form
V=$(qpdf --json=2 --json-key=acroform "$OUT/form.pdf" | python3 -c "import json,sys;print([f.get('value') for f in json.load(sys.stdin)['acroform']['fields']])")
check "a form field is filled and saved ($V)" "echo \"$V\" | grep -q 'Smoke Test'"
quit

# 6c. Shapes (R): a rectangle; an oval, filled, at 75 %; an arrow. Options bar at 1280×800:
# oval (317,109), arrow (376,109), Fill (413,109), opacity (550,109).
launch "$OUT/shapes.pdf"; key ctrl+9
drag() { xdotool mousemove $1 $2 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(($1 + i * $3 / 5)) $(($2 + i * $4 / 5)); sleep 0.05; done; xdotool mouseup 1; sleep 1.2; }
key r; drag 500 300 180 120
click 317 109; click 413 109; click 550 109; drag 500 600 200 100
click 376 109; drag 900 300 150 -100
key ctrl+s; sleep 1.5; shot 06c-shapes
SH=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null)
check "a rectangle, a filled oval at 75 % and an arrow are saved" "echo \"\$SH\" | grep -aq '/Subtype /Square' && echo \"\$SH\" | grep -aq '/Subtype /Circle' && echo \"\$SH\" | grep -aq '/IC \\[' && echo \"\$SH\" | grep -aq '/CA 0.75' && echo \"\$SH\" | grep -aq '/OpenArrow'"
quit

# 7. A password-protected PDF asks for its password, then opens in the reader.
qpdf --encrypt sesame sesame 256 -- "$OUT/text.pdf" "$OUT/locked.pdf"
launch "$OUT/locked.pdf"; sleep 1; shot 07-password
xdotool type --delay 20 "sesame"; xdotool key Return; sleep 2
key Escape; key v; echo -n | xclip -selection clipboard >/dev/null 2>&1; key ctrl+a; key ctrl+c; sleep 0.3
check "a protected PDF opens after its password" "xclip -o -selection clipboard 2>/dev/null | grep -q 'the quick brown fox'"
quit

# 8. Two documents open in two tabs; ⇧⌘[ switches to the other one.
launch "$OUT/text.pdf" "$OUT/form.pdf"; sleep 1
copied() { echo -n | xclip -selection clipboard >/dev/null 2>&1; key Escape; key v; key ctrl+a; key ctrl+c; sleep 0.3; xclip -o -selection clipboard 2>/dev/null; }
T1=$(copied); key ctrl+shift+bracketleft; sleep 0.8; T2=$(copied); shot 08-tabs
check "two files open in two tabs, ⇧⌘[ switches (${T1:0:12} → ${T2:0:12})" "[ -n \"$T1\" ] && [ -n \"$T2\" ] && [ \"$T1\" != \"$T2\" ]"
# 9. Home ▸ Preferences ▸ Author name: what is typed is kept.
click 50 21 0.8; click 90 192 0.8; click 700 610 0.5
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Smoke Author"; sleep 0.8; shot 09-preferences
check "the author name typed in Preferences is kept" "grep -rqs '\"author\": \"Smoke Author\"' $OUT/profile"
quit

check "the real user profile was not touched" "[ \"\$(real_profile)\" = \"$REAL0\" ]"
echo "---"
# Outputs are deleted on success (SMOKE_KEEP=1 keeps the screenshots, e.g. to file a proof).
if [ $fails -eq 0 ]; then echo "ALL PASS"; [ -n "${SMOKE_KEEP:-}" ] || rm -rf "$OUT"; else echo "$fails FAILED (screenshots in $OUT)"; fi
exit $fails
