# Sourced by dev/scripts/gui-smoke.sh, in name order: shares its helpers, profile and variables.
# Steps 1–6: the reader (annotate, sign, edit, pages, search, forms, view).

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
cp "$OUT/text.pdf" "$OUT/highlighted.pdf"   # kept for step 32

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
# Smoke-only inspection; python cryptography is needed by the smoke environment, not Sheaf.
secure_marks() {
    python3 - "$PWD/$OUT/profile/data/Sheaf" <<'PYSECURE'
from pathlib import Path
import sys
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
p = Path(sys.argv[1])
b = (p / "signatures.secure").read_bytes()
h = b"SheafMarks\x02"
assert b.startswith(h)
print(ChaCha20Poly1305((p / "signatures.key").read_bytes()).decrypt(b[len(h):len(h)+12], b[len(h)+12:], h).decode())
PYSECURE
}
sigs() { secure_marks | python3 -c 'import json, sys; p = json.load(sys.stdin); print(len(p.get("signatures", [])), "".join(map(str, p.get("current", [[], None, [0, 0], 0])[2])))'; }
S1=$(sigs); click 456 100 0.8; click 700 620 1
xdotool mousemove 520 400 mousedown 1; for i in $(seq 0 10); do xdotool mousemove $((520 + i * 10)) $((380 + i * 6)); done; xdotool mouseup 1; sleep 0.3
# The second one in black (the pad's first ink, 448,314); the first kept its blue.
click 448 314 0.3; click 818 555 1.2; S2=$(sigs); shot 02e-two-signatures; click 426 100 0.8; S3=$(sigs)
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
NS=$(at New 900x40+250+82); [ -n "$NS" ] && click $NS 0.8; click 700 620 1; click 640 512 0.3
PAD=$(at Create 600x400+340+200)
[ -n "$PAD" ] && { xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 20 "Ada Lovelace"; sleep 1.5; }; shot 02f-typed-fonts
[ -n "$PAD" ] && { click 742 506 0.4; click 817 614 1.2; key ctrl+s; sleep 1.5; }; SF=$(sfont)
check "a typed signature is placed in the handwriting font picked (font $SF, pad $PAD)" "[ \"$SF\" = 1 ] && qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -aq PCFillSign"
key ctrl+z; key ctrl+s; sleep 1.5
key Escape; key v

