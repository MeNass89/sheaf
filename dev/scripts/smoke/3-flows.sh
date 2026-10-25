# Sourced by dev/scripts/gui-smoke.sh, in name order: shares its helpers, profile and variables.
# Steps 21 on: tool flows (reorder, Merge, Split, Compare, Protect) and the later reader features.

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
# The Ranges field, under its label in the options (its place moves with the cards above it).
read _ RY <<< "$(at Ranges 280x600+1000+100)"; ranges_now() { ocr 240x60+1006+$(( ${RY:-240} - 5 )) | tr -d ' ' | grep -o '[0-9][0-9,-]*' | head -1; }
R1=$(ranges_now)
click 414 160 0.6; R2=$(ranges_now)
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

# 30. Pages panel ▸ Extract… (156,778) opens its options strip, Acrobat's: one file per page, delete the
# pages after. Deleting the only page is refused before any file is asked for.
qpdf "$OUT/text.pdf" --pages . 1 -- "$OUT/single.pdf"; launch "$OUT/single.pdf"; sleep 1
click 156 778 0.8; XS=$(ocr 260x110+0+620)
DL=$(at Delete 260x110+0+620); [ -n "$DL" ] && click $DL 0.3
EX=$(at Extract 260x40+0+690); [ -n "$EX" ] && click $EX 0.6
XT=$(import -window root png:- | convert - -resize 200% png:- | tesseract stdin stdout 2>/dev/null | tr '\n' ' ')
check "Extract offers one file per page and deleting the pages ($XS)" "echo \"$XS\" | grep -q 'One file per page' && echo \"$XS\" | grep -q 'Delete pages'"
check "extracting and deleting the only page is refused" "echo \"$XT\" | grep -q 'needs at least one page' && [ \"\$(npages $OUT/single.pdf)\" = 1 ]"
quit

# 31. The side panel's edge (247,400) dragged 100 px right: the first thumbnail (row y 230) widens
# from 150 to 250 px, and the width is kept for the next start (PDF Expert's sidebar).
thumbw() { import -window root -crop 500x1+0+230 png:- 2>/dev/null | convert - txt:- | awk -F'[(),]' 'NR > 1 && $3 + 0 > 240 && $4 + 0 > 240 && $5 + 0 > 240 { n++ } END { print n + 0 }'; }
launch "$OUT/text.pdf"; sleep 1.5; TW0=$(thumbw)
xdotool mousemove 247 400 mousedown 1; for i in $(seq 1 10); do xdotool mousemove $(( 247 + i * 10 )) 400; sleep 0.04; done; xdotool mouseup 1; sleep 1.5; TW1=$(thumbw); quit
# Dragged back, so later steps find the default panel.
launch "$OUT/text.pdf"; sleep 1.5; TW2=$(thumbw)
xdotool mousemove 347 400 mousedown 1; for i in $(seq 1 10); do xdotool mousemove $(( 347 - i * 10 )) 400; sleep 0.04; done; xdotool mouseup 1; sleep 0.5; quit
check "the side panel's edge resizes it, its thumbnails follow, kept on restart ($TW0 → $TW1 → $TW2 px)" "[ $TW0 -gt 130 ] && [ $TW0 -lt 160 ] && [ $TW1 -gt 230 ] && [ $TW1 -lt 260 ] && [ $TW2 = $TW1 ]"

# 32. A highlight saved without a note, on a page never shown: the Notes panel (153,106) lists the
# words it covers (step 1's highlight, its page moved third).
qpdf "$OUT/highlighted.pdf" --pages . 2,3,1,4 -- "$OUT/hl-third.pdf"; launch "$OUT/hl-third.pdf"; sleep 1
click 153 106 1; NT=$(ocr 260x300+0+130)
check "the Notes panel shows a highlight's words though its page was never shown ($NT)" "echo \"$NT\" | grep -qi 'quick'"
quit

# 33. More (1127,61) ▸ Two pages (1008,239) puts a red page 1 left of a blue page 2; More ▸ Right to left
# (1008,295) swaps them (Acrobat's right-to-left page order).
convert -size 300x400 xc:'#E02020' xc:'#2040E0' "$OUT/rl.pdf"; launch "$OUT/rl.pdf"; sleep 1
colour_at() { import -window root -crop "1x1+$1+420" txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}'; }
click 1127 61 0.8; click 1008 239 1.2; L0=$(colour_at 620); R0=$(colour_at 900)
click 1127 61 0.8; click 1008 295 1.2; L1=$(colour_at 620); R1=$(colour_at 900)
reddish() { python3 -c 'import sys; v = sys.argv[1].lstrip("#"); r, b = int(v[0:2], 16), int(v[4:6], 16); sys.exit(0 if r > b + 80 else 1)' "$1"; }
check "two pages read left to right, then right to left ($L0 $R0 → $L1 $R1)" "reddish $L0 && reddish $R1 && ! reddish $R0 && ! reddish $L1"
quit

