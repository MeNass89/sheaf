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
# 9. Settings ▸ Reading ▸ Author name: what is typed is kept.
# The field (220 wide) ends on the cards' trailing edge, on its label's row.
click 50 21 0.8; settings 2; AN=$(sfind Author); [ -n "$AN" ] && { read _ AY <<< "$AN"; click $(( SX + SWW - 36 - 110 )) $AY 0.5; }
stype "Smoke Author"; sleep 0.8; shot 09-preferences; sclose
check "the author name typed in Preferences is kept" "grep -rqs '\"author\": \"Smoke Author\"' $OUT/profile"
# Tools search: + opens Tools with the search focused; "pdf to", ↓, ↩ opens the second result.

# Home ▸ All Tools opens with the search taking the keyboard: typed without clicking the field.
TB0=$(ocr 1280x30+0+6); click $ALL_TOOLS_ROW 0.8; click $RECENTS_ROW 0.5; click $ALL_TOOLS_ROW 0.8
xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 20 "pdf to"; sleep 0.6; key Down; key Return; sleep 1.5; shot 09b-tool-search
# The tab's 13 px bold title can OCR badly ("ppFtoword"): the options panel's title is read too.
TB1="$(ocr 1280x30+0+6) $(ocr 290x40+990+47)"
# OCR drops spaces: compare without them.
check "tool search: 'pdf to', ↓, ↩ opens PDF to Word ($TB1)" "echo \"$TB1\" | tr -d ' ' | grep -qi 'PDFtoWord' && ! echo \"$TB0\" | tr -d ' ' | grep -qi 'PDFtoWord'"
# The star on Compress's tile (shown on hover, at the tile's top left corner: tile_star): saved as a favourite, a Favourites row on top.
# The search still holds "pdf to" (the page was not left): cleared with Escape in the field first.
click 40 21 0.8; click $ALL_TOOLS_ROW 0.8; click $TOOLS_SEARCH 0.4; key Escape; sleep 0.5; shot 09c-before-star
read CX CY <<< "$(at Compress 1028x652+252+140)"; [ -n "$CY" ] && tile_star $CX $CY
check "starring a tool puts it in the preferences' favourites" "grep -rqs '\"compress\"' $OUT/profile/data/Sheaf/prefs.json"
# ⌘, opens Settings from a tool tab and from a document (keys go up to the app): its window, on its tab bar.
settings; S1=$SWIN; PT1=$(ocr ${SWW}x52+$SX+$SY); sclose
click 230 21 1; settings; S2=$SWIN; PT2=$(ocr ${SWW}x52+$SX+$SY); sclose   # the text.pdf tab
check "⌘, opens Settings from a tool tab and from a document ($PT1 / $PT2)" "[ -n \"$S1\" ] && [ -n \"$S2\" ] && echo \"$PT1\" | grep -q Shortcuts && echo \"$PT2\" | grep -q Shortcuts"
quit

