# Sourced by dev/scripts/gui-smoke.sh, in name order: shares its helpers, profile and variables.
# Steps 7–20: passwords, tabs, tools from the reader, page images, files from outside, Redact, links,
# the Edit tool.

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
# Run (1140,755): DRAFT is stamped. Watermark again on that file with Remove (the third mode): it is gone.
click 1140 755 4; quit
launch "$OUT/tool-watermarked.pdf"; click $WRENCH_X 61 1.5; xdotool type --delay 20 "watermark"; key Return; sleep 2.5
RMV=$(at Remove 280x300+1000+340); [ -n "$RMV" ] && click $RMV 0.8; click 1140 755 4
check "Watermark ▸ Remove takes out the watermark placed (${RMV:-no Remove})" "pdftotext $OUT/tool-watermarked.pdf - | grep -q DRAFT && [ -f $OUT/tool-watermarked-watermarked.pdf ] && ! pdftotext $OUT/tool-watermarked-watermarked.pdf - | grep -q DRAFT"
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
    sleep 1; RS=$(ocr 700x40+100+720 | tr -s ' '); RV=$(ocr 900x320+100+120 | tr -s ' ')
fi
RT=$(pdftotext -f 1 -l 1 "$OUT/redact-redacted.pdf" - 2>/dev/null | tr -s ' \n' ' ')
check "Redact: a drag over words and a double click mark them ($RM), only they are removed ($RT), counted as text ($RS)" "echo \"$RM\" | grep -qi quick && echo \"$RT\" | grep -q '1 the' && ! echo \"$RT\" | grep -qE 'Page|quick|brown|fox' && echo \"$RS\" | grep -q '2 text passages'"
check "the Redact result shows the first redacted page as written ($(echo $RV | cut -c1-60))" "echo \"$RV\" | grep -q 'Page 1 as redacted'"
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