# 34. Pages copied between documents: rl.pdf's red page 1 (thumbnail 120,230) ⌘C, then in four.pdf (page 1
# selected) ⌘V puts it second; saved, four.pdf has five pages, the second red.
cp "$OUT/text.pdf" "$OUT/four.pdf"; launch "$OUT/rl.pdf" "$OUT/four.pdf"; sleep 3
read RX _ <<< "$(at rl.pdf 1280x40+0+0)"; xdotool mousemove "${RX:-0}" 21 click 1; sleep 1; click 120 230 0.6; key ctrl+c
read FX _ <<< "$(at four 1280x40+0+0)"; xdotool mousemove "${FX:-0}" 21 click 1; sleep 1; click 120 230 0.6; key ctrl+v; sleep 1.5; key ctrl+s; sleep 1.5
P2=$(pdftoppm -r 10 -f 2 -l 2 -png "$OUT/four.pdf" 2>/dev/null | convert - -resize 1x1\! txt:- | grep -o '#[0-9A-F]\{6\}')
check "pages copied from one document are pasted into another ($(npages $OUT/four.pdf) pages, page 2 ${P2:-?})" "[ \"\$(npages $OUT/four.pdf)\" = 5 ] && reddish ${P2:-#000000}"
quit

# 35. Pages panel ▸ Number pages… (188,778): page 1 selected, "i, ii, iii" (the style list, then its second
# entry), Number: the saved file labels its pages i to iv (/PageLabels, style /r).
cp "$OUT/text.pdf" "$OUT/numbered.pdf"; launch "$OUT/numbered.pdf"; sleep 1
click 120 230 0.6; click 188 778 0.8; read CX CY <<< "$(at "1," 260x160+0+600)"; [ -n "$CY" ] && click "$CX" "$CY" 0.6
read RX RY <<< "$(at "i," 260x300+0+500)"; [ -n "$RY" ] && click "$RX" "$RY" 0.6
NB=$(at Number 260x60+0+680); [ -n "$NB" ] && click $NB 1; key ctrl+s; sleep 1.5
check "Number pages labels the pages in the style chosen (${NB:-no Number button})" "qpdf --qdf --object-streams=disable $OUT/numbered.pdf - 2>/dev/null | grep -aq '/S /r'"
quit

# 36. Preferences (67,193 in Home's menu) ▸ Accent colour blue (the second swatch, 38 px right of the first,
# under its label) and Background behind pages black (the fifth): kept in the preferences, and a document
# opened then shows black around its page (zoomed out, ⌘-).
launch; click 67 193 1
read AX AY <<< "$(at Accent 1100x700+180+60)"; read SX _ <<< "$(at System 1100x700+180+60)"
SW0=$(( ${SX:-0} - 44 + 14 )); [ -n "$AY" ] && click $(( SW0 + 38 )) $(( AY + 30 )) 0.5
read BX BY <<< "$(at Background 1100x700+180+60)"; [ -n "$BY" ] && click $(( SW0 + 4 * 38 )) $(( BY + 30 )) 0.5; quit
cp "$OUT/text.pdf" "$OUT/backdrop.pdf"; launch "$OUT/backdrop.pdf"; sleep 1; key ctrl+minus; key ctrl+minus; sleep 1
BD=$(import -window root -crop 1x1+300+500 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}')
check "Preferences keep the accent and the background behind pages (${BD:-?}; $(grep -o '"accent": [0-9]' $OUT/profile/data/Sheaf/prefs.json))" "grep -q '\"accent\": 1' $OUT/profile/data/Sheaf/prefs.json && [ \"$BD\" = '#000000' ]"
quit
launch; click 67 193 1; read SX _ <<< "$(at System 1100x700+180+60)"; read AX AY <<< "$(at Accent 1100x700+180+60)"; read BX BY <<< "$(at Background 1100x700+180+60)"
SW0=$(( ${SX:-0} - 44 + 14 )); [ -n "$AY" ] && click $SW0 $(( AY + 30 )) 0.5; [ -n "$BY" ] && click $SW0 $(( BY + 30 )) 0.5; quit   # back to the defaults