# 10. A tool from the reader: the wrench (WRENCH_X,61) → Tools, "compress" typed in its search, ↩ → Compress PDF
# (1140,755); outputs go next to the original (set in the profile), the result screen shows.
cp "$OUT/form.pdf" "$OUT/tool.pdf"
mkdir -p "$PWD/$OUT/profile/data/Sheaf"
python3 -c 'import json, sys; p = sys.argv[1]; d = json.load(open(p)) if __import__("os").path.exists(p) else {}; d["output"] = "next_to_original"; json.dump(d, open(p, "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json"
launch "$OUT/tool.pdf"; sleep 1
# Low compression chosen first (its card, found by its title): Run again below must bring it back.
click $WRENCH_X 61 1.5; xdotool type --delay 20 "compress"; key Return; sleep 2
cardbg() { import -window root -crop "1x1+1232+$1" txt:- 2>/dev/null | tail -1 | awk '{ print $3 }'; }
read _ LOWY <<< "$(at Low 280x500+1000+40)"; LOW0=$(cardbg "${LOWY:-0}"); [ -n "$LOWY" ] && click 1100 $LOWY 0.6; LOW1=$(cardbg "${LOWY:-0}")
click 1140 755 4; shot 10-tool-result
check "a tool run from the reader writes its output (Compress)" "qpdf --check $OUT/tool-compressed.pdf >/dev/null 2>&1"
# The same star on Home ▸ Recents' Tools for you (Compress is the sixth tile; starred above, so its star shows).
click 40 21 0.8; click $RECENTS_ROW 0.8; read CX CY <<< "$(at Compress 1028x200+252+250)"; shot 09d-recent-tools
[ -n "$CY" ] && tile_star $CX $CY
check "unstarring from Home's Tools for you takes it out of the favourites (${CY:-no tile})" "! grep -qs '\"compress\"' $OUT/profile/data/Sheaf/prefs.json"
# Recent files' star (PDF Expert's Favorites), on the card of tool-compressed.pdf found by its name: starred in
# the preferences; the search field ("compressed") keeps it alone in the list.
read RX RY <<< "$(at tool-compressed 1280x760+0+40)"
# The star sits at the card's top centre, 138 px above its name.
if [ -n "$RY" ]; then xdotool mousemove $RX $(( RY - 60 )); sleep 0.4; click $RX $(( RY - 138 )) 0.8; fi
PN=$(grep -c 'tool-compressed.pdf' $OUT/profile/data/Sheaf/prefs.json 2>/dev/null)
# The field found by its placeholder's "recent" (the top bar has a Search of its own).
# A faint placeholder may not be read: then the field is the 200 px left of the order popup ("Newest").
SR=$(at recents 1280x760+0+40); [ -z "$SR" ] && { read NX NY <<< "$(at Newest 1280x760+0+40)"; SR=${NY:+$(( NX - 140 )) $NY}; }
[ -n "$SR" ] && { click $SR 0.3; xdotool type --delay 20 "compressed"; sleep 0.8; }
RL=$(ocr 1240x500+20+260); shot 09e-recent-search
check "a recent file is starred and the search finds it alone (${RY:-no card}; prefs: $PN; list: $(echo "$RL" | cut -c1-60))" "[ \"\${PN:-0}\" -ge 1 ] && echo \"$RL\" | grep -q 'tool-compressed' && ! echo \"$RL\" | grep -q 'text.pdf'"
# Tasks History ▸ Run again on the Compress row: a new Compress tab holding tool.pdf, Low compression chosen
# again (its card's background as when it was picked, not as before).
# Run again found by its first word (in the accent blue tesseract reads it "Runagain").
H=$(at Tasks 220x400+0+40); [ -n "$H" ] && click $H 1; RA=$(at Run 1080x700+200+40); [ -n "$RA" ] && click $RA 1.5; shot 10b-rerun
RR=$(ocr 940x680+30+70); RP=$(ocr 280x300+1000+40)   # the file area inside its dashed edge (below the tab bar, which names tool.pdf too; the dashes on the darker canvas garble the card), the options
read _ LOWY2 <<< "$(at Low 280x500+1000+40)"; LOW2=$(cardbg "${LOWY2:-0}")
check "Run again from Tasks History reopens Compress with its file ($H / $RA)" "[ -n \"$RA\" ] && echo \"$RR\" | grep -q 'tool.pdf' && echo \"$RP\" | grep -qi 'Recommended'"
check "Run again starts from the options the task ran with (Low: $LOW0 → $LOW1, again $LOW2)" "[ -n \"$LOWY\" ] && [ \"$LOW1\" != \"$LOW0\" ] && [ \"$LOW2\" = \"$LOW1\" ]"
# Add watermark from the reader tab (tool.pdf, 230,21): the preview of page 1, pinned above the options
# (1004–1252 × 106–336), turns red once "DRAFT" is typed (text field 31 px above the "Font" label) at 120 pt (the
# size field 71 px below it) in red (found by its word).
red() { import -window root -crop 248x230+1004+106 png:- 2>/dev/null | convert - -fx '(r>0.6&&g<0.4&&b<0.4)?1:0' -format '%[fx:mean*10000]' info: | cut -d. -f1; }
click 230 21 1; click $WRENCH_X 61 1.5; xdotool type --delay 20 "watermark"; key Return; sleep 2.5; R0=$(red)
read _ FY <<< "$(at Font 280x700+1000+80)"; FY=${FY:-455}; RC=$(at Red 280x700+1000+80)
click 1130 $(( FY - 31 )) 0.4; xdotool type --delay 30 "DRAFT"; click 1060 $(( FY + 71 )) 0.4; xdotool type --delay 30 "120"; [ -n "$RC" ] && click $RC 0.3; sleep 2.5
shot 10c-watermark-preview; R1=$(red)
check "the watermark preview shows the mark as typed, in its colour ($R0 → $R1)" "[ \"\${R1:-0}\" -gt \$(( \${R0:-0} + 50 )) ]"
# Run (1140,755): DRAFT is stamped. Watermark again on that file with Remove (the third mode): it is gone.
click 1140 755 4; quit
launch "$OUT/tool-watermarked.pdf"; click $WRENCH_X 61 1.5; xdotool type --delay 20 "watermark"; key Return; sleep 2.5
RMV=$(at Remove 280x400+1000+80); [ -n "$RMV" ] && click $RMV 0.8; click 1140 755 4   # Remove, the third mode beside Place Image
check "Watermark ▸ Remove takes out the watermark placed (${RMV:-no Remove})" "pdftotext $OUT/tool-watermarked.pdf - | grep -q DRAFT && [ -f $OUT/tool-watermarked-watermarked.pdf ] && ! pdftotext $OUT/tool-watermarked-watermarked.pdf - | grep -q DRAFT"
quit