# 2b'. Select that signature, pull its corner handle: it grows, keeping its proportions and top-left.
# Page 1 at ⌘9 in the 1280×800 window, Fill & Sign still on (its rail): the signature picked, its options bar
# docks and the page sits 944/612 px a point from 320,134, so screen = (320 + 1.5425·x, 1355.5 − 1.5425·y)
# for PDF (x, y). (Before the pick, with no bar, the page top is at 98: see 2b'' below.)
R0=$(inks "$OUT/text.pdf"); click 700 614 0.8   # the second signature (PDF y ≈ 457) with no bar docked: page top 98
set -- $(python3 -c '
rs = [list(map(float, r.split())) for r in "'"$R0"'".split(";")]
sx = lambda x: 320 + 1.5425 * x; sy = lambda y: 1355.5 - 1.5425 * y
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
print(round(276 + 1.6144 * (r[0] + 12)), round(1376.6 - 1.6144 * (r[1] + r[3]) / 2))')
# Two Escapes: the first drops the selection (or leaves Fill & Sign when there is none), the second leaves the
# mode. Back in Read the page is drawn 988/612 px a point from 276,98.
key Escape; key Escape; sleep 0.5; click $1 $2 0.8; shot 02g-signature-to-delete; key Delete; sleep 1; key ctrl+s; sleep 1.5
R2=$(inks "$OUT/text.pdf")
check "a selected signature is deleted with the Delete key" "[ \$(echo \"$R2\" | tr ';' '\\n' | grep -c .) -eq \$(( \$(echo \"$R1\" | tr ';' '\\n' | grep -c .) - 1 )) ]"
key Escape

# 2c. Edit text: E, click empty space, type, ⌘↩ → new text in the page content.
key e; sleep 0.8; click 640 600 0.8
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Added by smoke"; key ctrl+Return; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5
check "text added with the Edit tool is in the page content" "pdftotext -f 1 -l 1 $OUT/text.pdf - | grep -q 'Added by smoke'"
# The format row: Serif, bold, two sizes up → the page gains Times-Bold.
# The row (from its left, x 272; hairlines between its groups): Sans 297, Serif 352, Mono 409, B 476, I 507,
# U 538, • 592, 1. 627, align left 684 / centre 718 / right 752, − 807, the size field 851, + 894 (y 110). A
# click on the size field would keep the keyboard: the + is found from Mono (OCR reads it; the red Sans it
# does not).
key e; sleep 0.8; MX=$(at Mono 1280x40+0+90 | cut -d' ' -f1); DX=$(( ${MX:-409} - 409 ))
click $(( 352 + DX )) 110 0.3; click $(( 476 + DX )) 110 0.3; click $(( 894 + DX )) 110 0.3; click $(( 894 + DX )) 110 0.3
click 640 680 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Serif bold"; key ctrl+Return; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5; shot 02c-text-format
check "added text takes the chosen font (Times-Bold)" "pdffonts -f 1 -l 1 $OUT/text.pdf 2>/dev/null | grep -q 'Times-Bold'"
# A text box in the same row's format plus italic (the row is shared): Times Bold Italic. Then
# selected (the row takes its format), Mono, aligned right, 30 pt typed: redrawn in Courier Bold
# Oblique, wrapped on two lines in a box grown to keep them, still one box. The text box's row has no U
# nor lists: I 507, align right 628, the size field 727 (from Mono 409).
key t; sleep 0.6; MX=$(at Mono 1280x40+0+90 | cut -d' ' -f1); DX=$(( ${MX:-409} - 409 )); click $(( 507 + DX )) 110 0.3; click 1000 520 0.8
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Boxtest in a font"; sleep 0.5
# Typed on the page where the box goes (Acrobat, PDF Expert), not in a card: the words at the click.
# Read just below the note's icon (it sits above the box's corner and spoils the first letter).
IP=$(ocr 280x60+995+515)
check "a text box is typed in place on the page ($IP)" "echo \"$IP\" | grep -q 'oxtest in a font'"
key ctrl+Return; sleep 1.2
key v; key ctrl+s; sleep 1.5; shot 02c2-text-box-font
check "a text box takes the format row's font (Times Bold Italic)" "qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -aq '/BaseFont /Times-BoldItalic'"
NB=$(qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null | grep -ac '/Subtype /FreeText$')
BX=$(at Boxtest); [ -n "$BX" ] && click $BX 0.8; click $(( 409 + DX )) 110 1; click $(( 628 + DX )) 110 1
click $(( 727 + DX )) 110 0.3; key ctrl+a; xdotool type --delay 20 "30"; key Return; sleep 1.2
shot 02c3-text-box-restyled; click 1150 300 0.5; key ctrl+s; sleep 1.5
QB=$(qpdf --qdf --object-streams=disable $OUT/text.pdf - 2>/dev/null)
check "a selected text box takes a new font, alignment and a typed size, and grows ($BX)" "echo \"\$QB\" | grep -aq '/BaseFont /Courier-BoldOblique' && echo \"\$QB\" | grep -aq '/CoBO 30 Tf' && echo \"\$QB\" | grep -aq '/Q 2' && echo \"\$QB\" | grep -aq '(a font) Tj' && [ \$(echo \"\$QB\" | grep -ac '/Subtype /FreeText\$') -eq $NB ]"

# 2d. Edit tool: drag the first paragraph of page 1 down; its words move down in the saved file.
word_y() { pdftotext -f 1 -l 1 -bbox "$1" - 2>/dev/null | grep -m1 '>quick<' | sed 's/.*yMin="\([0-9.]*\)".*/\1/'; }
# Where "quick" is on screen, from the saved file (OCR misses the big highlighted line): in Edit mode (the
# rail and the docked options bar) the page starts at 320,134 and is drawn 944/612 px a point at fit width.
quick_at() { pdftotext -f 1 -l 1 -bbox "$1" - 2>/dev/null | grep -m1 '>quick<' | sed 's/.*xMin="\([0-9.]*\)" yMin="\([0-9.]*\)" xMax="\([0-9.]*\)" yMax="\([0-9.]*\)".*/\1 \2 \3 \4/' | awk '{ k = 944 / 612; printf "%d %d\n", 320 + ($1 + $3) / 2 * k, 134 + ($2 + $4) / 2 * k }'; }
QY0=$(word_y "$OUT/text.pdf"); read DX DY <<< "$(quick_at "$OUT/text.pdf")"
key e; sleep 1.2
xdotool mousemove $DX $DY mousedown 1; sleep 0.1; for d in 20 60 100 140; do xdotool mousemove $DX $(( DY + d )); sleep 0.05; done; xdotool mouseup 1; sleep 1.2
key Escape; key v; key ctrl+s; sleep 1.5; shot 02d-paragraph-moved
QY1=$(word_y "$OUT/text.pdf")
check "dragging a paragraph with the Edit tool moves it ($QY0 → $QY1)" "[ -n \"$QY1\" ] && python3 -c 'import sys; sys.exit(0 if float(\"$QY1\") - float(\"$QY0\") > 30 else 1)'"
# 2e. Editing that paragraph shows it on the page as typed, before ⌘↩; Escape puts it back and
# leaves the file as it was.
key e; sleep 1; QK=$(quick_at "$OUT/text.pdf"); [ -n "$QK" ] && click $QK 1.2
xdotool windowfocus --sync "$W" 2>/dev/null; key ctrl+End; xdotool type --delay 10 " zebra"
# The preview is a render: waited for (up to 6 s), not slept for (1.5 s missed it on a busy machine).
for _ in $(seq 1 12); do sleep 0.5; PV1=$(ocr 1000x500+260+90); echo "$PV1" | grep -q zebra && break; done
shot 02e-preview; key Escape
for _ in $(seq 1 12); do sleep 0.5; PV2=$(ocr 1000x500+260+90); echo "$PV2" | grep -q zebra || break; done
key Escape; key v
check "an edited paragraph shows on the page before ⌘↩, Escape takes it back" "echo \"$PV1\" | grep -q zebra && ! echo \"$PV2\" | grep -q zebra && ! pdftotext $OUT/text.pdf - | grep -q zebra"

# 3. Page operations from the Pages panel: duplicate page 1, delete it again, rotate.
click 120 230; click 124 778 1.2      # select page 1, Duplicate
key ctrl+s; sleep 1.5
check "duplicate adds a page" "[ \"\$(npages $OUT/text.pdf)\" = 5 ]"
click 120 230; click 220 778 1.2      # Delete
key ctrl+s; sleep 1.5
check "delete removes it" "[ \"\$(npages $OUT/text.pdf)\" = 4 ]"
click 120 230; click 92 778 1.2       # Insert blank page after page 1
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
# oval (317,109), arrow (376,109), Fill (413,109), opacity (550,109, a menu).
launch "$OUT/shapes.pdf"; key ctrl+9
drag() { xdotool mousemove $1 $2 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(($1 + i * $3 / 5)) $(($2 + i * $4 / 5)); sleep 0.05; done; xdotool mouseup 1; sleep 1.2; }
key r; drag 500 370 180 120   # 70 lower than before W3: the docked Shapes bar (69 tall) moved the page down, and the
                              # rectangle sat over the title line, which 6h reads by OCR
# ⇧ held at the press: a rectangle dragged 160 × 50 is saved square. From x 340: a press at 260 lands on the
# sidebar's resize handle (W1, 256–264) and widened the panel for every later step (run 4, 29/10).
xdotool keydown shift; drag 340 650 160 50; xdotool keyup shift
# Opacity opens its slider and quick values: 75 % picked.
# The opacity button found by its "100 %" (the bar's width depends on the shape's options).
# The oval (317,109), then its Fill found by its word (the bar's width depends on its options).
click 361 100 0.8; FL=$(at Fill 800x40+304+82); [ -z "$FL" ] && { click 361 100 1; FL=$(at Fill 800x40+304+82); }
[ -n "$FL" ] && click $FL 0.5; OP=$(at 100 800x40+380+90); [ -n "$OP" ] && click $OP 0.6
P75=$(at 75 600x160+400+120); [ -n "$P75" ] && click $P75 0.5; drag 500 600 200 100
click 420 100; drag 900 300 150 -100
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
check "the Shapes tool keeps its style for next time (opacity 75 % in the preferences)" "python3 -c 'import json, glob, sys; p = json.load(open(glob.glob(\"$PWD/$OUT/profile/data/Sheaf/prefs.json\")[0])); sys.exit(0 if any(t[0] == 10 and t[3] == 75 for t in p.get(\"tool_styles\", [])) else 1)'"
# 6c'0. The first rectangle (unfilled) selected by a click inside it: the Fill row shows for it, its first
# colour (the yellow dot, found by its colour: OCR misses the small "Fill") fills it at once (Acrobat's Properties
# bar): saved, two shapes have /IC.
key v; click 590 400 0.8; shot 06c0-selected   # inside the rectangle whether or not the Shapes bar (69 tall) is docked
YX=$(import -window root -crop 600x1+300+100 txt:- 2>/dev/null | grep -m1 'srgb(255,219,64)' | cut -d, -f1); FY=${YX:+100}
[ -n "$FY" ] && click $(( 300 + YX + 6 )) $FY 1; key ctrl+s; sleep 1.5
NIC=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null | grep -ac '/IC \[')
check "a selected rectangle takes a fill colour at once (${FY:+Fill row}; $NIC filled)" "[ \"\${NIC:-0}\" -ge 2 ]"
# 6c'. The eraser (X) along a short pen stroke rubs it out entirely.
ninks() { inks "$1" | tr ';' '\n' | grep -c .; }
key p; drag 600 720 40 0; key ctrl+s; sleep 1.5; N1=$(ninks "$OUT/shapes.pdf")
key x; drag 585 720 70 0; key ctrl+s; sleep 1.5; N2=$(ninks "$OUT/shapes.pdf")
check "the eraser rubs out a drawing it goes along ($N1 → $N2)" "[ $N1 -ge 1 ] && [ $N2 -eq \$(( N1 - 1 )) ]"
key v