# 37. A signature used once: Signature tool (S) ▸ New signature (412,109), a click on the page opens the pad;
# drawn, "Keep for next time" unticked, Use (818,555): placed and saved, the kept signatures as they were.
cp "$OUT/text.pdf" "$OUT/once.pdf"; launch "$OUT/once.pdf"; sleep 1; K0=$(sigs); N0=$(annots "$OUT/once.pdf")
key s; click 412 109 0.8; click 700 620 1
xdotool mousemove 520 420 mousedown 1; for i in $(seq 0 20); do xdotool mousemove $((520 + i * 12)) $((420 + (i % 3) * 8)); done; xdotool mouseup 1; sleep 0.3
KP=$(at Keep 1280x800+0+0); [ -n "$KP" ] && click $KP 0.3; click 818 555 1.2; key ctrl+s; sleep 1.5
K1=$(sigs); N1=$(annots "$OUT/once.pdf")
check "a signature used once is placed and not kept (${KP:-no Keep}; signatures $K0 → $K1, marks $N0 → $N1)" "[ -n \"$KP\" ] && [ \"$K0\" = \"$K1\" ] && [ $N1 -gt $N0 ]"
quit

# 38. More (1127,61) ▸ Page grid (1008,267): four pages (red, green, blue, yellow) side by side in one row; a click
# selects the blue one, Delete takes it out (saved: three pages, no blue); a double-click on the red one closes
# the grid on it (it fills the width again).
convert -size 300x400 xc:'#E02020' xc:'#20A020' xc:'#2040E0' xc:'#E0C020' "$OUT/grid.pdf"; launch "$OUT/grid.pdf"; sleep 1
click 1127 61 0.8; click 1008 267 1.5
# The x of the first pixel of colour channel $1 (r, g or b) dominant on row y = $2.
first_x() { import -window root -crop "1280x1+0+$2" txt:- 2>/dev/null | python3 -c '
import re, sys
want = sys.argv[1]
for line in sys.stdin:
    m = re.match(r"(\d+),0: \((\d+),(\d+),(\d+)", line)
    if not m: continue
    x, r, g, b = map(int, m.groups())
    c = {"r": r, "g": g, "b": b}
    if c[want] > 170 and all(c[k] < 110 for k in c if k != want): print(x); break' "$1"; }
GY=226; GR=$(first_x r $GY); GB=$(first_x b $GY); GG=$(first_x g $GY)
[ -n "$GB" ] && click $(( GB + 20 )) $GY 0.6; key Delete; sleep 1.5; key ctrl+s; sleep 1.5
NB=$(npages "$OUT/grid.pdf")
[ -n "$GR" ] && xdotool mousemove $(( GR + 20 )) $GY click --repeat 2 --delay 120 1; sleep 1.5; WIDE=$(first_x r 450)
check "the page grid shows the pages in a row, Delete takes out the one selected, a double-click opens one (red $GR, green $GG, blue $GB; $NB pages; red at x ${WIDE:-?} on row 450)" "[ -n \"$GR\" ] && [ -n \"$GB\" ] && [ $GR -lt ${GG:-0} ] && [ ${GG:-0} -lt $GB ] && [ \"$NB\" = 3 ] && [ -n \"$WIDE\" ] && [ $WIDE -lt 400 ]"
quit

# 39. Rotate: a portrait page's card shows it turned as chosen: wider than tall at Right 90° (the default),
# taller than wide again at 180° (found by its label).
convert -size 300x420 xc:'#2040E0' "$OUT/turn.pdf"; launch "$OUT/turn.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "rotate"; key Return; sleep 2.5
cardbox() { import -window root -crop 600x300+40+100 png:- 2>/dev/null | convert - -fuzz 20% -fill white +opaque '#2040E0' -trim -format '%w %h' info: 2>/dev/null; }
read CW0 CH0 <<< "$(cardbox)"; HF=$(at 180 300x500+990+90); [ -n "$HF" ] && click $HF 1; read CW1 CH1 <<< "$(cardbox)"
check "Rotate shows each file's page turned as chosen (${CW0:-?}x${CH0:-?} → ${CW1:-?}x${CH1:-?})" "[ -n \"$CW1\" ] && [ ${CW0:-0} -gt ${CH0:-0} ] && [ ${CH1:-0} -gt ${CW1:-0} ]"
quit

# 40. Crop: a box dragged on the page preview (found by its accent outline's place, the preview under the margin
# fields) fills the four margins; the Top field then holds a number.
cp "$OUT/text.pdf" "$OUT/crop.pdf"; launch "$OUT/crop.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "crop"; key Return; sleep 2.5
read _ MY <<< "$(at Margins 300x600+990+100)"
if [ -n "$MY" ]; then PY=$(( MY + 190 )); xdotool mousemove 1100 $PY mousedown 1 mousemove 1120 $(( PY + 20 )) mousemove 1150 $(( PY + 60 )) mouseup 1; sleep 0.8; fi
read _ TY <<< "$(at Top 300x600+990+100)"; CT=$(ocr 120x30+1000+$(( ${TY:-200} + 8 )) | tr -cd '0-9.')
check "a box dragged on Crop's preview sets the margins (top: ${CT:-none})" "[ -n \"$CT\" ] && [ \"$CT\" != 0 ]"
quit