# 11. A damaged file says so in one line and the app stays up; Preferences ▸ Dark and Light switch the window.
printf '%%PDF-1.7\nnot really a pdf\n' > "$OUT/broken.pdf"
# The toast (white on dark, bottom centre, 3 s) read inverted until it is seen (up to 30 reads).
# Only the inside of the pill (dark, whatever is behind it), inverted to black on white.
toastText() { import -window root -crop 380x26+450+742 png:- 2>/dev/null | convert - -negate -resize 300% png:- | tesseract stdin stdout --psm 7 2>/dev/null; }
# Read from the start, alongside launch: its wait for the first frame can outlast the toast's 3 s on a busy machine.
( for i in $(seq 1 30); do t=$(toastText); echo "$t"; echo "$t" | grep -qi damaged && break; sleep 0.1; done > "$OUT/toast.txt" ) & TP=$!
launch "$OUT/broken.pdf"; wait $TP; BR=$(tr '\n' ' ' < "$OUT/toast.txt"); shot 11-broken
check "a damaged PDF is refused in one line, the app stays open" "kill -0 $APP_PID 2>/dev/null && echo \"$BR\" | grep -qi 'damaged or not a PDF'"
# It opens in Repair PDF, ready to rebuild it; Home (60,21) again for the theme checks.
RP=$(ocr 1280x30+0+5); check "a damaged PDF opens in Repair PDF (${RP:0:60})" "echo \"$RP\" | grep -q 'Repair'"
click 60 21 1
lum() { import -window root -crop 40x40+600+700 png:- 2>/dev/null | convert - -colorspace Gray -format '%[fx:mean]' info:; }
settings 1
DK=$(at Dark "$SG"); [ -n "$DK" ] && click $DK 1.2; L1=$(lum); shot 11-dark
LT=$(at Light "$SG"); [ -n "$LT" ] && click $LT 1.2; L2=$(lum)
check "Settings ▸ Appearance ▸ Dark then Light switch the window ($L1 → $L2)" "python3 -c 'import sys; sys.exit(0 if $L1 < 0.3 and $L2 > 0.7 else 1)'"
quit

