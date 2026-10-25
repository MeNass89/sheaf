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
# -noreset: an X server resets when its last client leaves; an app launched during the reset
# cannot connect (intermittent "opened no window" after a quit, 21/10).
Xvfb :98 -screen 0 1280x800x24 -nolisten tcp -noreset >"$OUT/xvfb.log" 2>&1 &
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
# labels.pdf: text.pdf's pages numbered i, ii, then 1, 2 (/PageLabels)
objs[1] = "<< /Type /Catalog /Pages 3 0 R /PageLabels << /Nums [0 << /S /r >> 2 << /S /D >>] >> >>"
write("labels.pdf", objs, 2)
# links.pdf: a link to page 2 and a web link on page 1
c = "BT /F1 24 Tf 72 700 Td (Go to page 2) Tj 0 -60 Td (Visit example.org) Tj ET"
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", "<< /Type /Catalog /Pages 3 0 R >>",
        "<< /Type /Pages /Kids [5 0 R 7 0 R] /Count 2 >>", f"<< /Length {len(c)} >>\nstream\n{c}\nendstream",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 1 0 R >> >> /Annots [8 0 R 9 0 R] >>",
        "<< /Length 0 >>\nstream\n\nendstream", "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 6 0 R >>",
        "<< /Type /Annot /Subtype /Link /Rect [72 695 260 725] /Border [0 0 0] /Dest [7 0 R /XYZ 0 792 0] >>",
        "<< /Type /Annot /Subtype /Link /Rect [72 635 300 665] /Border [0 0 0] /A << /S /URI /URI (https://example.org/) >> >>"]
write("links.pdf", objs, 2)
# form.pdf: a text field "name" over the page, Helvetica
c = "BT /F1 18 Tf 72 720 Td (Name:) Tj ET"
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        "<< /Type /Catalog /Pages 3 0 R /AcroForm << /Fields [6 0 R] /NeedAppearances true /DA (/Helv 12 Tf 0 g) /DR << /Font << /Helv 1 0 R >> >> >> >>",
        "<< /Type /Pages /Kids [5 0 R] /Count 1 >>",
        f"<< /Length {len(c)} >>\nstream\n{c}\nendstream",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 1 0 R >> >> /Annots [6 0 R] >>",
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) /Rect [72 400 540 700] /P 5 0 R /DA (/Helv 12 Tf 0 g) /F 4 >>"]
write("form.pdf", objs, 2)
# image.pdf: an image 60×40 px, red left half and blue right half, drawn 150×100 pt at (200, 500),
# and a green one below it at (200, 200)
hexdata = ("FF0000" * 30 + "0000FF" * 30) * 40 + ">"
green = "00FF00" * 20 * 12 + ">"
c = "BT /F1 18 Tf 72 720 Td (An image below) Tj ET q 150 0 0 100 200 500 cm /Im1 Do Q q 100 0 0 60 200 200 cm /Im2 Do Q"
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", "<< /Type /Catalog /Pages 3 0 R >>",
        "<< /Type /Pages /Kids [5 0 R] /Count 1 >>", f"<< /Length {len(c)} >>\nstream\n{c}\nendstream",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 1 0 R >> /XObject << /Im1 6 0 R /Im2 7 0 R >> >> >>",
        f"<< /Type /XObject /Subtype /Image /Width 60 /Height 40 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length {len(hexdata)} >>\nstream\n{hexdata}\nendstream",
        f"<< /Type /XObject /Subtype /Image /Width 20 /Height 12 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length {len(green)} >>\nstream\n{green}\nendstream"]
write("image.pdf", objs, 2)
PY
for f in text form labels links image; do qpdf "$OUT/$f.pdf" "$OUT/$f.fixed.pdf" && mv "$OUT/$f.fixed.pdf" "$OUT/$f.pdf"; done
cp "$OUT/text.pdf" "$OUT/shapes.pdf"

# The user's own Sheaf data, as it is before the run (the check at the end compares).
real_profile() { ls -la --time-style=full-iso "${XDG_DATA_HOME:-$HOME/.local/share}/Sheaf" 2>/dev/null | md5sum; }
REAL0=$(real_profile)

# Toolbar controls reached by position (1280×800 window), kept here: update them when the toolbar
# changes (a capture of the toolbar shows where they went).
ZOOM_X=716; ZOOM_MENU_X=745; WRENCH_X=1222