# 6c''. Stamps (M): the default (Approved) with a click; a dated one (Received, seventh in the list at
# 361,304 once the list at 394,100 is open, rows 28 from y 136, after the four marks) says who and when.
key m; click 900 300 1.2; click 394 100 0.8; click 361 304 0.8; click 700 600 1.2
# A checkmark (first in the list, 361,136) placed beside them.
# In blue (the colour row shows for marks, 584,100): Acrobat's Fill & Sign marks take a colour.
click 394 100 0.8; click 361 136 0.8; click 584 100 0.5; click 400 650 1.2; key ctrl+s; sleep 1.5; key v
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
# The filter (64,186 ▸ Stamps at 40,334: rows 28 tall from 222 since W1) lists only stamps; deleting the first row deletes a stamp,
# not what sat at that row's place in the full list; back to All (40,230).
NST=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null | grep -ac '/Subtype /Stamp$')
click 64 186 0.8; click 40 334 1; click 120 240 0.8; key Delete; sleep 1; key ctrl+s; sleep 1.5
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
# The reply's own bin (right of its line, x 232 in the 260-wide panel) deletes it alone: the comment and its status stay.
read _ RY <<< "$(at Smoke 260x500+0+130)"; [ -n "$RY" ] && click 232 "$RY" 1; key ctrl+s; sleep 1.5
QD=$(qpdf --qdf --object-streams=disable "$OUT/shapes.pdf" - 2>/dev/null)
check "a reply is deleted alone, its comment and status kept" "[ -n \"$RY\" ] && ! echo \"\$QD\" | grep -aq '(Smoke reply)' && echo \"\$QD\" | grep -aq '/State (Accepted)'"
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
mid() { import -window root -crop 6x200+767+300 png:- 2>/dev/null | convert - -format "%[fx:mean]" info:; }
# Back to Read first (two Escapes: selection, then the mode): the canvas is then 260–1280 and the gap at 770,
# as it is when the document reopens (6f).
key Escape; key Escape; sleep 0.8; M1=$(mid); key ctrl+alt+2; sleep 2.5; M2=$(mid); key ctrl+alt+2; sleep 2.5; key ctrl+alt+2; sleep 2.5; M3=$(mid); shot 06d-spread
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
# 6g. The zoom field (ZOOM_X,61): 200 typed and ↩ sets 200 %; Fit page from its list (ZOOM_MENU_X,61 ▸ 16 px
# left of it, 311) sets a smaller zoom that shows the whole page. The figure is read from the field: with the
# side panel open the page already covers the view's left edge at 163 %, so the edge said nothing (28/10).
zoomread() { import -window root -crop 80x26+$((ZOOM_X - 37))+48 png:- 2>/dev/null | convert - -resize 300% png:- | tesseract stdin stdout --psm 7 2>/dev/null | tr -cd '0-9'; }
click $ZOOM_X 61 0.5; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool key ctrl+a; xdotool type --delay 20 "200"; key Return; sleep 2; Z1=$(zoomread)
# The arrow takes its click once the pointer has rested on it (a click arriving with the move did nothing
# after the field had been typed in, 28/10).
xdotool mousemove $ZOOM_MENU_X 61; sleep 0.5; xdotool click 1; sleep 1; xdotool mousemove $((ZOOM_MENU_X - 16)) 311; sleep 0.5; xdotool click 1; sleep 2; Z2=$(zoomread)
check "zoom typed (200 %) and Fit page from the list (${Z1:-?} → ${Z2:-?} %)" "[ \"$Z1\" = 200 ] && [ -n \"$Z2\" ] && [ \"$Z2\" -lt 200 ] && [ \"$Z2\" -gt 20 ]"
key ctrl+9; sleep 1
# 6h. By hand: ⌘ + wheel zooms in around the pointer (the zoom field at ZOOM_X grows); a middle-button drag,
# then a drag with Space held, move the pages (from the top of page 1, where the text is); Space let go
# without a drag still turns the page.
pagesig() { import -window root -crop 1000x690+260+90 png:- 2>/dev/null | md5sum | cut -c1-8; }
# The zoom field read alone (a single line at 300 %: the default OCR misses it).
zoomval() { import -window root -crop 70x26+$((ZOOM_X - 32))+48 png:- 2>/dev/null | convert - -resize 300% png:- | tesseract stdin stdout --psm 7 2>/dev/null | tr -cd '0-9'; }
key Home; sleep 1; shot 06h-before-wheel; ZA=$(zoomval); QK=$(at quick 1020x700+255+90); # The pointer arrives as a hand moves it (two steps, a pause): a wheel event read before the move
# into the page was handled would anchor on where the pointer was before.
[ -n "$QK" ] && xdotool mousemove $(echo $QK | awk '{ print $1 - 6, $2 - 4 }') && sleep 0.15 && xdotool mousemove $QK && sleep 0.4; xdotool keydown ctrl click 4 click 4 keyup ctrl; sleep 2; ZB=$(zoomval); QK2=$(at quick 1020x700+255+90)
check "⌘ + wheel zooms in ($ZA → $ZB %)" "[ -n \"$ZA\" ] && [ -n \"$ZB\" ] && [ $ZB -gt $ZA ]"
# The word under the pointer stays under it (OCR centres within 60 px). "quick", not "fox": the arrow drawn
# at 6c ends beside "fox", and OCR read the two as one word some runs, its centre 63 px right of the word
# (the "drift" 961 → 889 of 25/10 was that: the zoom kept the point under the pointer).
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

# 6i. A link says where it goes under the pointer ("Page 2", the tooltip layer's tip under the
# pointer, centred on it); away from it, nothing. Read by OCR under the pointer (the page's own
# "Go to page 2" is above it).
launch "$OUT/links.pdf"; key ctrl+9; sleep 0.5
under() { ocr 200x40+385+252; }
xdotool mousemove 900 600; sleep 0.6; U0=$(under); xdotool mousemove 480 235; sleep 0.2; xdotool mousemove 485 238; sleep 0.8; U1=$(under)
check "a link shows where it goes on hover ($U0 → $U1)" "! echo \"$U0\" | grep -q 'Page 2' && echo \"$U1\" | grep -q 'Page 2'"
# Scrolling under a still pointer takes the tooltip away (it would name a link no longer there).
key Down; key Down; sleep 0.8; U2=$(under)
check "the link tooltip goes when the page scrolls ($U1 → $U2)" "! echo \"$U2\" | grep -q 'Page 2'"
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