# 12. Twelve documents: the tabs narrow to share the bar, the active one keeps its name readable and
# its ×; a middle click on it closes it.
for i in $(seq -w 1 12); do cp "$OUT/form.pdf" "$OUT/m$i.pdf"; done
launch $(for i in $(seq -w 1 12); do echo "$OUT/m$i.pdf"; done); sleep 4; shot 12-many-tabs
# Tabs counted along the bar (y = 10, above the names and icons) by the one hairline at every boundary
# of the strip: Home | tab | … | tab | +, a divider or the selected tab's side (widgets/tab-bar.slint):
# one more boundary than tabs.
# The active tab, last opened, is the one before the + (1160,21), its name in full.
ntabs() { import -window root -crop 1280x1+0+10 png:- 2>/dev/null | convert - txt:- | python3 -c '
import re, sys
runs, inside = 0, False
for line in sys.stdin:
    m = re.search(r"\((\d+),(\d+),(\d+)", line)
    if m:
        r, g, b = map(int, m.groups()); p = 205 <= r <= 228 and 205 <= b <= 234 and abs(r - g) < 6
        runs += p and not inside; inside = p
print(max(runs - 1, 0))'; }
# The active tab's name alone (its icon and × beside it made tesseract read m12 as ml2).
N0=$(ntabs); NM=$(ocr 70x30+1125+5 | tr -d ' '); xdotool mousemove 1160 21 click 2; sleep 1; N1=$(ntabs)
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
# The image bar: rotate left, rotate right, flip, replace, save as, delete at x 287, 322, 355, 388, 423, 458 (y 110).
click 355 110 1.2; key ctrl+s; sleep 1.5; FR=$(centre "$(saved '#ff0000')"); FB=$(centre "$(saved '#0000ff')")
check "Flip puts the red half on the right ($FR | $FB)" "[ \"\${FR%% *}\" ] && python3 -c 'import sys; sys.exit(0 if float(sys.argv[1]) > float(sys.argv[2]) + 20 else 1)' \${FR%% *} \${FB%% *}"
click 322 110 1.2; key ctrl+s; sleep 1.5; TR=$(centre "$(saved '#ff0000')"); TB=$(centre "$(saved '#0000ff')")
check "Rotate right turns it a quarter clockwise: red (right) goes down ($TR / $TB)" "[ \"\${TR#* }\" ] && python3 -c 'import sys; sys.exit(0 if float(sys.argv[1]) > float(sys.argv[2]) + 20 else 1)' \${TR#* } \${TB#* }"
# Save image as… (Acrobat's): the picture written in its own format (a PNG here, 60 × 40 px turned: 40 × 60).
answer "$PWD/$OUT/saved-image.png"; click 423 110 1.5; answer ""; SI=$(identify -format '%m %w %h' "$OUT/saved-image.png" 2>/dev/null)
check "Save image as writes the picture to a file ($SI)" "echo \"$SI\" | grep -qE '^PNG (40 60|60 40)$'"
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
    check "Word files opened from outside open Word to PDF, each with its page count ($OT, $OP)" "echo \"$OT\" | tr -d ' ' | grep -q 'WordtoPDF' && [ $OP -ge 3 ]"
    check "the order button sorts the files A to Z (abc ${A0:-?} > file 10 ${T0:-?} → abc ${A1:-?} < file 10 ${T1:-?})" "[ -n \"$A1\" ] && [ -n \"$T1\" ] && [ $A0 -gt $T0 ] && [ $A1 -lt $T1 ]"
    click 1133 755 1; for _ in $(seq 1 60); do [ -f "$OUT/office/file 10.pdf" ] && [ -f "$OUT/office/abc.pdf" ] && break; sleep 0.5; done; sleep 1
    check "converted next to the originals, headings as bookmarks" "[ \$(ls \"$OUT\"/office/*.pdf | wc -l) -eq 3 ] && qpdf --qdf --object-streams=disable \"$OUT/office/abc.pdf\" - 2>/dev/null | grep -aq '/Outlines'"
    quit
else
    echo "SKIP 15 (Office to PDF): LibreOffice not installed"
fi

# 16. Images opened from outside open in the reader, each in its own tab, as a preview (C-reader-064, Preview's
# and Stirling's way): a.png and b.png give two reader tabs, each a one-page preview PDF of its picture in Sheaf's
# temporary folder; nothing is merged or written next to the pictures.
mkdir -p "$OUT/img"; convert -size 400x300 xc:'#3366cc' "$OUT/img/a.png"; convert -size 300x400 xc:'#cc6633' "$OUT/img/b.png"
launch "$OUT/img/a.png" "$OUT/img/b.png"; sleep 3; JT=$(ocr 700x30+100+8); shot 16-images-opened
JA=$(ls "$OUT"/profile/tmp/sheaf-previews/*/a.png.pdf 2>/dev/null | head -1); JB=$(ls "$OUT"/profile/tmp/sheaf-previews/*/b.png.pdf 2>/dev/null | head -1)
NA=$(qpdf --show-npages "$JA" 2>/dev/null); NB=$(qpdf --show-npages "$JB" 2>/dev/null); NX=$(ls "$OUT/img" | grep -vc '\.png$')
# The page in view is a.png's blue or b.png's orange: a picture, not an empty tool page (8-bit colour; c8 is
# defined again in 4-flows.sh, which runs later).
c8() { import -window root -crop "1x1+$1+420" -depth 8 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}' | head -1; }
PV=$(c8 760)
check "images opened from outside open each in its own reader tab ($JT; previews ${NA:-?} + ${NB:-?} pages; $NX other files; page $PV)" "echo \"$JT\" | grep -q 'a.png' && echo \"$JT\" | grep -q 'b.png' && [ \"$NA\" = 1 ] && [ \"$NB\" = 1 ] && [ \"$NX\" = 0 ] && ! echo \"$JT\" | grep -q 'JPG to PDF' && python3 -c 'import sys; v = sys.argv[1].lstrip(\"#\"); r, g, b = (int(v[k:k + 2], 16) for k in (0, 2, 4)); sys.exit(0 if abs(r - b) > 80 else 1)' '${PV:-#FFFFFF}'"
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
RM=; RS=; RV=   # said empty in the check when the words are not found
Q=$(span quick 820x700+140+60); F=$(span fox 820x700+140+60)
if [ -n "$Q" ] && [ -n "$F" ]; then
    # From the left edge of "quick" to the right edge of "fox" (a letter counts once the pointer passes its middle).
    set -- $Q; QX=$(( $1 + 1 )); QY=$3; set -- $F; FX=$(( $2 - 1 ))
    xdotool mousemove $QX $QY mousedown 1; for i in 1 2 3 4 5 6; do xdotool mousemove $(( QX + (FX - QX) * i / 6 )) $QY; sleep 0.05; done; xdotool mouseup 1; sleep 1
    P=$(at Page 820x700+140+60); [ -n "$P" ] && { set -- $P; xdotool mousemove $1 $2 click --repeat 2 --delay 120 1; sleep 1; }
    shot 18-redact-marked; RM=$(ocr 280x90+1000+525 | tr -s ' ')
    click 1140 755 1; for _ in $(seq 1 20); do [ -f "$OUT/redact-redacted.pdf" ] && break; sleep 0.5; done
    sleep 1; RS=$(ocr 700x40+100+720 | tr -s ' '); RV=$(ocr 900x50+20+95 | tr -s ' ')
fi
RT=$(pdftotext -f 1 -l 1 "$OUT/redact-redacted.pdf" - 2>/dev/null | tr -s ' \n' ' ')
check "Redact: a drag over words and a double click mark them ($RM), only they are removed ($RT), counted as text ($RS)" "echo \"$RM\" | grep -qi quick && echo \"$RT\" | grep -q '1 the' && ! echo \"$RT\" | grep -qE 'Page|quick|brown|fox' && echo \"$RS\" | grep -q '2 text passages'"
check "the Redact result shows the first redacted page as written ($(echo $RV | cut -c1-60))" "echo \"$RV\" | grep -q 'Page 1 as redacted'"
quit

# 18b. Redact whole pages by range (Acrobat's Redact Pages): "2-3" typed in the field by "Mark whole page", ↩:
# two marks listed; applied, pages 2 and 3 have no text left, pages 1 and 4 keep theirs.
cp "$OUT/text.pdf" "$OUT/pages.pdf"
launch "$OUT/pages.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "redact"; key Return; sleep 3
PF=$(at Pages: 300x700+990+60); [ -n "$PF" ] && click $PF 0.4; xdotool type --delay 30 "2-3"; key Return; sleep 3
PM=$(ocr 260x80+1000+540 | tr -s ' ')   # the marks list, under "Marked for redaction"
click 1140 755 1; for _ in $(seq 1 20); do [ -f "$OUT/pages-redacted.pdf" ] && break; sleep 0.5; done; sleep 0.5
PT=$(for p in 1 2 3 4; do pdftotext -f $p -l $p "$OUT/pages-redacted.pdf" - 2>/dev/null | tr -d '[:space:]' | wc -c; done | tr '\n' ' ')
check "Redact marks a range of whole pages ($(echo $PM | grep -oi 'Whole page [0-9]' | tr '\n' ' '); characters per page: $PT)" "echo \"$PM\" | grep -qi 'Whole page 2' && echo \"$PM\" | grep -qi 'Whole page 3' && [ \"\$(echo $PT | cut -d' ' -f2,3)\" = '0 0' ] && [ \"\$(echo $PT | cut -d' ' -f1)\" -gt 0 ]"
quit

# 19. Link tool (L), as PDF Expert's Edit ▸ Link: drag a box, type a page number, ↩; click the link with the
# tool, change it to an address, ↩; saved, the file has one /Link to https://example.org. Then the edit group's
# Redact… (folded below 1320 px: its button at 413,61, then the item found by its word) opens the document in
# Redact PDF.
cp "$OUT/text.pdf" "$OUT/link.pdf"
launch "$OUT/link.pdf"; key ctrl+9; sleep 1; key l
xdotool mousemove 330 190 mousedown 1; for i in 1 2 3 4 5 6; do xdotool mousemove $((330 + i * 40)) $((190 + i * 4)); sleep 0.05; done; xdotool mouseup 1; sleep 1
xdotool type --delay 30 "2"; key Return; sleep 1.5
click 420 200 1; LE=$(ocr 320x130+300+200 | tr -s ' '); shot 19-link-existing
key ctrl+a; xdotool type --delay 20 "example.org"; key Return; sleep 1; key ctrl+s; sleep 2
LK=$(qpdf --qdf --object-streams=disable "$OUT/link.pdf" - 2>/dev/null | grep -ac "/Subtype /Link"); LU=$(qpdf --qdf --object-streams=disable "$OUT/link.pdf" - 2>/dev/null | grep -ao "/URI (https://example.org)")
check "Link tool: a box linked to page 2 ($LE), changed to an address, saved ($LK link, $LU)" "echo \"\$LE\" | grep -q 'Remove link' && [ \"$LK\" = 1 ] && [ -n \"$LU\" ]"
click 413 61 0.8; RX=$(at Redact 300x200+398+80); [ -n "$RX" ] && click $RX 3; RD=$(ocr 1280x40+0+0 | tr -s ' ')
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
shot 20c-drawing-moved
# Still picked: ⌘C copies it (as objects for Sheaf, as a picture for other apps).
printf '' | xclip -selection clipboard; key ctrl+c; sleep 1.2; DC=$(xclip -selection clipboard -t TARGETS -o 2>/dev/null | tr '\n' ' ')
key ctrl+s; sleep 2
Y1=$(blue_top); WA=$(pdftotext -bbox "$OUT/square.pdf" - 2>/dev/null | grep -m1 '>Alpha<' | sed 's/.*yMin="\([0-9.]*\)".*/\1/')
check "a drawing pressed and dragged moves alone (square top $Y0 → $Y1 px, Alpha at $WA)" "[ \"$Y0\" -ge 0 ] && [ \"$Y1\" -gt \"$(( Y0 + 20 ))\" ] && python3 -c 'import sys; sys.exit(0 if abs(float(\"$WA\") - 74.768) < 1 else 1)'"
check "a picked drawing is copied as a picture by ⌘C ($DC)" "echo \"$DC\" | grep -q 'image/png'"
# Saved after the copy: ⌘W closes it without asking (a copy's answer gave back its count; Undo was greyed and
# closing asked to save until 26/10).
key ctrl+w; sleep 1.2; CW=$(ntabs)
check "a document copied from and saved closes without asking ($CW tabs)" "[ \"$CW\" = 0 ]"
quit

# 20e. A paragraph mixing fonts copied and pasted (C-edit-010, PDF Expert's formatted paste): "Payment is due within"
# in Helvetica, "thirty days" in Helvetica-Bold, a second line in Helvetica; picked with the Edit tool, ⌘C, page 2,
# ⌘V, saved: page 2 holds the words in a bold and a regular font (it was one standard font, the bold lost).
python3 - "$OUT/mixed.pdf" <<'PY'
import sys
objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>", "<< /Type /Catalog /Pages 3 0 R >>", "<< /Type /Pages /Kids [5 0 R 7 0 R] /Count 2 >>"]
c = "BT /F1 24 Tf 72 700 Td (Payment is due within ) Tj /F2 24 Tf (thirty days) Tj /F1 24 Tf 0 -30 Td (of the invoice date.) Tj ET"
objs.append(f"<< /Length {len(c)} >>\nstream\n{c}\nendstream")
res = "/Resources << /Font << /F1 1 0 R /F2 6 0 R >> >>"
objs.append(f"<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Contents 4 0 R {res} >>")
objs.append("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>")
# Page 2 empty, with no fonts of its own: what the paste brings is all pdffonts lists there.
objs.append("<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] >>")
b = b"%PDF-1.7\n"; offs = []
for i, o in enumerate(objs, 1):
    offs.append(len(b)); b += f"{i} 0 obj\n{o}\nendobj\n".encode()
x = len(b)
b += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode() + b"".join(f"{n:010d} 00000 n \n".encode() for n in offs)
b += f"trailer\n<< /Size {len(objs)+1} /Root 2 0 R >>\nstartxref\n{x}\n%%EOF\n".encode()
open(sys.argv[1], "wb").write(b)
PY
launch "$OUT/mixed.pdf"; key ctrl+9; sleep 1; key e; sleep 1.5
P=$(at Payment 1020x700+255+90)
if [ -n "$P" ]; then
    set -- $P; xdotool mousemove $1 $2 keydown shift click 1 keyup shift; sleep 0.8
    # Page 2 by its thumbnail (Page Down only scrolls page 1 at this zoom): the second cell, from y 488.
    key ctrl+c; sleep 1.5; click 124 560 1.5; key ctrl+v; sleep 2; shot 20e-pasted; key ctrl+s; sleep 2
fi
F2=$(pdffonts -f 2 -l 2 "$OUT/mixed.pdf" 2>/dev/null | tail -n +3 | awk '{print $1}' | tr '\n' ' ')
T2=$(pdftotext -f 2 -l 2 "$OUT/mixed.pdf" - 2>/dev/null | tr '\n' ' ' | tr -s ' ')
check "a pasted paragraph mixing fonts keeps its bold run bold (${P:-not found}; page 2 fonts: $F2; text: $T2)" "echo \"$F2\" | grep -q 'Bold' && echo \"$F2\" | tr ' ' '\n' | grep -v Bold | grep -q 'Helvetica\|Arial\|Liberation' && echo \"$T2\" | grep -q 'thirty days' && echo \"$T2\" | grep -q 'invoice date'"
quit

# 20d. Header & footer with Bates numbers (Acrobat's Header & Footer ▸ Bates Numbering), in the Page numbers tool
# found by "bates" (its description): No page number, "Case {bates}" in the top left, prefix ACME-, run: page 1
# carries "Case ACME-000001", page 2 "Case ACME-000002".
cp "$OUT/text.pdf" "$OUT/hf.pdf"; launch "$OUT/hf.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "bates"; key Return; sleep 2.5
xdotool mousemove 1130 450; for i in $(seq 1 25); do xdotool click 5; sleep 0.03; done; sleep 0.8; shot 20d-header-footer
# Fields found from their labels (placeholders are too faint to read): Top's Left field 84 px right of "Top",
# the prefix field 33 px under its label's "prefix" (the help above says "Bates" too).
NP=$(at "No" 280x700+1000+40); [ -n "$NP" ] && click $NP 0.5
read TX TY <<< "$(at Top 280x700+1000+40)"; TL=${TY:+$(( TX + 76 )) $TY}; [ -n "$TL" ] && { click $TL 0.4; xdotool type --delay 20 "Case {bates}"; }
read BX BY <<< "$(at prefix 280x700+1000+40)"; BP=${BY:+$BX $(( BY + 33 ))}; [ -n "$BP" ] && { click $BP 0.4; xdotool type --delay 20 "ACME-"; }
click 1140 755 4
HF=$(pdftotext "$OUT/hf-numbered.pdf" - 2>/dev/null | tr '\n' ' ')
check "Header and footer text with Bates numbers (${NP:-no radio} ${TL:-no field} ${BP:-no prefix}; $(echo "$HF" | cut -c1-80))" "echo \"$HF\" | grep -q 'Case ACME-000001' && echo \"$HF\" | grep -q 'Case ACME-000002'"
quit