launch() {
    # A fresh profile: no kept signature, history or preferences from earlier runs.
    # Absolute paths: the dirs crate ignores relative XDG paths and would fall back to the real
    # profile (it did until 21/10: smoke runs wrote into ~/.local/share/Sheaf).
    # Its own temp folder too: the app is killed, so its temp copies are not removed on exit.
    local p="$PWD/$OUT/profile"; mkdir -p "$p/tmp"
    TMPDIR="$p/tmp" XDG_CONFIG_HOME="$p/config" XDG_DATA_HOME="$p/data" XDG_CACHE_HOME="$p/cache" SHEAF_READER_DEBUG=1 "$APP" "$@" >>"$OUT/app.log" 2>&1 &
    APP_PID=$!
    for _ in $(seq 1 40); do W=$(xdotool search --name '^Sheaf$' 2>/dev/null | tail -1); [ -n "$W" ] && break; sleep 0.25; done
    [ -n "$W" ] || { echo "FAIL the app opened no window (Xvfb $(kill -0 $XVFB 2>/dev/null && echo alive || echo gone), socket $(ls /tmp/.X11-unix/X98 2>/dev/null || echo missing), $(ls /proc/$XVFB/fd 2>/dev/null | wc -l) fds, $(pgrep -c xclip) xclip):"; tail -3 "$OUT/app.log"; tail -5 "$OUT/xvfb.log"; exit 1; }
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
# The screen centre "x y" of the first word starting with $1 (OCR at 200 %, its box halved; OCR may glue
# punctuation to it) in the area $2 (WxH+X+Y, default the whole screen), empty when not seen.
at() { local g="${2:-1280x800+0+0}"; local ox=${g#*+}; ox=${ox%%+*}; local oy=${g##*+}; import -window root -crop "$g" png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' -v w="$1" -v ox="$ox" -v oy="$oy" 'index($12, w) == 1 { printf "%d %d\n", ox + ($7 + $9 / 2) / 2, oy + ($8 + $10 / 2) / 2; exit }'; }
# The screen "left right y" of the first word starting with $1 in the area $2 (as `at`), empty when not seen.
span() { local g="${2:-1280x800+0+0}"; local ox=${g#*+}; ox=${ox%%+*}; local oy=${g##*+}; import -window root -crop "$g" png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' -v w="$1" -v ox="$ox" -v oy="$oy" 'index($12, w) == 1 { printf "%d %d %d\n", ox + $7 / 2, ox + ($7 + $9) / 2, oy + ($8 + $10 / 2) / 2; exit }'; }
# Text on screen, read by tesseract (the tab bar and page titles have no other handle under Xvfb).
ocr() { import -window root -crop "$1" png:- 2>/dev/null | convert - -resize 200% png:- | tesseract stdin stdout 2>/dev/null | tr '\n' ' '; }
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
# Blue ink and the bold pen (the pad's first row), then Use (bottom right of the centred dialog).
click 477 314 0.3; click 610 314 0.3
click 818 555 1.2
key ctrl+s; sleep 1.5
A1=$(annots "$OUT/text.pdf")
check "a drawn signature is placed and saved ($A0 → $A1)" "[ $A1 -gt $A0 ]"
check "the signature is in the ink and pen chosen on the pad (blue, bold)" "qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -a -B30 PCFillSign | tr -s ' \\n' ' ' | grep -aq '/W 2.8 >> /C \\[ 0.1 0.32 0.85'"
click 700 650 1.2; key ctrl+s; sleep 1.5
A2=$(annots "$OUT/text.pdf")
check "the kept signature is placed again with one click ($A1 → $A2)" "[ $A2 -gt $A1 ]"
# New signature (412,109 in the Signature tool's bar) keeps the first: two in the list, the new one
# first; a click on the second thumbnail (382,109) brings the first back.
sigs() { python3 -c 'import json, sys; p = json.load(open(sys.argv[1])); print(len(p.get("signatures", [])), "".join(map(str, p.get("signature_pen", []))))' "$PWD/$OUT/profile/data/Sheaf/prefs.json"; }
S1=$(sigs); click 412 109 0.8; click 700 620 1
xdotool mousemove 520 400 mousedown 1; for i in $(seq 0 10); do xdotool mousemove $((520 + i * 10)) $((380 + i * 6)); done; xdotool mouseup 1; sleep 0.3
# The second one in black (the pad's first ink, 448,314); the first kept its blue.
click 448 314 0.3; click 818 555 1.2; S2=$(sigs); shot 02e-two-signatures; click 382 109 0.8; S3=$(sigs)
blue() { qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -a -B30 PCFillSign | tr -s ' \n' ' ' | grep -o '/C \[ 0.1 0.32 0.85' | wc -l; }
BL0=$(blue); click 760 700 1.2; key ctrl+s; sleep 1.5; BL1=$(blue)
check "a signature picked again is placed in its own ink (blue: $BL0 → $BL1)" "[ $BL1 -gt $BL0 ]"
key ctrl+z; key ctrl+s; sleep 1.5
check "a second signature is kept beside the first, and the first can be picked again ($S1 / $S2 / $S3)" "[ \"${S2%% *}\" = 2 ] && [ \"${S3#* }\" = \"${S1#* }\" ] && [ \"${S2#* }\" != \"${S1#* }\" ]"
key ctrl+z; key ctrl+s; sleep 1.5
# A typed signature: New signature, the name typed (field 640,512) shows in the four handwriting fonts; the
# second (Great Vibes, 742,506 once the previews push the pad up) is picked and Use (817,614) places it.
sfont() { python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("signature_font", 0))' "$PWD/$OUT/profile/data/Sheaf/prefs.json"; }
# "New signature" moves right as signatures are kept: found by OCR in the options bar; the name is typed
# only once the pad is open (else its letters would pick tools).
NS=$(at New 900x40+250+90); [ -n "$NS" ] && click $NS 0.8; click 700 620 1; click 640 512 0.3
PAD=$(at Create 600x400+340+200)
[ -n "$PAD" ] && { xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 20 "Ada Lovelace"; sleep 1.5; }; shot 02f-typed-fonts
[ -n "$PAD" ] && { click 742 506 0.4; click 817 614 1.2; key ctrl+s; sleep 1.5; }; SF=$(sfont)
check "a typed signature is placed in the handwriting font picked (font $SF, pad $PAD)" "[ \"$SF\" = 1 ] && qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -aq PCFillSign"
key ctrl+z; key ctrl+s; sleep 1.5
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
# The row: Sans 299, Serif 359, Mono 421, B 470, I 502, align left 534 / centre 568 / right 602,
# − 635, the size field 680, + 723 (y 110).
key e; sleep 0.8; click 359 110 0.3; click 470 110 0.3; click 723 110 0.3; click 723 110 0.3
click 640 680 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Serif bold"; key ctrl+Return; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5; shot 02c-text-format
check "added text takes the chosen font (Times-Bold)" "pdffonts -f 1 -l 1 $OUT/text.pdf 2>/dev/null | grep -q 'Times-Bold'"
# A text box in the same row's format plus italic (the row is shared): Times Bold Italic. Then
# selected (the row takes its format), Mono, aligned right, 30 pt typed: redrawn in Courier Bold
# Oblique, wrapped on two lines in a box grown to keep them, still one box.
key t; sleep 0.6; click 502 110 0.3; click 1000 520 0.8
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Boxtest in a font"; key ctrl+Return; sleep 1.2
key v; key ctrl+s; sleep 1.5; shot 02c2-text-box-font
check "a text box takes the format row's font (Times Bold Italic)" "qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -aq '/BaseFont /Times-BoldItalic'"
NB=$(qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -ac '/Subtype /FreeText$')
BX=$(at Boxtest); [ -n "$BX" ] && click $BX 0.8; click 421 110 1; click 602 110 1
click 680 110 0.3; key ctrl+a; xdotool type --delay 20 "30"; key Return; sleep 1.2
shot 02c3-text-box-restyled; click 1150 300 0.5; key ctrl+s; sleep 1.5
QB=$(qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null)
check "a selected text box takes a new font, alignment and a typed size, and grows ($BX)" "echo \"\$QB\" | grep -aq '/BaseFont /Courier-BoldOblique' && echo \"\$QB\" | grep -aq '/CoBO 30 Tf' && echo \"\$QB\" | grep -aq '/Q 2' && echo \"\$QB\" | grep -aq '(a font) Tj' && [ \$(echo \"\$QB\" | grep -ac '/Subtype /FreeText\$') -eq $NB ]"

# 2d. Edit tool: drag the first paragraph of page 1 down; its words move down in the saved file.
word_y() { pdftotext -f 1 -l 1 -bbox "$1" - 2>/dev/null | grep -m1 '>quick<' | sed 's/.*yMin="\([0-9.]*\)".*/\1/'; }
QY0=$(word_y "$OUT/text.pdf")
key e; sleep 1.2
xdotool mousemove 500 200 mousedown 1; sleep 0.1; for y in 220 260 300 340; do xdotool mousemove 500 $y; sleep 0.05; done; xdotool mouseup 1; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5; shot 02d-paragraph-moved
QY1=$(word_y "$OUT/text.pdf")
check "dragging a paragraph with the Edit tool moves it ($QY0 → $QY1)" "[ -n \"$QY1\" ] && python3 -c 'import sys; sys.exit(0 if float(\"$QY1\") - float(\"$QY0\") > 30 else 1)'"
# 2e. Editing that paragraph shows it on the page as typed, before ⌘↩; Escape puts it back and
# leaves the file as it was.
# "quick" looked for on the page only: the open thumbnails show it too, and a click there opened no editor.
key e; sleep 1; QK=$(at quick 1020x700+255+90); [ -n "$QK" ] && click $QK 1.2
xdotool windowfocus --sync "$W" 2>/dev/null; key ctrl+End; xdotool type --delay 10 " zebra"
# The preview is a render: waited for (up to 6 s), not slept for (1.5 s missed it on a busy machine).
for _ in $(seq 1 12); do sleep 0.5; PV1=$(ocr 1000x500+260+90); echo "$PV1" | grep -q zebra && break; done
shot 02e-preview; key Escape
for _ in $(seq 1 12); do sleep 0.5; PV2=$(ocr 1000x500+260+90); echo "$PV2" | grep -q zebra || break; done
key Escape; key v
check "an edited paragraph shows on the page before ⌘↩, Escape takes it back" "echo \"$PV1\" | grep -q zebra && ! echo \"$PV2\" | grep -q zebra && ! pdftotext $OUT/text.pdf - | grep -q zebra"

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
# Drag page 1's thumbnail (120,230) below page 2's (the drop line, a page badge under the pointer):
# page 2 comes first; ⌘Z puts it back.
xdotool mousemove 120 230 mousedown 1; for i in $(seq 1 12); do xdotool mousemove 120 $((230 + i * 25)); sleep 0.04; done; sleep 0.3; shot 03b-page-drag; xdotool mouseup 1; sleep 1.2
key ctrl+s; sleep 1.5; FP=$(first_text $OUT/text.pdf 1)
check "dragging a thumbnail reorders the pages ($FP)" "[ \"$FP\" = 'Page 2' ]"
key ctrl+z; key ctrl+s; sleep 1.5
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

# 4. Search (⌘F): "Page 3" is only on page 3; its first result is brought into view, which ⌘A ⌘C
# then reads back.
key Home; sleep 0.5; key ctrl+f; xdotool type --delay 10 "Page 3"; key Return; sleep 2; shot 04-search
echo -n | xclip -selection clipboard >/dev/null 2>&1; key Escape; key Escape; key v; key ctrl+a; key ctrl+c; sleep 0.3
SR=$(xclip -o -selection clipboard 2>/dev/null | head -1 | cut -c1-6)
check "search finds the text and shows its page ($SR)" "[ \"$SR\" = 'Page 3' ]"
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
    BEFORE_PDF=$(ls ~/PDF/*.pdf 2>/dev/null); N0=$(echo -n "$BEFORE_PDF" | grep -c .)
    key ctrl+p; sleep 0.8; shot 06b-print-sheet; key Return
    for _ in $(seq 1 20); do [ "$(ls ~/PDF/*.pdf 2>/dev/null | wc -l)" -gt "$N0" ] && break; sleep 0.5; done; sleep 0.5
    # The file this run printed: in ~/PDF now and not before (never one that was already there).
    P=$(comm -13 <(echo "$BEFORE_PDF" | sort) <(ls ~/PDF/*.pdf 2>/dev/null | sort) | head -1)
    check "printing sends the document with its unsaved edits" "[ -n \"$P\" ] && pdftotext \"$P\" - | grep -q 'Smoke Test'"
    [ -n "$P" ] && rm -f "$P"
else
    echo "SKIP printing (no default CUPS printer named PDF)"
fi
key ctrl+s; sleep 1.5; shot 06-form
V=$(qpdf --json=2 --json-key=acroform "$OUT/form.pdf" | python3 -c "import json,sys;print([f.get('value') for f in json.load(sys.stdin)['acroform']['fields']])")
check "a form field is filled and saved ($V)" "echo \"$V\" | grep -q 'Smoke Test'"
# Clear form asks first (Acrobat's Reset Form): Cancel keeps the value; Clear empties it.
CF=$(at Clear 1280x40+0+90)
[ -n "$CF" ] && click $CF 0.6; ASK=$(ocr 1280x40+0+90)
CN=$(at Cancel 1280x40+0+90); [ -n "$CN" ] && click $CN 0.6; key ctrl+s; sleep 1
V1=$(qpdf --json=2 --json-key=acroform "$OUT/form.pdf" | python3 -c "import json,sys;print([f.get('value') for f in json.load(sys.stdin)['acroform']['fields']])")
CF=$(at Clear 1280x40+0+90); [ -n "$CF" ] && click $CF 0.6
# The confirming Clear (white on the accent: OCR misses it) sits just left of Cancel.
CN=$(at Cancel 1280x40+0+90); OK=$([ -n "$CN" ] && echo "$CN" | awk '{ print $1 - 68, $2 }')
[ -n "$OK" ] && click $OK 0.6; key ctrl+s; sleep 1.5
V2=$(qpdf --json=2 --json-key=acroform "$OUT/form.pdf" | python3 -c "import json,sys;print([f.get('value') for f in json.load(sys.stdin)['acroform']['fields']])")
check "Clear form asks first; Cancel keeps the value, Clear empties it ($ASK; $V1 → $V2)" "echo \"$ASK\" | grep -q 'Clear every field' && echo \"$V1\" | grep -q 'Smoke Test' && ! echo \"$V2\" | grep -q 'Smoke Test'"
quit

# 6c. Shapes (R): a rectangle; an oval, filled, at 75 %; an arrow. Options bar at 1280×800:
# oval (317,109), arrow (376,109), Fill (413,109), opacity (550,109).
launch "$OUT/shapes.pdf"; key ctrl+9
drag() { xdotool mousemove $1 $2 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(($1 + i * $3 / 5)) $(($2 + i * $4 / 5)); sleep 0.05; done; xdotool mouseup 1; sleep 1.2; }
key r; drag 500 300 180 120
# ⇧ held at the press: a rectangle dragged 160 × 50 is saved square.
xdotool keydown shift; drag 260 650 160 50; xdotool keyup shift
click 317 109; click 413 109; click 550 109; drag 500 600 200 100
click 376 109; drag 900 300 150 -100
key ctrl+s; sleep 1.5; shot 06c-shapes
SH=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null)
check "a rectangle, a filled oval at 75 % and an arrow are saved" "echo \"\$SH\" | grep -aq '/Subtype /Square' && echo \"\$SH\" | grep -aq '/Subtype /Circle' && echo \"\$SH\" | grep -aq '/IC \\[' && echo \"\$SH\" | grep -aq '/CA 0.75' && echo \"\$SH\" | grep -aq '/OpenArrow'"
SQ=$(qpdf --json=2 "$OUT/shapes.pdf" | python3 -c 'import json, sys
o = json.load(sys.stdin)["qpdf"][1]
for v in o.values():
    d = v.get("value", {}) if isinstance(v, dict) else {}
    if d.get("/Subtype") == "/Square":
        r = d["/Rect"]; print(round(r[2] - r[0]), round(r[3] - r[1]))')
check "⇧ makes a rectangle square ($(echo $SQ))" "echo \"$SQ\" | awk '{ if (\$1 > 20 && (\$1 - \$2) * (\$1 - \$2) <= 4) ok = 1 } END { exit !ok }'"
check "the Shapes tool keeps its style for next time (opacity 75 % in the preferences)" "python3 -c 'import json, glob, sys; p = json.load(open(glob.glob(\"$PWD/$OUT/profile/data/Sheaf/prefs.json\")[0])); sys.exit(0 if any(t[0] == 10 and t[3] == 1 for t in p.get(\"tool_styles\", [])) else 1)'"
# 6c'. The eraser (X) along a short pen stroke rubs it out entirely.
ninks() { inks "$1" | tr ';' '\n' | grep -c .; }
key p; drag 600 720 40 0; key ctrl+s; sleep 1.5; N1=$(ninks "$OUT/shapes.pdf")
key x; drag 585 720 70 0; key ctrl+s; sleep 1.5; N2=$(ninks "$OUT/shapes.pdf")
check "the eraser rubs out a drawing it goes along ($N1 → $N2)" "[ $N1 -ge 1 ] && [ $N2 -eq \$(( N1 - 1 )) ]"
key v

# 6c''. Stamps (M): the default (Approved) with a click; a dated one (Received, seventh in the list at
# 317,330 once the list at 350,109 is open, after the four marks) says who and when.
key m; click 900 300 1.2; click 350 109 0.8; click 317 330 0.8; click 700 600 1.2
# A checkmark (first in the list, 317,150) placed beside them.
# In blue (the colour row shows for marks, 540,109): Acrobat's Fill & Sign marks take a colour.
click 350 109 0.8; click 317 150 0.8; click 540 109 0.5; click 400 650 1.2; key ctrl+s; sleep 1.5; key v
SH=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null)
check "stamps: Approved, and Received dated by the author" "echo \"\$SH\" | grep -aq '/Name /Approved' && echo \"\$SH\" | grep -aq '/PCByLine (By $USER at '"
check "a checkmark is placed (Fill & Sign mark)" "echo \"\$SH\" | grep -aq 'PCFillSign' && echo \"\$SH\" | grep -aq '(Checkmark)'"
MC=$(qpdf --json=2 "$OUT/shapes.pdf" | python3 -c 'import json, sys
for v in json.load(sys.stdin)["qpdf"][1].values():
    d = v.get("value", {}) if isinstance(v, dict) else {}
    if d.get("/Name") == "/PCCheck": print(d.get("/C"))')
check "the checkmark is in the colour picked (blue: $MC)" "echo \"$MC\" | grep -q '0.9'"

# 6c-notes. Notes panel (153,106): Hide all (45,149) hides the stamp at 900,300 and shows it again;
# the trash (226,152) removes every comment, in the saved file too.
click 153 106 0.8
stampshade() { import -window root -crop 200x40+800+280 png:- 2>/dev/null | convert - -format "%[fx:mean]" info:; }
H0=$(stampshade); click 45 149 1.2; H1=$(stampshade); click 45 149 1.2; H2=$(stampshade)
check "Hide all comments hides them and Show all brings them back ($H0 → $H1 → $H2)" "python3 -c 'import sys; a, b, c = $H0, $H1, $H2; sys.exit(0 if b - a > 0.05 and abs(a - c) < 0.005 else 1)'"
click 226 152 1.5; key ctrl+s; sleep 1.5
# The file must be read for the absence to mean anything.
QD=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null)
check "Remove all comments leaves none in the file" "[ -n \"\$QD\" ] && ! echo \"\$QD\" | grep -aqE '/Subtype /(Square|Circle|Line|Ink|Stamp|Text|FreeText|Highlight)\$'"
key ctrl+z; key ctrl+s; sleep 1.5
check "⌘Z brings them all back" "qpdf --qdf --object-streams=disable $OUT/shapes.pdf - 2>/dev/null | grep -aq '/Subtype /Stamp'"
# The filter (64,186 ▸ Stamps at 40,350) lists only stamps; deleting the first row deletes a stamp,
# not what sat at that row's place in the full list; back to All (40,230).
NST=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null | grep -ac '/Subtype /Stamp$')
click 64 186 0.8; click 40 350 1; click 120 240 0.8; key Delete; sleep 1; key ctrl+s; sleep 1.5
QD=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null)
check "Notes filter: the first row of Stamps deletes a stamp ($NST → one less), shapes kept" "[ \$(echo \"\$QD\" | grep -ac '/Subtype /Stamp\$') -eq \$(( NST - 1 )) ] && echo \"\$QD\" | grep -aq '/Subtype /Square\$'"
click 64 186 0.8; click 40 230 1
# Reply (52,270 on the selected first row): typed, ↩ sends it; the file holds it as a reply (/IRT).
click 120 240 1; click 52 270 1; xdotool windowfocus --sync "$W" 2>/dev/null; sleep 0.3; xdotool type --delay 20 "Smoke reply"; key Return; sleep 1; key ctrl+s; sleep 1.5
QD=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null)
check "a reply to a comment is saved as a reply" "echo \"\$QD\" | grep -aq '/IRT' && echo \"\$QD\" | grep -aq '(Smoke reply)'"
# Status (the list right of Reply, 150,292 once the reply shows ▸ Accepted, the second entry):
# saved as a status reply.
click 120 240 1; click 150 292 0.8; shot 06c-status; click 150 358 1; key ctrl+s; sleep 1.5
check "a review status is saved (Accepted)" "qpdf --qdf --object-streams=disable $OUT/shapes.pdf - 2>/dev/null | grep -aq '/State (Accepted)'"
key Escape
click 37 106 0.8   # back to the Pages panel

# 6c-props. ⌘D: Document Properties; a title typed and Return is saved into the file's Info.
key ctrl+d; sleep 0.5; xdotool key ctrl+a; xdotool type --delay 10 "Smoke Title"; key Return; sleep 1; key ctrl+s; sleep 1.5
check "Document Properties: the title typed is saved" "qpdf --qdf --object-streams=disable $OUT/shapes.pdf - 2>/dev/null | grep -aq '/Title (Smoke Title)'"

# 6c-outline. Outline panel (95,106): + Bookmark (55,149) bookmarks the page in view; saved.
click 95 106 0.8; click 55 149 1.2; key ctrl+s; sleep 1.5; shot 06c-outline
check "+ Bookmark adds the page in view to the outline" "qpdf --qdf --object-streams=disable $OUT/shapes.pdf - 2>/dev/null | grep -aq '/Title (Page 1)'"
# Double-click the bookmark (120,190) to rename it in place; ↩ keeps the new title.
xdotool mousemove 120 190 click --repeat 2 --delay 80 1; sleep 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool key ctrl+a; xdotool type --delay 20 "Renamed"; key Return; sleep 1; key ctrl+s; sleep 1.5; shot 06c-outline-renamed
check "a bookmark is renamed in place" "qpdf --qdf --object-streams=disable $OUT/shapes.pdf - 2>/dev/null | grep -aq '/Title (Renamed)'"
click 37 106 0.8

# 6d. Two pages side by side (⌥⌘2): the middle of the pages area shows the gap between
# the two pages instead of one page; two more clicks (cover alone, then one column) come back.
mid() { import -window root -crop 6x200+761+300 png:- 2>/dev/null | convert - -format "%[fx:mean]" info:; }
M1=$(mid); key ctrl+alt+2; sleep 2.5; M2=$(mid); key ctrl+alt+2; sleep 2.5; key ctrl+alt+2; sleep 2.5; M3=$(mid); shot 06d-spread
check "two pages side by side, then back to one column ($M1 → $M2 → $M3)" "python3 -c 'import sys; a, b, c = $M1, $M2, $M3; sys.exit(0 if abs(a - b) > 0.02 and abs(a - c) < 0.005 else 1)'"
# 6e. Keyboard paging: End, Home, → and Space; ⌘A ⌘C copies the page in view, which tells which it is.
inview() { echo -n | xclip -selection clipboard >/dev/null 2>&1; key Escape; key v; key ctrl+a; key ctrl+c; sleep 0.3; xclip -o -selection clipboard 2>/dev/null | head -1 | cut -c1-6; }
key End; sleep 1; P4=$(inview); key Home; sleep 1; P1=$(inview); key Right; sleep 1; P2=$(inview); key space; key space; sleep 1; P3=$(inview)
check "End, Home, → and Space move through the pages ($P4, $P1, $P2, $P3)" "[ \"$P4\" = 'Page 4' ] && [ \"$P1\" = 'Page 1' ] && [ \"$P2\" = 'Page 2' ] && [ \"$P3\" != 'Page 2' ]"
# Search for the selection (⇧⌘E): page 4's text selected, back to page 1, the search brings page 4 back.
key End; sleep 0.8; key ctrl+a; key Home; sleep 0.8; key ctrl+shift+e; sleep 2.5; SE=$(inview)
check "searching the selected text finds its page ($SE)" "[ \"$SE\" = 'Page 4' ]"
# With the toolbar hidden (⇧T), ⇧⌘E brings it back so the search field and results show.
key End; sleep 0.8; key ctrl+a; shifted t; sleep 0.5; key ctrl+shift+e; sleep 1.5; BE=$(bar)
check "searching the selection with the toolbar hidden shows it again ($BE)" "python3 -c 'import sys; sys.exit(0 if abs($BE - $B0) < 0.03 else 1)'"
key Escape
# 6g. The zoom field (ZOOM_X,61): 200 typed and ↩ covers the view's left edge with the page; Fit page
# from its list (ZOOM_MENU_X,61 ▸ 41 px left of it, 330) shows the background there again.
leftedge() { import -window root -crop 6x200+300+300 png:- 2>/dev/null | convert - -format "%[fx:mean]" info:; }
click $ZOOM_X 61 0.5; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool key ctrl+a; xdotool type --delay 20 "200"; key Return; sleep 2; Z1=$(leftedge)
click $ZOOM_MENU_X 61 0.6; click $((ZOOM_MENU_X - 41)) 330 2; Z2=$(leftedge)
check "zoom typed (200 %) and Fit page from the list ($Z1 → $Z2)" "python3 -c 'import sys; sys.exit(0 if $Z1 - $Z2 > 0.02 else 1)'"
key ctrl+9; sleep 1
# 6h. By hand: ⌘ + wheel zooms in around the pointer (the zoom field at ZOOM_X grows); a middle-button drag,
# then a drag with Space held, move the pages (from the top of page 1, where the text is); Space let go
# without a drag still turns the page.
pagesig() { import -window root -crop 1000x690+260+90 png:- 2>/dev/null | md5sum | cut -c1-8; }
# The zoom field read alone (a single line at 300 %: the default OCR misses it).
zoomval() { import -window root -crop 70x26+$((ZOOM_X - 32))+48 png:- 2>/dev/null | convert - -resize 300% png:- | tesseract stdin stdout --psm 7 2>/dev/null | tr -cd '0-9'; }
key Home; sleep 1; ZA=$(zoomval); QK=$(at fox 1020x700+255+90); # The pointer arrives as a hand moves it (two steps, a pause): a wheel event read before the move
# into the page was handled would anchor on where the pointer was before (961 → 889 on 25/10).
[ -n "$QK" ] && xdotool mousemove $(echo $QK | awk '{ print $1 - 6, $2 - 4 }') && sleep 0.15 && xdotool mousemove $QK && sleep 0.4; xdotool keydown ctrl click 4 click 4 keyup ctrl; sleep 2; ZB=$(zoomval); QK2=$(at fox 1020x700+255+90)
check "⌘ + wheel zooms in ($ZA → $ZB %)" "[ -n \"$ZA\" ] && [ -n \"$ZB\" ] && [ $ZB -gt $ZA ]"
# The word under the pointer stays under it (OCR centres within 60 px).
check "⌘ + wheel zooms around the pointer ($QK → $QK2)" "[ -n \"$QK\" ] && [ -n \"$QK2\" ] && python3 -c 'import sys; a, b = [list(map(int, v.split())) for v in sys.argv[1:]]; sys.exit(0 if abs(a[0] - b[0]) < 60 and abs(a[1] - b[1]) < 60 else 1)' \"$QK\" \"$QK2\""
PG0=$(pagesig); xdotool mousemove 700 520; sleep 0.4; xdotool mousedown 2; for i in $(seq 1 8); do xdotool mousemove $((700 - i * 8)) $((520 - i * 25)); sleep 0.03; done; xdotool mouseup 2; sleep 1; PG1=$(pagesig); shot 06h-pan
check "a middle-button drag moves the pages" "[ \"$PG0\" != \"$PG1\" ]"
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool keydown space; sleep 0.2; xdotool mousemove 700 300 mousedown 1; for i in $(seq 1 8); do xdotool mousemove 700 $((300 + i * 25)); sleep 0.03; done; xdotool mouseup 1; xdotool keyup space; sleep 1; PG2=$(pagesig)
check "a drag with Space held moves the pages" "[ \"$PG1\" != \"$PG2\" ]"
key ctrl+9; sleep 1

# 6f. Closed in two pages, it reopens in two pages (and on the same page).
key ctrl+alt+2; sleep 2.5; key ctrl+w; sleep 1; quit
launch "$OUT/shapes.pdf"; sleep 2; M4=$(mid)
check "a document reopens as it was left: two pages ($M4)" "python3 -c 'import sys; sys.exit(0 if abs($M4 - $M2) < 0.005 else 1)'"
quit

# 6h. Page labels: "1" typed in the page field (⌥⌘G) is the page labelled 1, the third.
launch "$OUT/labels.pdf"; key ctrl+9; sleep 0.5
key ctrl+alt+g; xdotool type --delay 20 "1"; key Return; sleep 1.2; PL=$(inview)
check "a page label typed in the page field goes to that page ($PL)" "[ \"$PL\" = 'Page 3' ]"
quit

# 6i. A link says where it goes under the pointer (a dark tooltip below it); away from it, nothing.
launch "$OUT/links.pdf"; key ctrl+9; sleep 0.5
under() { import -window root -crop 120x20+490+250 png:- 2>/dev/null | convert - -format "%[fx:mean]" info:; }
xdotool mousemove 900 600; sleep 0.6; U0=$(under); xdotool mousemove 480 235; sleep 0.2; xdotool mousemove 485 238; sleep 0.8; U1=$(under)
check "a link shows where it goes on hover ($U0 → $U1)" "python3 -c 'import sys; sys.exit(0 if $U0 - $U1 > 0.1 else 1)'"
# Scrolling under a still pointer takes the tooltip away (it would name a link no longer there).
key Down; key Down; sleep 0.8; U2=$(under)
check "the link tooltip goes when the page scrolls ($U1 → $U2)" "python3 -c 'import sys; sys.exit(0 if $U2 - $U1 > 0.1 else 1)'"
# ⇧⌘L tints every link (the first one, under 480,235 once back at the top); ⇧⌘P presents (no
# toolbar), Escape ends it.
key Home; sleep 0.6; xdotool mousemove 900 700; sleep 0.3
link() { import -window root -crop 80x16+400+228 png:- 2>/dev/null | convert - -format "%[fx:mean]" info:; }
L0=$(link); key ctrl+shift+l; sleep 0.6; L1=$(link); key ctrl+shift+l; sleep 0.4
check "Highlight links tints the links ($L0 → $L1)" "python3 -c 'import sys; sys.exit(0 if $L0 - $L1 > 0.005 else 1)'"
P0=$(bar); key ctrl+shift+p; sleep 1.2; P1=$(bar); key Escape; sleep 1; P2=$(bar); shot 06i-present
check "Presentation hides the toolbar, Escape brings it back ($P0 → $P1 → $P2)" "python3 -c 'import sys; a, b, c = $P0, $P1, $P2; sys.exit(0 if abs(a - b) > 0.01 and abs(a - c) < 0.005 else 1)'"
# Presenting from two pages, Space shows the next single page (links.pdf: page 2 is blank); hidden toolbar kept
# after plain full screen.
key ctrl+alt+2; sleep 1.5; key ctrl+shift+p; sleep 1.2; key space; sleep 1
# Read while still presenting (inview presses Escape, which would end it).
echo -n | xclip -selection clipboard >/dev/null 2>&1; key ctrl+a; key ctrl+c; sleep 0.3; PS=$(xclip -o -selection clipboard 2>/dev/null | head -1 | cut -c1-6)
key Escape; sleep 0.8; key ctrl+alt+2; key ctrl+alt+2; sleep 1
check "Space in a presentation turns to the next page ($PS)" "[ -z \"$PS\" ]"
shifted t; sleep 0.5; key ctrl+super+f; sleep 0.5; key Escape; sleep 0.8; PH=$(bar); shifted t; sleep 0.5
check "leaving full screen keeps the toolbar hidden ($PH)" "python3 -c 'import sys; sys.exit(0 if abs($PH - $P0) > 0.01 else 1)'"
quit

# 7. A password-protected PDF asks for its password, then opens in the reader.
qpdf --encrypt sesame sesame 256 -- "$OUT/text.pdf" "$OUT/locked.pdf"
# A wrong password first: the prompt stays, saying so; then the right one.
launch "$OUT/locked.pdf"; sleep 1; shot 07-password
# (Its small red line is read in grey at 300 %.)
xdotool type --delay 20 "wrong"; xdotool key Return; sleep 1
WP=$(import -window root -crop 500x200+390+300 png:- 2>/dev/null | convert - -colorspace Gray -resize 300% png:- | tesseract stdin stdout 2>/dev/null | tr '\n' ' ')
check "a wrong password keeps the prompt and says so" "echo \"$WP\" | grep -qi 'wrong password'"
xdotool type --delay 20 "sesame"; xdotool key Return; sleep 2
key Escape; key v; echo -n | xclip -selection clipboard >/dev/null 2>&1; key ctrl+a; key ctrl+c; sleep 0.3
check "a protected PDF opens after its password" "xclip -o -selection clipboard 2>/dev/null | grep -q 'the quick brown fox'"
quit
# Escape in the prompt skips the file, as Cancel would.
launch "$OUT/locked.pdf"; sleep 1; key Escape; sleep 0.8; EP=$(ocr 500x200+390+300)
check "Escape closes the password prompt" "kill -0 $APP_PID 2>/dev/null && ! echo \"$EP\" | grep -qi 'password protected'"
quit

# 8. Two documents open in two tabs; ⇧⌘[ switches to the other one.
launch "$OUT/text.pdf" "$OUT/form.pdf"; sleep 1
copied() { echo -n | xclip -selection clipboard >/dev/null 2>&1; key Escape; key v; key ctrl+a; key ctrl+c; sleep 0.3; xclip -o -selection clipboard 2>/dev/null; }
T1=$(copied); key ctrl+shift+bracketleft; sleep 0.8; T2=$(copied); shot 08-tabs
check "two files open in two tabs, ⇧⌘[ switches (${T1:0:12} → ${T2:0:12})" "[ -n \"$T1\" ] && [ -n \"$T2\" ] && [ \"$T1\" != \"$T2\" ]"
# 9. Home ▸ Preferences ▸ Author name: what is typed is kept.
# The field sits 30 px below its "Author name" label.
click 50 21 0.8; click 90 192 0.8; AN=$(at Author 900x700+200+90); [ -n "$AN" ] && click $(echo $AN | awk '{ print $1 + 200, $2 + 30 }') 0.5
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Smoke Author"; sleep 0.8; shot 09-preferences
check "the author name typed in Preferences is kept" "grep -rqs '\"author\": \"Smoke Author\"' $OUT/profile"
# Tools search: + opens Tools with the search focused; "pdf to", ↓, ↩ opens the second result.

# + opens Home ▸ Tools with the search taking the keyboard: typed without clicking the field.
TB0=$(ocr 1280x30+0+6); click 90 112 0.8; click 90 72 0.5; click 90 112 0.8
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 20 "pdf to"; sleep 0.6; key Down; key Return; sleep 1.5; shot 09b-tool-search
TB1=$(ocr 1280x30+0+6)
# OCR drops spaces: compare without them.
check "tool search: 'pdf to', ↓, ↩ opens PDF to Word ($TB1)" "echo \"$TB1\" | tr -d ' ' | grep -qi 'PDFtoWord' && ! echo \"$TB0\" | tr -d ' ' | grep -qi 'PDFtoWord'"
# The star on Compress's card (shown on hover, at 233,318): saved as a favourite, a Favourites row on top.
# The search still holds "pdf to" (the page was not left): cleared with Escape in the field first.
click 40 21 0.8; click 90 112 0.8; click 990 76 0.4; key Escape; sleep 0.5; shot 09c-before-star; xdotool mousemove 270 350; sleep 0.4; click 233 318 1
check "starring a tool puts it in the preferences' favourites" "grep -rqs '\"compress\"' $OUT/profile/data/Sheaf/prefs.json"
# ⌘, opens Preferences from a tool tab and from a document (keys go up to the app).
key ctrl+comma; sleep 1; PT1=$(ocr 500x60+200+40)
click 230 21 1; key ctrl+comma; sleep 1; PT2=$(ocr 500x60+200+40)   # the text.pdf tab
check "⌘, opens Preferences from a tool tab and from a document ($PT1 / $PT2)" "echo \"$PT1\" | grep -q Preferences && echo \"$PT2\" | grep -q Preferences"
quit

# 10. A tool from the reader: the wrench (WRENCH_X,61) → Tools, "compress" typed in its search, ↩ → Compress PDF
# (1140,755); outputs go next to the original (set in the profile), the result screen shows.
cp "$OUT/form.pdf" "$OUT/tool.pdf"
mkdir -p "$PWD/$OUT/profile/data/Sheaf"
python3 -c 'import json, sys; p = sys.argv[1]; d = json.load(open(p)) if __import__("os").path.exists(p) else {}; d["output"] = "next_to_original"; json.dump(d, open(p, "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json"
launch "$OUT/tool.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "compress"; key Return; sleep 2; click 1140 755 4; shot 10-tool-result
check "a tool run from the reader writes its output (Compress)" "qpdf --check $OUT/tool-compressed.pdf >/dev/null 2>&1"
# The same star on Home ▸ Recents' Recent tools (Compress just ran: its card is the first, star at 233,266; starred above).
click 40 21 0.8; click 90 72 0.8; xdotool mousemove 270 300; sleep 0.4; shot 09d-recent-tools; click 233 266 1
check "unstarring from Home's Recent tools takes it out of the favourites" "! grep -qs '\"compress\"' $OUT/profile/data/Sheaf/prefs.json"
# Tasks History ▸ Run again on the Compress row: a new Compress tab holding tool.pdf.
H=$(at History); [ -n "$H" ] && click $H 1; RA=$(at again); [ -n "$RA" ] && click $RA 1.5; shot 10b-rerun
RR=$(ocr 960x700+20+60); RP=$(ocr 280x300+1000+40)   # the file area (below the tab bar, which names tool.pdf too), the options
check "Run again from Tasks History reopens Compress with its file ($H / $RA)" "[ -n \"$RA\" ] && echo \"$RR\" | grep -q 'tool.pdf' && echo \"$RP\" | grep -qi 'Recommended'"
# Add watermark from the reader tab (tool.pdf, 230,21): the preview of page 1, pinned above the options
# (1004–1252 × 106–336), turns red once "DRAFT" is typed (text field 1130,432) at 120 pt (size 1060,546) in red
# (1165,615).
red() { import -window root -crop 248x230+1004+106 png:- 2>/dev/null | convert - -fx '(r>0.6&&g<0.4&&b<0.4)?1:0' -format '%[fx:mean*10000]' info: | cut -d. -f1; }
click 230 21 1; click $WRENCH_X 61 1.5; xdotool type --delay 20 "watermark"; key Return; sleep 2.5; R0=$(red)
click 1130 432 0.4; xdotool type --delay 30 "DRAFT"; click 1060 546 0.4; xdotool type --delay 30 "120"; click 1165 615 0.3; sleep 2.5
shot 10c-watermark-preview; R1=$(red)
check "the watermark preview shows the mark as typed, in its colour ($R0 → $R1)" "[ \"\${R1:-0}\" -gt \$(( \${R0:-0} + 50 )) ]"
quit

# 11. A damaged file says so in one line and the app stays up; Preferences ▸ Dark and Light switch the window.
printf '%%PDF-1.7\nnot really a pdf\n' > "$OUT/broken.pdf"
# The toast (white on dark, bottom centre, 3 s) read inverted until it is seen (up to 8 s: opening may be slow).
# Only the inside of the pill (dark, whatever is behind it), inverted to black on white.
toastText() { import -window root -crop 380x26+450+742 png:- 2>/dev/null | convert - -negate -resize 300% png:- | tesseract stdin stdout --psm 7 2>/dev/null; }
launch "$OUT/broken.pdf"; BR=""; for i in $(seq 1 32); do sleep 0.25; BR="$BR $(toastText)"; echo "$BR" | grep -qi 'damaged' && break; done; shot 11-broken
check "a damaged PDF is refused in one line, the app stays open" "kill -0 $APP_PID 2>/dev/null && echo \"$BR\" | grep -qi 'damaged or not a PDF'"
lum() { import -window root -crop 40x40+600+700 png:- 2>/dev/null | convert - -colorspace Gray -format '%[fx:mean]' info:; }
PR=$(at Preferences 250x300+0+40); [ -n "$PR" ] && click $PR 1
DK=$(at Dark); [ -n "$DK" ] && click $DK 1.2; L1=$(lum); shot 11-dark
LT=$(at Light); [ -n "$LT" ] && click $LT 1.2; L2=$(lum)
check "Preferences ▸ Dark then Light switch the window ($L1 → $L2)" "python3 -c 'import sys; sys.exit(0 if $L1 < 0.3 and $L2 > 0.7 else 1)'"
quit

# 12. Twelve documents: the tabs narrow to share the bar, the active one keeps its name readable and
# its ×; a middle click on it closes it.
for i in $(seq -w 1 12); do cp "$OUT/form.pdf" "$OUT/m$i.pdf"; done
launch $(for i in $(seq -w 1 12); do echo "$OUT/m$i.pdf"; done); sleep 4; shot 12-many-tabs
# Tabs counted by their purple document icons along the bar (y = 15); the active tab, last opened, is the
# one before the + (1160,21), its name in full.
ntabs() { import -window root -crop 1280x1+0+15 png:- 2>/dev/null | convert - txt:- | python3 -c '
import re, sys
runs, inside = 0, False
for line in sys.stdin:
    m = re.search(r"\((\d+),(\d+),(\d+)", line)
    if m:
        r, g, b = map(int, m.groups()); p = r > 120 and b > 140 and g < 110
        runs += p and not inside; inside = p
print(runs)'; }
N0=$(ntabs); NM=$(ocr 150x36+1095+4 | tr -d ' '); xdotool mousemove 1160 21 click 2; sleep 1; N1=$(ntabs)
check "twelve tabs share the bar, the active keeps its name, a middle click closes it ($N0 → $N1, $NM)" "[ \"$N0\" = 12 ] && [ \"$N1\" = 11 ] && echo \"$NM\" | grep -q '12'"
quit

# 13. Four documents: the first, scrolled to its last page, is dragged two places right; the bar
# reorders and the document keeps its page (only the bar moves, not the tab's page). Its right-click
# menu then closes the others.
cp "$OUT/labels.pdf" "$OUT/first.pdf"; for n in second third fourth; do cp "$OUT/form.pdf" "$OUT/$n.pdf"; done
launch $OUT/first.pdf $OUT/second.pdf $OUT/third.pdf $OUT/fourth.pdf; sleep 4
read FX0 _ <<< "$(at first 1280x40+0+0)"; xdotool mousemove "${FX0:-0}" 21 click 1; sleep 1; key End; sleep 1; DP0=$(inview)
read FX0 _ <<< "$(at first 1280x40+0+0)"; read TX0 _ <<< "$(at third 1280x40+0+0)"
xdotool mousemove "${FX0:-0}" 21 mousedown 1
for i in $(seq 1 20); do xdotool mousemove $(( ${FX0:-0} + i * 18 )) 21; sleep 0.03; done
xdotool mouseup 1; sleep 0.8; shot 13-tab-drag
read FX1 _ <<< "$(at first 1280x40+0+0)"; read TX1 _ <<< "$(at third 1280x40+0+0)"
check "dragging a tab reorders the bar (first ${FX0:-?} < third ${TX0:-?} → first ${FX1:-?} > third ${TX1:-?})" "[ -n \"$FX1\" ] && [ -n \"$TX1\" ] && [ $FX0 -lt $TX0 ] && [ $FX1 -gt $TX1 ]"
DP1=$(inview)
check "a dragged document tab keeps its page ($DP0 → $DP1)" "[ \"$DP0\" = 'Page 4' ] && [ \"$DP1\" = 'Page 4' ]"
xdotool mousemove "${TX1:-0}" 21 click 3; sleep 0.8
read OX OY <<< "$(at Other 1280x300+0+0)"; xdotool mousemove "${OX:-0}" "${OY:-0}" click 1; sleep 1
N2=$(ntabs); TB=$(ocr 1280x40+0+0)
check "the tab menu's Close Other Tabs leaves only that one ($N2: $TB)" "[ \"$N2\" = 1 ] && echo \"$TB\" | grep -q third"
quit

# 14. Page images: the Edit tool selects the first (red | blue), its bottom-right corner pulled
# out makes it bigger (proportions kept); the bar flips it (red goes right) and turns it right
# (red goes down); Delete removes it and only it (the green one stays). Measured on the saved
# file, at 72 dpi.
colourbox() { convert - -fuzz 12% -fill white +opaque "$1" -trim -format '%w %h %X %Y' info: 2>/dev/null; }
saved() { pdftoppm -r 72 -f 1 -l 1 -png "$OUT/image.pdf" 2>/dev/null | colourbox "$1"; }
# The centre of a box "W H +X +Y" as "x y".
centre() { echo "$1" | awk '{ gsub(/\+/, ""); print $3 + $1 / 2, $4 + $2 / 2 }'; }
launch "$OUT/image.pdf"; key e; sleep 1.2
# The first image found on screen below the options bar (whose red buttons would count too): red
# and blue together.
SHOT=$(mktemp); import -window root -crop 1000x640+260+140 +repage "png:$SHOT" 2>/dev/null
read RW RH RX RY <<< "$(colourbox '#ff0000' < "$SHOT")"; read BW BH BX BY <<< "$(colourbox '#0000ff' < "$SHOT")"; rm -f "$SHOT"
IX=$(( ${RX#+} + 260 )); IY=$(( ${RY#+} + 140 )); IW=$(( ${BX#+} + BW - ${RX#+} )); IH=$RH
S0=$(saved '#ff0000')
click $((IX + IW / 2)) $((IY + IH / 2)) 0.6
xdotool mousemove $((IX + IW - 1)) $((IY + IH - 1)) mousedown 1; for i in 1 2 3 4 5 6; do xdotool mousemove $((IX + IW - 1 + i * 12)) $((IY + IH - 1 + i * 8)); sleep 0.05; done; xdotool mouseup 1; sleep 1.2
shot 14-image-resized; key ctrl+s; sleep 1.5; S1=$(saved '#ff0000')
check "a page image's corner pulled out makes it bigger, proportions kept (red half ${S0%% +*} → ${S1%% +*})" "python3 -c '
import sys
a, b = [list(map(int, v.split()[:2])) for v in sys.argv[1:]]
sys.exit(0 if b[0] > a[0] + 15 and abs(b[0] / b[1] - a[0] / a[1]) < 0.05 else 1)' \"$S0\" \"$S1\""
# The image bar: rotate left, rotate right, flip, replace, delete at x 287, 322, 355, 388, 423 (y 110).
click 355 110 1.2; key ctrl+s; sleep 1.5; FR=$(centre "$(saved '#ff0000')"); FB=$(centre "$(saved '#0000ff')")
check "Flip puts the red half on the right ($FR | $FB)" "[ \"\${FR%% *}\" ] && python3 -c 'import sys; sys.exit(0 if float(sys.argv[1]) > float(sys.argv[2]) + 20 else 1)' \${FR%% *} \${FB%% *}"
click 322 110 1.2; key ctrl+s; sleep 1.5; TR=$(centre "$(saved '#ff0000')"); TB=$(centre "$(saved '#0000ff')")
check "Rotate right turns it a quarter clockwise: red (right) goes down ($TR / $TB)" "[ \"\${TR#* }\" ] && python3 -c 'import sys; sys.exit(0 if float(sys.argv[1]) > float(sys.argv[2]) + 20 else 1)' \${TR#* } \${TB#* }"
key Delete; sleep 1; key ctrl+s; sleep 1.5; S2=$(saved '#ff0000'); G2=$(saved '#00ff00')
check "Delete removes the selected image and only it (red: $S2, green: ${G2%% +*})" "! echo \"$S2\" | grep -qE '^[0-9]{2,}' && echo \"$G2\" | grep -qE '^[0-9]{2,} [0-9]{2,}'"
# ⌘V of an image on the clipboard (magenta, 120×60 px): placed on the page, selected (Delete
# removes it again).
convert -size 120x60 xc:'#ff00ff' "$OUT/magenta.png"; xclip -selection clipboard -t image/png -i "$OUT/magenta.png" & CLIP=$!
sleep 0.5; key ctrl+v; sleep 1.5; key ctrl+s; sleep 1.5; M1=$(saved '#ff00ff')
key Delete; sleep 1; key ctrl+s; sleep 1.5; M2=$(saved '#ff00ff'); kill $CLIP 2>/dev/null
check "⌘V places the image on the clipboard, selected (${M1%% +*}, then Delete: ${M2%% +*})" "echo \"$M1\" | grep -qE '^1[12][0-9] (5[0-9]|6[0-9]) ' && ! echo \"$M2\" | grep -qE '^[0-9]{2,}'"
quit

# 15. Word files opened from outside (Finder, command line) open Word to PDF with them: each card
# shows page 1 and its page count once LibreOffice has read it; the order button sorts them A to Z;
# converted next to the originals (set in step 10), headings as bookmarks. Needs LibreOffice.
if command -v soffice >/dev/null; then
    cat > "$OUT/headed.fodt" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
<office:body><office:text><text:h text:outline-level="1">Chapter One</text:h><text:p>Intro.</text:p><text:h text:outline-level="1">Chapter Two</text:h><text:p>More.</text:p></office:text></office:body></office:document>
XML
    HOME="$PWD/$OUT/profile" soffice --headless --norestore --convert-to docx --outdir "$OUT" "$OUT/headed.fodt" >/dev/null 2>&1
    mkdir -p "$OUT/office"; for n in "file 10" "file 2" abc; do cp "$OUT/headed.docx" "$OUT/office/$n.docx"; done
    launch "$OUT/office/file 10.docx" "$OUT/office/file 2.docx" "$OUT/office/abc.docx"
    for _ in $(seq 1 40); do [ "$(ocr 980x200+20+150 | grep -o '1 page' | wc -l)" -ge 3 ] && break; sleep 0.5; done
    OT=$(ocr 400x30+100+8); OP=$(ocr 980x200+20+150 | grep -o '1 page' | wc -l); shot 15-office-cards
    read A0 _ <<< "$(at abc 980x260+20+40)"; read T0 _ <<< "$(at 10.docx 980x260+20+40)"
    click 928 150 1; read A1 _ <<< "$(at abc 980x260+20+40)"; read T1 _ <<< "$(at 10.docx 980x260+20+40)"
    check "Word files opened from outside open Word to PDF, each with its page count ($OT, $OP)" "echo \"$OT\" | grep -q 'Word to PDF' && [ $OP -ge 3 ]"
    check "the order button sorts the files A to Z (abc ${A0:-?} > file 10 ${T0:-?} → abc ${A1:-?} < file 10 ${T1:-?})" "[ -n \"$A1\" ] && [ -n \"$T1\" ] && [ $A0 -gt $T0 ] && [ $A1 -lt $T1 ]"
    click 1133 755 1; for _ in $(seq 1 60); do [ -f "$OUT/office/file 10.pdf" ] && [ -f "$OUT/office/abc.pdf" ] && break; sleep 0.5; done; sleep 1
    check "converted next to the originals, headings as bookmarks" "[ \$(ls \"$OUT\"/office/*.pdf | wc -l) -eq 3 ] && qpdf --qdf --object-streams=disable \"$OUT/office/abc.pdf\" - 2>/dev/null | grep -aq '/Outlines'"
    quit
else
    echo "SKIP 15 (Office to PDF): LibreOffice not installed"
fi

# 16. Images opened from outside open JPG to PDF. US Letter (1130,260), Landscape (1188,341), "Merge
# all images" unticked (1012,453): one Letter landscape PDF next to each image (set in step 10).
mkdir -p "$OUT/img"; convert -size 400x300 xc:'#3366cc' "$OUT/img/a.png"; convert -size 300x400 xc:'#cc6633' "$OUT/img/b.png"
launch "$OUT/img/a.png" "$OUT/img/b.png"; JT=$(ocr 400x30+100+8)
click 1130 260 0.5; click 1188 341 0.4; click 1012 453 0.4; shot 16-jpg-options; click 1133 755 1
for _ in $(seq 1 20); do [ -f "$OUT/img/a.pdf" ] && [ -f "$OUT/img/b.pdf" ] && break; sleep 0.5; done
JA=$(pdfinfo "$OUT/img/a.pdf" 2>/dev/null | grep -o '[0-9]* x [0-9]* pts'); JB=$(pdfinfo "$OUT/img/b.pdf" 2>/dev/null | grep -o '[0-9]* x [0-9]* pts')
check "images opened from outside open JPG to PDF ($JT); unmerged, a US Letter landscape PDF each ($JA / $JB)" "echo \"$JT\" | grep -q 'JPG to PDF' && [ \"$JA\" = '792 x 612 pts' ] && [ \"$JB\" = '792 x 612 pts' ]"
quit

# 17. A scanned PDF (a page that is only a picture) taken from the reader to PDF to Word: the options
# say it looks scanned and to choose OCR (iLovePDF converts a scan's pages as pictures otherwise).
convert -size 600x800 xc:white -fill black -draw "rectangle 100,100 500,140" "$OUT/scan.png"; convert "$OUT/scan.png" "$OUT/scan.pdf"
launch "$OUT/scan.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "pdf to word"; key Return; sleep 3; shot 17-scanned
SC=$(ocr 280x700+1000+60 | tr -s ' ')
check "PDF to Word says a scanned PDF needs OCR ($SC)" "echo \"$SC\" | grep -qi 'looks scanned'"
quit

# 18. Redact by selecting text, as Acrobat: in the Redact tool, a drag that starts on a word marks the
# words it crosses (not a rectangle), shown as it goes; a double click marks a word ("Page"); applied,
# they are gone from the text, the rest of the line stays, and the result counts text passages.
cp "$OUT/text.pdf" "$OUT/redact.pdf"
launch "$OUT/redact.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "redact"; key Return; sleep 3
Q=$(span quick 820x700+140+60); F=$(span fox 820x700+140+60)
if [ -n "$Q" ] && [ -n "$F" ]; then
    # From the left edge of "quick" to the right edge of "fox" (a letter counts once the pointer passes its middle).
    set -- $Q; QX=$(( $1 + 1 )); QY=$3; set -- $F; FX=$(( $2 - 1 ))
    xdotool mousemove $QX $QY mousedown 1; for i in 1 2 3 4 5 6; do xdotool mousemove $(( QX + (FX - QX) * i / 6 )) $QY; sleep 0.05; done; xdotool mouseup 1; sleep 1
    P=$(at Page 820x700+140+60); [ -n "$P" ] && { set -- $P; xdotool mousemove $1 $2 click --repeat 2 --delay 120 1; sleep 1; }
    shot 18-redact-marked; RM=$(ocr 280x90+1000+525 | tr -s ' ')
    click 1140 755 1; for _ in $(seq 1 20); do [ -f "$OUT/redact-redacted.pdf" ] && break; sleep 0.5; done
    sleep 1; RS=$(ocr 700x40+100+720 | tr -s ' ')
fi
RT=$(pdftotext -f 1 -l 1 "$OUT/redact-redacted.pdf" - 2>/dev/null | tr -s ' \n' ' ')
check "Redact: a drag over words and a double click mark them ($RM), only they are removed ($RT), counted as text ($RS)" "echo \"$RM\" | grep -qi quick && echo \"$RT\" | grep -q '1 the' && ! echo \"$RT\" | grep -qE 'Page|quick|brown|fox' && echo \"$RS\" | grep -q '2 text passages'"
quit

# 19. Link tool (L), as PDF Expert's Edit ▸ Link: drag a box, type a page number, ↩; click the link with the
# tool, change it to an address, ↩; saved, the file has one /Link to https://example.org. Then the edit group's
# Redact button (509,61) opens the document in Redact PDF.
cp "$OUT/text.pdf" "$OUT/link.pdf"
launch "$OUT/link.pdf"; key ctrl+9; sleep 1; key l
xdotool mousemove 330 190 mousedown 1; for i in 1 2 3 4 5 6; do xdotool mousemove $((330 + i * 40)) $((190 + i * 4)); sleep 0.05; done; xdotool mouseup 1; sleep 1
xdotool type --delay 30 "2"; key Return; sleep 1.5
click 420 200 1; LE=$(ocr 320x130+300+200 | tr -s ' '); shot 19-link-existing
key ctrl+a; xdotool type --delay 20 "example.org"; key Return; sleep 1; key ctrl+s; sleep 2
LK=$(qpdf --qdf --object-streams=disable "$OUT/link.pdf" - 2>/dev/null | grep -ac "/Subtype /Link"); LU=$(qpdf --qdf --object-streams=disable "$OUT/link.pdf" - 2>/dev/null | grep -ao "/URI (https://example.org)")
check "Link tool: a box linked to page 2 ($LE), changed to an address, saved ($LK link, $LU)" "echo \"$LE\" | grep -q 'Remove link' && [ \"$LK\" = 1 ] && [ -n \"$LU\" ]"
click 509 61 3; RD=$(ocr 1280x40+0+0 | tr -s ' ')
check "the edit group's Redact button opens Redact PDF ($RD)" "echo \"$RD\" | grep -qE 'Redact ?PDF'"
quit

# 19b. A link from selected text (⌘K, as the selection bar's Link): "quick brown" linked to page 3, saved with
# its destination.
cp "$OUT/text.pdf" "$OUT/link2.pdf"
launch "$OUT/link2.pdf"; key ctrl+9; sleep 1
Q=$(span quick 1020x700+255+90); B=$(span brown 1020x700+255+90)
if [ -n "$Q" ] && [ -n "$B" ]; then
    set -- $Q; SX=$(( $1 + 1 )); SY=$3; set -- $B; EX=$(( $2 - 1 ))
    xdotool mousemove $SX $SY mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( SX + (EX - SX) * i / 5 )) $SY; sleep 0.05; done; xdotool mouseup 1; sleep 1
    key ctrl+k; sleep 1; xdotool type --delay 30 "3"; key Return; sleep 1.5; key ctrl+s; sleep 2
fi
LD=$(qpdf --qdf --object-streams=disable "$OUT/link2.pdf" - 2>/dev/null | grep -ac "/Dest")
check "a link from selected text, saved with its destination ($LD)" "[ \"$LD\" = 1 ]"
quit

# 20. Edit tool, several objects: ⇧-click two paragraphs, drag one (both move together), ⇧→ nudges both 10 pt
# right; saved, both words moved by the same amount.
cp /dev/null "$OUT/two.pdf"; python3 - "$OUT/two.pdf" <<'PY'
import sys
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", "<< /Type /Catalog /Pages 3 0 R >>", "<< /Type /Pages /Kids [5 0 R] /Count 1 >>"]
c = "BT /F1 24 Tf 72 700 Td (Alpha paragraph here) Tj ET BT /F1 24 Tf 300 450 Td (Beta block) Tj ET"
objs.append(f"<< /Length {len(c)} >>\nstream\n{c}\nendstream")
objs.append("<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 1 0 R >> >> >>")
b = b"%PDF-1.7\n"; offs = []
for i, o in enumerate(objs, 1):
    offs.append(len(b)); b += f"{i} 0 obj\n{o}\nendobj\n".encode()
x = len(b)
b += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode() + b"".join(f"{n:010d} 00000 n \n".encode() for n in offs)
b += f"trailer\n<< /Size {len(objs)+1} /Root 2 0 R >>\nstartxref\n{x}\n%%EOF\n".encode()
open(sys.argv[1], "wb").write(b)
PY
wpos() { pdftotext -bbox "$OUT/two.pdf" - 2>/dev/null | grep -m1 ">$1<" | sed 's/.*xMin="\([0-9.]*\)" yMin="\([0-9.]*\)".*/\1 \2/'; }
A0=$(wpos Alpha); B0=$(wpos Beta)
launch "$OUT/two.pdf"; key ctrl+9; sleep 1; key e; sleep 1.5
A=$(at Alpha 1020x700+255+90); B=$(at Beta 1020x700+255+90)
if [ -n "$A" ] && [ -n "$B" ]; then
    set -- $A; AX=$1; AY=$2; xdotool mousemove $AX $AY keydown shift click 1 keyup shift; sleep 0.6
    set -- $B; xdotool mousemove $1 $2 keydown shift click 1 keyup shift; sleep 0.8
    xdotool mousemove $AX $AY mousedown 1; for i in 1 2 3 4 5 6; do xdotool mousemove $AX $(( AY + i * 20 )); sleep 0.08; done; xdotool mouseup 1; sleep 2
    key shift+Right; sleep 2; shot 20-objects-moved; key ctrl+s; sleep 2
fi
A1=$(wpos Alpha); B1=$(wpos Beta)
check "⇧-click picks two paragraphs, a drag moves both, ⇧→ nudges both ($A0 → $A1; $B0 → $B1)" "python3 -c '
import sys
a0, a1, b0, b1 = [list(map(float, v.split())) for v in sys.argv[1:]]
da, db = (a1[0] - a0[0], a1[1] - a0[1]), (b1[0] - b0[0], b1[1] - b0[1])
sys.exit(0 if abs(da[0] - 10) < 1 and abs(db[0] - 10) < 1 and da[1] > 30 and abs(da[1] - db[1]) < 1 else 1)' \"$A0\" \"$A1\" \"$B0\" \"$B1\""
quit

# 20b. One picked paragraph: its right corner pulled 200 px left rewraps it to the new width; saved, its words
# stand on more lines ("here" below "Alpha").
key_y() { pdftotext -bbox "$OUT/para.pdf" - 2>/dev/null | grep -m1 ">$1<" | sed 's/.*yMin="\([0-9.]*\)".*/\1/'; }
cp "$OUT/two.pdf" "$OUT/para.pdf"
launch "$OUT/para.pdf"; key ctrl+9; sleep 1; key e; sleep 1.5
S=$(span here 1020x700+255+90); A=$(at Alpha 1020x700+255+90)
if [ -n "$S" ] && [ -n "$A" ]; then
    set -- $A; xdotool mousemove $1 $2 keydown shift click 1 keyup shift; sleep 0.8
    # The bottom-right corner: right of "here", 22 px below its middle (24 pt text at fit width).
    set -- $S; RX=$(( $2 + 2 )); RY=$(( $3 + 23 ))
    xdotool mousemove $RX $RY mousedown 1; for i in 1 2 3 4 5 6 7 8; do xdotool mousemove $(( RX - i * 25 )) $RY; sleep 0.08; done; xdotool mouseup 1; sleep 2
    shot 20b-rewrapped; key ctrl+s; sleep 2
fi
HA=$(key_y Alpha); HH=$(key_y here)
check "a picked paragraph's side pulled rewraps it (Alpha at $HA, here at $HH)" "[ -n \"$HH\" ] && python3 -c 'import sys; sys.exit(0 if float(\"$HH\") - float(\"$HA\") > 20 else 1)'"
quit

# 20c. A drawing is an object too: a blue square pressed and dragged 60 px down moves alone (rendered from the
# saved file: its blue pixels lower, the words where they were).
python3 - "$OUT/square.pdf" <<'PY'
import sys
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", "<< /Type /Catalog /Pages 3 0 R >>", "<< /Type /Pages /Kids [5 0 R] /Count 1 >>"]
c = "0 0 1 rg 400 600 60 60 re f 0 g BT /F1 24 Tf 72 700 Td (Alpha paragraph here) Tj ET"
objs.append(f"<< /Length {len(c)} >>\nstream\n{c}\nendstream")
objs.append("<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 1 0 R >> >> >>")
b = b"%PDF-1.7\n"; offs = []
for i, o in enumerate(objs, 1):
    offs.append(len(b)); b += f"{i} 0 obj\n{o}\nendobj\n".encode()
x = len(b)
b += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode() + b"".join(f"{n:010d} 00000 n \n".encode() for n in offs)
b += f"trailer\n<< /Size {len(objs)+1} /Root 2 0 R >>\nstartxref\n{x}\n%%EOF\n".encode()
open(sys.argv[1], "wb").write(b)
PY
blue_top() { pdftoppm -r 72 -png "$OUT/square.pdf" "$OUT/sq" 2>/dev/null; python3 -c "
import struct, zlib, sys
d = open('$OUT/sq-1.png', 'rb').read()
w, h = struct.unpack('>II', d[16:24]); raw = b''; i = 8
while i < len(d):
    n = struct.unpack('>I', d[i:i+4])[0]; t = d[i+4:i+8]
    if t == b'IDAT': raw += d[i+8:i+8+n]
    i += 12 + n
data = zlib.decompress(raw); bpp = 3 if d[25] == 2 else 4; stride = w * bpp + 1; rows = []; prev = bytearray(w * bpp)
for y in range(h):
    f = data[y*stride]; line = bytearray(data[y*stride+1:(y+1)*stride])
    for x in range(len(line)):
        a = line[x-bpp] if x >= bpp else 0; b = prev[x]; c = prev[x-bpp] if x >= bpp else 0
        if f == 1: line[x] = (line[x] + a) & 255
        elif f == 2: line[x] = (line[x] + b) & 255
        elif f == 3: line[x] = (line[x] + (a + b) // 2) & 255
        elif f == 4:
            p = a + b - c; pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
            line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
    rows.append(line); prev = line
print(min((y for y in range(h) for x in range(w) if rows[y][x*bpp] < 60 and rows[y][x*bpp+2] > 200), default=-1))
"; }
Y0=$(blue_top)
launch "$OUT/square.pdf"; key ctrl+9; sleep 1; key e; sleep 1.5
# The square's centre at fit width: 430,630 pt of a 612 pt page shown 1000 px wide from x 264.
SX=$(( 264 + 430 * 1000 / 612 )); SY=$(( 100 + (792 - 630) * 1000 / 612 ))
# Hovered, it is outlined (drawings are not otherwise); a plain press picks it, a drag moves it.
xdotool mousemove $(( SX - 200 )) $SY; sleep 0.3; xdotool mousemove $SX $SY; sleep 0.8; shot 20c-drawing-hover
xdotool mousemove $SX $SY mousedown 1; for i in 1 2 3 4 5 6; do xdotool mousemove $SX $(( SY + i * 10 )); sleep 0.08; done; xdotool mouseup 1; sleep 2
shot 20c-drawing-moved; key ctrl+s; sleep 2
Y1=$(blue_top); WA=$(pdftotext -bbox "$OUT/square.pdf" - 2>/dev/null | grep -m1 '>Alpha<' | sed 's/.*yMin="\([0-9.]*\)".*/\1/')
check "a drawing pressed and dragged moves alone (square top $Y0 → $Y1 px, Alpha at $WA)" "[ \"$Y0\" -ge 0 ] && [ \"$Y1\" -gt \"$(( Y0 + 20 ))\" ] && python3 -c 'import sys; sys.exit(0 if abs(float(\"$WA\") - 74.768) < 1 else 1)'"
quit

# 21. A tool's files reorder by drag (iLovePDF, Stirling): three images open in JPG to PDF, a.png's card dragged
# past c.png's (cards at 101/237/373, 130); the dragged card dims, a bar shows where it goes; the order becomes b c a.
mkdir -p "$OUT/reo"; convert -size 300x400 xc:'#3366cc' "$OUT/reo/a.png"; convert -size 300x400 xc:'#cc6633' "$OUT/reo/b.png"; convert -size 300x400 xc:'#33aa55' "$OUT/reo/c.png"
launch "$OUT/reo/a.png" "$OUT/reo/b.png" "$OUT/reo/c.png"; sleep 1
xdotool mousemove 101 130 mousedown 1; for i in 1 2 3 4 5 6 7 8 9 10; do xdotool mousemove $(( 101 + i * 35 )) 140; sleep 0.08; done; sleep 0.4; shot 21-dragging; xdotool mouseup 1; sleep 1
# Read by place, not by OCR's word order (it reads a row of names out of order).
RX=$(for n in a b c; do at "$n.png" 460x30+40+180 | cut -d' ' -f1; done | tr '\n' ' ')
read -r XA XB XC <<<"$RX"
check "a file card dragged past the others moves there (a b c at x $RX)" "[ -n \"$XC\" ] && [ \"$XB\" -lt \"$XC\" ] && [ \"$XC\" -lt \"$XA\" ]"
# A drag cancelled by Escape, then one let go over the options panel: the order stays b c a.
xdotool mousemove 101 130 mousedown 1; for i in 1 2 3 4 5 6 7 8 9 10; do xdotool mousemove $(( 101 + i * 35 )) 140; sleep 0.08; done; sleep 0.3; xdotool key Escape; sleep 0.3; xdotool mouseup 1; sleep 0.8
xdotool mousemove 101 130 mousedown 1; for i in $(seq 1 30); do xdotool mousemove $(( 101 + i * 35 )) 300; sleep 0.04; done; sleep 0.3; xdotool mouseup 1; sleep 0.8
RX=$(for n in a b c; do at "$n.png" 460x30+40+180 | cut -d' ' -f1; done | tr '\n' ' ')
read -r XA XB XC <<<"$RX"
check "a drag cancelled by Escape or let go outside the files moves nothing (a b c at x $RX)" "[ -n \"$XC\" ] && [ \"$XB\" -lt \"$XC\" ] && [ \"$XC\" -lt \"$XA\" ]"
quit

# 21b. More files than fit: a card held at the bottom edge scrolls the files and goes past what was in view.
# 30 images, f01 held at the bottom: the files scroll to the end and it lands in the last row (before f27).
for i in $(seq 1 30); do convert -size 60x80 xc:"hsl($(( i * 12 )),60%,50%)" "$OUT/reo/f$(printf %02d "$i").png"; done
launch "$OUT"/reo/f*.png; sleep 1
xdotool mousemove 101 130 mousedown 1; for y in 200 300 400 500 600 700 750 765; do xdotool mousemove 300 $y; sleep 0.08; done; sleep 2.5; xdotool mouseup 1; sleep 0.8; shot 21b-edge
F1=$(at f01 940x30+20+712)
check "a card held at the bottom edge scrolls the files and lands in the last row (f01 at $F1)" "[ -n \"$F1\" ]"
quit

# 21c. "Open file" in a tool offers the PDFs open in reader tabs (Acrobat's open files in Combine):
# two documents open, Merge opened from Tools, Open file → Open documents (2) → both are its files.
convert -size 300x400 xc:'#3366cc' "$OUT/reo/one.pdf"; convert -size 300x400 xc:'#cc6633' "$OUT/reo/two.pdf"
launch "$OUT/reo/one.pdf" "$OUT/reo/two.pdf"
# A note left unsaved in two.pdf: it goes in as last saved, and a toast says so.
key n; click 640 400 0.8; xdotool type --delay 10 "smoke note"; key ctrl+Return; sleep 0.6
click 50 21 1; click 67 113 1; click 271 197 1.5; click 494 405 1; shot 21c-menu
M=$(at Open 400x100+340+440)
click 460 527 1
TOAST=$(import -window root png:- | convert - -resize 200% png:- | tesseract stdin stdout 2>/dev/null | tr '\n' ' ')
check "an open document with unsaved edits is added as saved, and a toast says so" "echo \"$TOAST\" | grep -q 'two.pdf has unsaved changes'"
sleep 1
NAMES=$(ocr 300x24+40+184)
check "Open file in Merge offers the open documents and adds them (menu at $M; cards: $NAMES)" "[ -n \"$M\" ] && echo \"$NAMES\" | grep -q one.pdf && echo \"$NAMES\" | grep -q two.pdf"
quit

# 21d. Merge takes chosen pages from each file (Acrobat's Combine): a.pdf (red green blue yellow) "4,1-2",
# b.pdf (black grey light) "9" refused (the run waits, the card says why), then "3": the merge is yellow red green light.
convert -size 200x260 xc:'#dd2222' xc:'#22aa22' xc:'#2222dd' xc:'#dddd22' "$OUT/reo/ra.pdf"; convert -size 200x260 xc:'#111111' xc:'#777777' xc:'#bbbbbb' "$OUT/reo/rb.pdf"
launch "$OUT/reo/ra.pdf" "$OUT/reo/rb.pdf"
click 50 21 1; click 67 113 1; click 271 197 1.5; click 494 405 1; click 460 527 2
click 101 212 0.6; xdotool type --delay 20 "4,1-2"; key Return
click 237 212 0.6; xdotool type --delay 20 "9"; key Return; sleep 0.3
BAD=$(ocr 300x20+30+202); RUN=$(import -window root -crop 1x1+1015+740 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}')
click 237 212 0.6; xdotool type --delay 20 "3"; key Return
click 1133 755 4
MC=$(for i in 1 2 3 4; do pdftoppm -r 10 -f $i -l $i -png "$OUT/reo/ra-merged.pdf" 2>/dev/null | convert - -format "%[pixel:p{5,5}] " info: 2>/dev/null; done)
NP=$(pdfinfo "$OUT/reo/ra-merged.pdf" 2>/dev/null | awk '/^Pages/{print $2}')
check "a page choice outside the file is refused ($BAD; run button $RUN)" "echo \"$BAD\" | grep -q 'only 1' && [ \"$RUN\" != '#E5322D' ]"
check "Merge takes the pages chosen for each file ($NP pages: $MC)" "[ \"$NP\" = 4 ] && [ \"$MC\" = 'srgb(220,220,33) srgb(220,33,33) srgb(33,169,33) srgb(186,186,186) ' ]"
quit

# 22. Split's pages, cut between by a click (PDF Expert's split points; iLovePDF's ranges): ten pages, cuts after
# 3 and 7 → 1-3, 4-7, 8-10 in the Ranges field and three files; a second click after 3 joins again (1-7,8-10).
# Cards step 128 px from x 41 (gap centre +117), rows 178 px from y 79.
convert -size 200x260 xc:'#dd2222' xc:'#22aa22' xc:'#2222dd' xc:'#dddd22' xc:'#22dddd' xc:'#dd22dd' xc:'#888888' xc:'#111111' xc:'#ff8800' xc:'#0088ff' "$OUT/reo/ten.pdf"
launch "$OUT/reo/ten.pdf"
click 50 21 1; click 67 113 1; click 406 197 2; click 494 405 1; click 460 527 3
click 414 160 0.6; click 158 337 0.6; shot 22-split
R1=$(ocr 240x30+1006+254 | tr -d ' ')
click 414 160 0.6; R2=$(ocr 240x30+1006+254 | tr -d ' ')
click 414 160 0.6; click 1133 755 4
check "clicks between Split's pages cut and join its ranges ($R1 → $R2)" "[ \"$R1\" = 1-3,4-7,8-10 ] && [ \"$R2\" = 1-7,8-10 ]"
check "Split writes a file for each cut range" "[ \"\$(pdfinfo $OUT/reo/ten-4-7.pdf 2>/dev/null | awk '/^Pages/{print \$2}')\" = 4 ] && [ -f $OUT/reo/ten-1-3.pdf ] && [ -f $OUT/reo/ten-8-10.pdf ]"
quit

# 23. Search history (Acrobat's and PDF Expert's recent searches): "fox" then "quick"; the clock in the results
# panel (223,153) lists them newest first; the second ("fox", 60,237) runs again and is back in the field.
launch "$OUT/text.pdf"
key ctrl+f; xdotool type --delay 20 "fox"; key Return; sleep 1.5
key ctrl+f; xdotool type --delay 20 "quick"; key Return; sleep 1.5
click 223 153 0.8; shot 23-recent; click 60 237 1.5
SF=$(ocr 110x26+1000+48 | tr -d ' '); RN=$(ocr 240x24+0+166)
check "a recent search runs again from the list ($SF; $RN)" "echo \"$SF\" | grep -q '^fox' && echo \"$RN\" | grep -q '4 results'"
quit

# 24. More ▸ Highlight form fields (1127,61 ▸ "form" 1008,211) turns the fields' tint off and on (Acrobat's
# Highlight existing fields); Document properties of a file with only an owner password say what it forbids.
launch "$OUT/form.pdf"; key ctrl+9; sleep 1
tint() { import -window root -crop 1x1+640+300 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}'; }
T0=$(tint); click 1127 61 0.8; click 1008 211 1; T1=$(tint); click 1127 61 0.8; click 1008 211 1; T2=$(tint)
check "Highlight form fields turns the fields' tint off and on ($T0 → $T1 → $T2)" "[ \"$T0\" != \"$T1\" ] && [ \"$T0\" = \"$T2\" ] && [ \"$T1\" = '#FFFFFF' ]"
quit
qpdf --encrypt "" owner 256 --print=none --extract=n -- "$OUT/text.pdf" "$OUT/owner.pdf"
launch "$OUT/owner.pdf"; key ctrl+d; sleep 2; PR=$(ocr 440x330+420+400)
check "Document properties say what the owner password forbids and the dates ($(echo $PR | cut -c1-80))" "echo \"$PR\" | grep -q 'Restricted: no printing' && echo \"$PR\" | grep -q 'PDF/A'"
quit

# 25. Files panel (211,106), Acrobat's Attachments: four attached files; "con" in Find (160,150) leaves
# contract.txt alone; a click on annex.pdf (100,185) opens it in its own tab.
echo hello > "$OUT/notes.txt"; echo b > "$OUT/budget.txt"; echo c > "$OUT/contract.txt"; cp "$OUT/text.pdf" "$OUT/annex.pdf"
qpdf "$OUT/text.pdf" --add-attachment "$OUT/annex.pdf" -- --add-attachment "$OUT/notes.txt" -- --add-attachment "$OUT/budget.txt" -- --add-attachment "$OUT/contract.txt" -- "$OUT/withatt.pdf"
launch "$OUT/withatt.pdf"; click 211 106 1
click 160 150 0.5; xdotool type --delay 30 "con"; sleep 0.6; xdotool mousemove 100 400; sleep 0.3; AF=$(ocr 240x140+0+165)
xdotool key ctrl+a BackSpace; sleep 0.5; click 100 185 2; TB=$(ocr 700x30+0+8)
check "attachments are found by name and open, a PDF in its own tab ($AF; tabs: $TB)" "echo \"$AF\" | grep -q contract && ! echo \"$AF\" | grep -q budget && echo \"$TB\" | grep -q 'annex.pdf'"
quit

# 26. Compare's slots offer the documents open in tabs (Original: one.pdf at 330,481 in the menu under Open
# file 388,411; Changed then lists only two.pdf, 560,481 under 600,411).
launch "$OUT/reo/one.pdf" "$OUT/reo/two.pdf"
click 50 21 1; click 67 113 1; click 993 76 0.5; xdotool type --delay 20 "compare"; key Return; sleep 2
click 388 411 1; click 330 481 2; click 600 411 1; click 560 481 2; shot 26-compare
CS=$(ocr 500x40+300+450)
check "Compare's slots take open documents ($CS)" "echo \"$CS\" | grep -q 'one.pdf.*two.pdf'"
quit

# 27. Protect with restrictions only (Acrobat's Restrict editing and printing): "Require a password to open"
# off, a permissions password, Printing off: the file opens with no password and cannot be printed.
cp "$OUT/text.pdf" "$OUT/perm.pdf"
launch "$OUT/perm.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "protect"; key Return; sleep 2
RQ=$(at Require 280x700+1000+90); [ -n "$RQ" ] && click $RQ 0.6
PL=$(at Permissions 280x700+1000+90); [ -n "$PL" ] && click $(echo $PL | awk '{ print $1 + 40, $2 + 30 }') 0.4; xdotool type --delay 20 "owner"
PR=$(at Printing 280x700+1000+90); [ -n "$PR" ] && click $PR 0.4
click 1133 755 4
EN=$(qpdf --show-encryption "$OUT/perm-protected.pdf" 2>&1)
check "Protect can restrict without an open password ($(echo "$EN" | grep -E 'print low' | tr -s ' '))" "echo \"$EN\" | grep -q 'User password = $' && echo \"$EN\" | grep -q 'print low resolution: not allowed'"
quit

# 28. Preferences ▸ Open new documents at Actual size (Preferences 67,193 in Home's menu): a file never
# opened shows at 100 %.
launch; click 67 193 1; AS=$(at Actual 900x700+200+90); [ -n "$AS" ] && click $AS 0.6; quit
cp "$OUT/text.pdf" "$OUT/fresh.pdf"; launch "$OUT/fresh.pdf"; sleep 1.5; ZF=$(zoomval)
check "a new document opens at the zoom chosen in Preferences (Actual size: $ZF %)" "[ \"$ZF\" = 100 ]"
quit
# Fit page: a whole letter page in the 700 px high view, well under fit width (163 %).
launch; click 67 193 1; FP2=$(import -window root -crop 900x700+200+90 png:- | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' '$12=="page"{printf "%d %d\n", 200+($7+$9/2)/2, 90+($8+$10/2)/2; exit}'); [ -n "$FP2" ] && click $FP2 0.6; quit
cp "$OUT/text.pdf" "$OUT/fresh2.pdf"; launch "$OUT/fresh2.pdf"; sleep 1.5; ZP=$(zoomval)
check "a new document opens fitted to the page when chosen ($ZP %)" "[ -n \"$ZP\" ] && [ \"$ZP\" -lt 120 ]"
quit

# 29. Close Other Tabs asks about a document with unsaved edits (as ⌘W would): b.pdf highlighted, then from
# a.pdf's tab menu Close Other Tabs: the Save / Don't Save prompt names b.pdf; Don't Save closes it.
cp "$OUT/text.pdf" "$OUT/ca.pdf"; cp "$OUT/text.pdf" "$OUT/cb.pdf"
launch "$OUT/ca.pdf" "$OUT/cb.pdf"; sleep 1
key h; xdotool mousemove 330 200 mousedown 1 mousemove 800 201 mouseup 1; sleep 0.3; key h; sleep 1
read AX _ <<< "$(at ca 1280x40+0+0)"; xdotool mousemove "${AX:-0}" 21 click 3; sleep 0.8
read OX OY <<< "$(at Other 1280x300+0+0)"; xdotool mousemove "${OX:-0}" "${OY:-0}" click 1; sleep 1; shot 29-close-other
PQ=$(ocr 700x300+290+250)
DS=$(at "Don" 700x300+290+250); [ -n "$DS" ] && click $DS 1
check "Close Other Tabs asks about unsaved edits, Don't Save closes it ($(echo $PQ | cut -c1-60); $(ntabs) tab)" "echo \"$PQ\" | grep -q cb.pdf && [ \"\$(ntabs)\" = 1 ]"
quit

check "the real user profile was not touched" "[ \"\$(real_profile)\" = \"$REAL0\" ]"
echo "---"
# Outputs are deleted on success (SMOKE_KEEP=1 keeps the screenshots, e.g. to file a proof).
if [ $fails -eq 0 ]; then echo "ALL PASS"; [ -n "${SMOKE_KEEP:-}" ] || rm -rf "$OUT"; else echo "$fails FAILED (screenshots in $OUT)"; fi
exit $fails
