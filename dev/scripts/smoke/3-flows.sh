# Sourced by dev/scripts/gui-smoke.sh, in name order: shares its helpers, profile and variables.
# Steps 21–78: tool flows (reorder, Merge, Split, Compare, Protect) and the later reader features.

# 21. A tool's files reorder by drag (iLovePDF, Stirling): three images added to JPG to PDF (Tools 67,113 ▸ JPG to PDF
# 271,517 ▸ Open file 494,405, the dialog answered with the three; images opened from outside go to the reader since
# C-reader-064), a.png's card dragged past c.png's (cards at 101/237/373, 130); the dragged card dims, a bar shows
# where it goes; the order becomes b c a.
mkdir -p "$OUT/reo"; convert -size 300x400 xc:'#3366cc' "$OUT/reo/a.png"; convert -size 300x400 xc:'#cc6633' "$OUT/reo/b.png"; convert -size 300x400 xc:'#33aa55' "$OUT/reo/c.png"
jpg_to_pdf "$OUT/reo/a.png" "$OUT/reo/b.png" "$OUT/reo/c.png"
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
jpg_to_pdf "$OUT"/reo/f*.png
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

# 21e. Merge ▸ Pages (Acrobat's Combine Files ▸ Expand): pa.pdf (red green blue yellow) and pb.pdf (black grey light)
# opened out into 7 page cards, found by their labels ("pa.pdf · 1"); the first dropped with its ×, the last moved
# left with its arrow: the merge is pa 2-4, pb 1, pb 3, pb 2.
cp "$OUT/reo/ra.pdf" "$OUT/reo/pa.pdf"; cp "$OUT/reo/rb.pdf" "$OUT/reo/pb.pdf"
launch "$OUT/reo/pa.pdf" "$OUT/reo/pb.pdf"
click 50 21 1; click 67 113 1; click 271 197 1.5; click 494 405 1; click 460 527 2
PG=$(at Pages 280x400+1000+60); [ -n "$PG" ] && click $PG 2; shot 21e-pages
# Page cards are 108 px wide, 124 apart, from x 41, y 79 (162 high); hovered, × at +93,+15 and ← at +15,+143.
# Seven cards: six in the first row, one below; labels read "pa.pdf · 1" (11 px, read at 3×).
NC=$(import -window root -crop 940x380+20+200 png:- 2>/dev/null | convert - -resize 300% png:- | tesseract stdin stdout 2>/dev/null | grep -o 'p[ab]\.pdf' | wc -l)
xdotool mousemove 95 140; sleep 0.4; click 134 94 1
xdotool mousemove $(( 41 + 124 * 5 + 54 )) 140; sleep 0.4; click $(( 41 + 124 * 5 + 15 )) 222 1
click 1133 755 4
px() { pdftoppm -r 10 -f $2 -l $2 -png "$1" 2>/dev/null | convert - -format "%[pixel:p{5,5}] " info: 2>/dev/null; }
WANT="$(px $OUT/reo/pa.pdf 2)$(px $OUT/reo/pa.pdf 3)$(px $OUT/reo/pa.pdf 4)$(px $OUT/reo/pb.pdf 1)$(px $OUT/reo/pb.pdf 3)$(px $OUT/reo/pb.pdf 2)"
GOT=$(for i in 1 2 3 4 5 6 7; do px "$OUT/reo/pa-merged.pdf" $i; done)
check "Merge's Pages view merges the pages in the order arranged (${PG:-no Pages}; $NC labels; $GOT)" "[ \"$GOT\" = \"$WANT\" ]"
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
SF=$(ocr 60x26+1004+48 | tr -d ' '); RN=$(ocr 240x24+0+166)
check "a recent search runs again from the list ($SF; $RN)" "echo \"$SF\" | grep -q '^fox' && echo \"$RN\" | grep -q '4 results'"
quit

# 24. More ▸ Highlight form fields (1127,61 ▸ "form" 1008,211) turns the fields' tint off and on (Acrobat's
# Highlight existing fields); Document properties of a file with only an owner password say what it forbids.
launch "$OUT/form.pdf"; key ctrl+9; sleep 1
tint() { import -window root -crop 1x1+640+300 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}'; }
T0=$(tint); click 1108 61 0.8; click 1008 211 1; T1=$(tint); click 1108 61 0.8; click 1008 211 1; T2=$(tint)
check "Highlight form fields turns the fields' tint off and on ($T0 → $T1 → $T2)" "[ \"$T0\" != \"$T1\" ] && [ \"$T0\" = \"$T2\" ] && [ \"$T1\" = '#FFFFFF' ]"
quit
qpdf --encrypt "" owner 256 --print=none --extract=n -- "$OUT/text.pdf" "$OUT/owner.pdf"
# The sheet's fields scroll within the window: the About lines are further down.
launch "$OUT/owner.pdf"; key ctrl+d; sleep 2; xdotool mousemove 640 400; for _ in $(seq 1 12); do xdotool click 5; sleep 0.05; done; sleep 0.8
PR=$(ocr 440x700+420+50)
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
# Run with Overlay: the result card's open icon (167,236) shows the marked copy in the reader, its differences listed in the Notes panel
# (the two pages differ by colour only: one changed area).
OV=$(at Overlay 280x700+1000+90); [ -n "$OV" ] && click $OV 1
# Compare PDF (1134,756), its result waited for (a second press if the first was not taken); where to save
# it is answered in case the profile asks.
answer "$PWD/$OUT/reo/compared.pdf"
for try in 1 2; do
    click 1134 756 1; for _ in $(seq 1 20); do ocr 700x60+100+712 | grep -q successfully && break 2; sleep 0.5; done
done
answer ""; click 167 236 3; shot 26-compare-open   # the result card's open icon
NL=$(ocr 245x600+0+130)
check "Compare's marked copy opens in the reader, its differences listed (${NL:0:60})" "echo \"$NL\" | grep -qi 'changed area'"
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

# The end of the Preferences page (Documents is below Appearance, Identity and output).
prefs_end() { xdotool mousemove 700 500 click --repeat 12 5; sleep 0.6; }
# 28. Preferences ▸ Open new documents at Actual size (Preferences in Home's menu, found by its word: Library
# and Actions came above it): a file never opened shows at 100 %.
prefs_open() { read PX PY <<< "$(at Preferences 196x400+0+40)"; click "${PX:-0}" "${PY:-0}" 1; }
# Preferences scrolled until "Open new documents at"'s row shows (Documents is no longer the last section).
prefs_documents() { for _ in $(seq 1 20); do [ -n "$(at Actual 900x700+200+90)" ] && return; xdotool mousemove 700 500 click --repeat 2 5; sleep 0.3; done; }
launch; prefs_open; prefs_documents; AS=$(at Actual 900x700+200+90); [ -n "$AS" ] && click $AS 0.6; quit
cp "$OUT/text.pdf" "$OUT/fresh.pdf"; launch "$OUT/fresh.pdf"; sleep 1.5; ZF=$(zoomval)
check "a new document opens at the zoom chosen in Preferences (Actual size: $ZF %)" "[ \"$ZF\" = 100 ]"
quit
# Fit page: a whole letter page in the 700 px high view, well under fit width (163 %).
launch; prefs_open; prefs_documents; FP2=$(import -window root -crop 900x700+200+90 png:- | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' '$12=="page" && prev=="Fit"{printf "%d %d\n", 200+($7+$9/2)/2, 90+($8+$10/2)/2; exit} {prev=$12}'); [ -n "$FP2" ] && click $FP2 0.6; quit
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
DS=$(at "Don" 700x300+290+250)
# OCR sometimes misses "Don’t Save": it sits 153 px left of Cancel on the same row.
[ -n "$DS" ] || { read CX CY <<< "$(at Cancel 700x300+290+250)"; [ -n "$CY" ] && DS="$(( CX - 153 )) $CY"; }
[ -n "$DS" ] && click $DS 1
# The tab goes once the prompt is answered: up to 5 s under load.
for _ in 1 2 3 4 5 6 7 8 9 10; do [ "$(ntabs)" = 1 ] && break; sleep 0.5; done
check "Close Other Tabs asks about unsaved edits, Don't Save closes it ($(echo $PQ | cut -c1-60); $(ntabs) tab)" "echo \"$PQ\" | grep -q cb.pdf && [ \"\$(ntabs)\" = 1 ]"
quit

# 30. Pages panel ▸ Extract… (156,778) opens its options strip, Acrobat's: one file per page, delete the
# pages after. Deleting the only page is refused before any file is asked for.
qpdf "$OUT/text.pdf" --pages . 1 -- "$OUT/single.pdf"; launch "$OUT/single.pdf"; sleep 1
click 156 778 0.8; XS=$(ocr 260x110+0+620)
click 17 704 0.3; click 201 734 0.6   # "Delete pages after extracting", then Extract
XT=$(import -window root png:- | convert - -resize 200% png:- | tesseract stdin stdout 2>/dev/null | tr '\n' ' ')
check "Extract offers one file per page and deleting the pages ($XS)" "echo \"$XS\" | grep -q 'One file per page' && echo \"$XS\" | grep -q 'Delete pages'"
check "extracting and deleting the only page is refused" "echo \"$XT\" | grep -q 'needs at least one page' && [ \"\$(npages $OUT/single.pdf)\" = 1 ]"
quit

# 31. The side panel's edge (247,400) dragged 100 px right: the first thumbnail (row y 230) widens
# from 150 to 250 px, and the width is kept for the next start (PDF Expert's sidebar).
# The width of the first blue run on row 230: the first thumbnail of a blue page.
thumbw() { import -window root -crop 500x1+0+230 png:- 2>/dev/null | convert - txt:- | awk -F'[(),]' 'NR > 1 { b = ($5 + 0 > 180 && $3 + 0 < 90); if (b) n++; else if (n > 20) { print n; exit } else n = 0 }' | head -1; }
convert -size 300x400 xc:'#2040E0' "$OUT/blue.pdf"; launch "$OUT/blue.pdf"; sleep 1.5; TW0=$(thumbw)
xdotool mousemove 247 400 mousedown 1; for i in $(seq 1 10); do xdotool mousemove $(( 247 + i * 10 )) 400; sleep 0.04; done; xdotool mouseup 1; sleep 1.5; TW1=$(thumbw); quit
launch "$OUT/blue.pdf"; sleep 1.5; TW2=$(thumbw)
quit
# Back to the default panel for the later steps.
python3 -c 'import json, sys; p = json.load(open(sys.argv[1])); p["panel_width"] = 0; json.dump(p, open(sys.argv[1], "w"))' "$OUT/profile/data/Sheaf/prefs.json"
check "the side panel's edge resizes it, its thumbnails follow, kept on restart ($TW0 → $TW1 → $TW2 px)" "[ \"\${TW0:-0}\" -gt 130 ] && [ \"\${TW0:-0}\" -lt 160 ] && [ \"\${TW1:-0}\" -gt 230 ] && [ \"\${TW1:-0}\" -lt 260 ] && [ \"\${TW2:-x}\" = \"\${TW1:-y}\" ]"

# 32. A highlight saved without a note, on a page never shown: the Notes panel (153,106) lists the
# words it covers (step 1's highlight, its page moved third; made as step 1 does when this part runs alone).
[ -f "$OUT/highlighted.pdf" ] || { cp "$OUT/text.pdf" "$OUT/highlighted.pdf"; launch "$OUT/highlighted.pdf"; key ctrl+9
    xdotool mousemove 330 200 mousedown 1 mousemove 600 201 mousemove 900 201 mouseup 1; sleep 0.4; key h; sleep 1; key ctrl+s; sleep 1.5; quit; }
qpdf "$OUT/highlighted.pdf" --pages . 2,3,1,4 -- "$OUT/hl-third.pdf"; launch "$OUT/hl-third.pdf"; sleep 1
click 153 106 1; NT=$(ocr 260x300+0+130)
check "the Notes panel shows a highlight's words though its page was never shown ($NT)" "echo \"$NT\" | grep -qi 'quick'"
quit

# 33. More (1127,61) ▸ Two pages (1008,239) puts a red page 1 left of a blue page 2; More ▸ Right to left
# (1008,295) swaps them (Acrobat's right-to-left page order).
convert -size 300x400 xc:'#E02020' xc:'#2040E0' "$OUT/rl.pdf"; launch "$OUT/rl.pdf"; sleep 1
colour_at() { import -window root -crop "1x1+$1+420" txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}'; }
click 1108 61 0.8; click 1008 239 1.2; L0=$(colour_at 620); R0=$(colour_at 900)
click 1108 61 0.8; click 1008 295 1.2; L1=$(colour_at 620); R1=$(colour_at 900)
reddish() { python3 -c 'import sys; v = sys.argv[1].lstrip("#"); r, b = int(v[0:2], 16), int(v[4:6], 16); sys.exit(0 if r > b + 80 else 1)' "$1"; }
check "two pages read left to right, then right to left ($L0 $R0 → $L1 $R1)" "reddish '${L0:-#000000}' && reddish '${R1:-#000000}' && ! reddish '${R0:-#FF0000}' && ! reddish '${L1:-#FF0000}'"
quit

# 34. Pages copied between documents: rl.pdf's red page 1 (thumbnail 120,230) ⌘C, then in four.pdf (page 1
# selected) ⌘V puts it second; saved, four.pdf has five pages, the second red.
cp "$OUT/text.pdf" "$OUT/four.pdf"; launch "$OUT/rl.pdf" "$OUT/four.pdf"; sleep 3
read RX _ <<< "$(at rl.pdf 1280x40+0+0)"; xdotool mousemove "${RX:-0}" 21 click 1; sleep 1; click 120 230 0.6; key ctrl+c
read FX _ <<< "$(at four 1280x40+0+0)"; xdotool mousemove "${FX:-0}" 21 click 1; sleep 1; click 120 230 0.6; key ctrl+v; sleep 1.5; key ctrl+s; sleep 1.5
P2=$(pdftoppm -r 10 -f 2 -l 2 -png "$OUT/four.pdf" 2>/dev/null | convert - -resize 1x1\! txt:- | grep -o '#[0-9A-F]\{6\}')
check "pages copied from one document are pasted into another ($(npages $OUT/four.pdf) pages, page 2 ${P2:-?})" "[ \"\$(npages $OUT/four.pdf)\" = 5 ] && reddish '${P2:-#000000}'"
quit

# 35. Pages panel ▸ Number pages… (188,778): page 1 selected, "i, ii, iii" (the style list, then its second
# entry), Number: the saved file labels its pages i to iv (/PageLabels, style /r).
cp "$OUT/text.pdf" "$OUT/numbered.pdf"; launch "$OUT/numbered.pdf"; sleep 1
click 120 230 0.6; click 188 778 0.8; click 62 665 0.8; click 40 661 0.6   # the style list, then "i, ii, iii"
NB="198 732"; click $NB 1; key ctrl+s; sleep 1.5
check "Number pages labels the pages in the style chosen (${NB:-no Number button})" "qpdf --qdf --object-streams=disable $OUT/numbered.pdf - 2>/dev/null | grep -aq '/S /r'"
quit
# 35c. More (1127,61) ▸ Page background… (13th item, 1008,435): pale yellow chosen (the first colour), Apply,
# saved: the pages' corners turn pale yellow, behind the text (a /Background artifact).
cp "$OUT/text.pdf" "$OUT/bg.pdf"; launch "$OUT/bg.pdf"; sleep 1
click 1108 61 0.8; click 1008 435 1; shot 35c-background; BG=$(read CX CY <<< "$(at Cancel 700x500+300+150)"; [ -n "$CY" ] && echo "$(( CX + 70 )) $CY"); [ -n "$BG" ] && click $BG 1.5; key ctrl+s; sleep 1.5
BC=$(pdftoppm -r 10 -f 1 -l 1 -png "$OUT/bg.pdf" 2>/dev/null | convert - -format "%[pixel:p{2,2}]" info: 2>/dev/null)
check "Page background puts a colour behind every page (${BG:-no Apply}; corner $BC)" "[ \"$BC\" = 'srgb(255,248,214)' ] && qpdf --qdf --object-streams=disable $OUT/bg.pdf - 2>/dev/null | grep -aq '/Background'"
quit
# 35d. More ▸ Versions… (14th item, 1008,463), Acrobat's Revert: a background applied and not saved, then Revert to
# saved: the page is white again and nothing is left to save; Open on version 1 opens it in a tab of its own.
cp "$OUT/text.pdf" "$OUT/ver.pdf"; launch "$OUT/ver.pdf"; sleep 1
click 1108 61 0.8; click 1008 435 1; AP=$(read CX CY <<< "$(at Cancel 700x500+300+150)"; [ -n "$CY" ] && echo "$(( CX + 70 )) $CY"); [ -n "$AP" ] && click $AP 1.5
click 1108 61 0.8; click 1008 463 1; shot 35d-versions; RV=$(at Revert 700x500+300+150); [ -n "$RV" ] && click $RV 2
VC=$(import -window root -crop 1x1+700+700 txt:- 2>/dev/null | tail -1 | awk '{ print $3 }')
click 1108 61 0.8; click 1008 463 1; OP=$(at Open 700x500+300+150); [ -n "$OP" ] && click $OP 2.5
check "Versions: Revert drops the unsaved background, Open shows version 1 in a tab (${RV:-no Revert} ${OP:-no Open}; page $VC; $(ntabs) tabs)" "echo \"$VC\" | grep -qi 'FFFFFF\|white' && [ \"\$(ntabs)\" = 2 ]"
quit
# 35b. A thumbnail's right-click menu (Acrobat's, Preview's): Reverse page order turns text.pdf's four pages back to
# front; Insert pages from file… (single.pdf, page 1 of text.pdf) adds one after page 1; Replace with a file's
# pages… puts it in page 1's place (the one right-clicked). Saved: five pages, the first two single.pdf's.
# ("Insert blank page after" comes first: the item is found by its "file…".)
cp "$OUT/text.pdf" "$OUT/pagemenu.pdf"; launch "$OUT/pagemenu.pdf"; sleep 1
xdotool mousemove 120 230 click 3; sleep 0.8; shot 35b-menu; RV=$(at Reverse 300x400+20+200); [ -n "$RV" ] && click $RV 1.5
answer "$PWD/$OUT/single.pdf"; xdotool mousemove 120 230 click 3; sleep 0.8; IN=$(at file 300x400+20+200); [ -n "$IN" ] && click $IN 1.5
xdotool mousemove 120 230 click 3; sleep 0.8; RP=$(at Replace 300x400+20+200); [ -n "$RP" ] && click $RP 1.5; answer ""; key ctrl+s; sleep 1.5
PM=$(for i in 1 2 3 4 5; do echo -n "$(first_text "$OUT/pagemenu.pdf" $i)|"; done)
check "A thumbnail's menu reverses the pages, inserts and replaces from a file ($PM)" "[ \"$PM\" = 'Page 1|Page 1|Page 3|Page 2|Page 1|' ]"
quit

# 36. Preferences (in Home's menu) ▸ Accent colour blue (the second swatch, 38 px right of the first,
# under its label) and Background behind pages black (the fifth): kept in the preferences, and a document
# opened then shows black around its page (zoomed out, ⌘-).
launch; prefs_open
read AX AY <<< "$(at Accent 1100x700+180+60)"; read SX _ <<< "$(at System 1100x700+180+60)"
SW0=$(( ${SX:-0} - 44 + 14 )); [ -n "$AY" ] && click $(( SW0 + 38 )) $(( AY + 30 )) 0.5
read BX BY <<< "$(at Background 1100x700+180+60)"; [ -n "$BY" ] && click $(( SW0 + 4 * 38 )) $(( BY + 30 )) 0.5; quit
cp "$OUT/text.pdf" "$OUT/backdrop.pdf"; launch "$OUT/backdrop.pdf"; sleep 1; key ctrl+minus; key ctrl+minus; sleep 1
BD=$(import -window root -crop 1x1+300+500 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}')
check "Preferences keep the accent and the background behind pages (${BD:-?}; $(grep -o '"accent": [0-9]' $OUT/profile/data/Sheaf/prefs.json))" "grep -q '\"accent\": 1' $OUT/profile/data/Sheaf/prefs.json && [ \"$BD\" = '#000000' ]"
quit
launch; prefs_open; read SX _ <<< "$(at System 1100x700+180+60)"; read AX AY <<< "$(at Accent 1100x700+180+60)"; read BX BY <<< "$(at Background 1100x700+180+60)"
SW0=$(( ${SX:-0} - 44 + 14 )); [ -n "$AY" ] && click $SW0 $(( AY + 30 )) 0.5; [ -n "$BY" ] && click $SW0 $(( BY + 30 )) 0.5; quit   # back to the defaults

# 37. A signature used once: Signature tool (S) ▸ New signature (412,109), a click on the page opens the pad;
# drawn, "Keep for next time" unticked, Use (818,555): placed and saved, the kept signatures as they were.
cp "$OUT/text.pdf" "$OUT/once.pdf"; launch "$OUT/once.pdf"; sleep 1; K0=$(sigs); N0=$(annots "$OUT/once.pdf")
# "New signature" moves right as kept signatures add thumbnails before it.
key s; sleep 0.5; NS=$(at New 1280x40+0+90); click ${NS:-412 109} 0.8; click 700 620 1
xdotool mousemove 520 420 mousedown 1; for i in $(seq 0 20); do xdotool mousemove $((520 + i * 12)) $((420 + (i % 3) * 8)); done; xdotool mouseup 1; sleep 0.3
# Untick "Keep for next time", then Use.
KP="511 555"; click $KP 0.3; click 818 555 1.2; key ctrl+s; sleep 1.5
K1=$(sigs); N1=$(annots "$OUT/once.pdf")
check "a signature used once is placed and not kept (${KP:-no Keep}; signatures $K0 → $K1, marks $N0 → $N1)" "[ \"${K0%% *}\" = \"${K1%% *}\" ] && [ $N1 -gt $N0 ]"
quit

# 38. More (1127,61) ▸ Page grid (1008,267): four pages (red, green, blue, yellow) side by side in one row; a click
# selects the blue one, Delete takes it out (saved: three pages, no blue); a double-click on the red one closes
# the grid on it (it fills the width again).
convert -size 300x400 xc:'#E02020' xc:'#20A020' xc:'#2040E0' xc:'#E0C020' "$OUT/grid.pdf"; launch "$OUT/grid.pdf"; sleep 1
click 1108 61 0.8; click 1008 267 1.5
# The x of the first pixel where colour channel $1 (r, g or b) dominates on row y = $2, right of the panel.
first_x() { import -window root -crop "1030x1+250+$2" txt:- 2>/dev/null | python3 -c '
import re, sys
want = "rgb".index(sys.argv[1])
for line in sys.stdin:
    # The 8-bit srgb() field: the hex is 16-bit per channel when read from the X screen.
    m = re.match(r"(\d+),0: .*srgb\((\d+),(\d+),(\d+)", line)
    if not m: continue
    c = [int(m.group(k)) for k in (2, 3, 4)]
    if c[want] > 150 and all(c[k] < 110 for k in range(3) if k != want): print(250 + int(m.group(1))); break' "$1"; }
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

# 40. Crop: a box dragged on the page preview (under the margin fields) fills the four margins in mm; the Top
# field changes to the box's.
cp "$OUT/text.pdf" "$OUT/crop.pdf"; launch "$OUT/crop.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "crop"; key Return; sleep 2.5
# Placed from the Top label (the cards above it vary): its field 18 px under it, the page preview from 129 px
# under it to 329, x 1051–1205; a box from inside its top-left to near its bottom-right.
read _ TY <<< "$(at Top 300x700+990+90)"; TY=${TY:-327}
topval() { ocr 110x30+1006+$(( TY + 18 )) | tr -cd '0-9.'; }
CT0=$(topval)
xdotool mousemove 1075 $(( TY + 163 )) mousedown 1 mousemove 1110 $(( TY + 213 )) mousemove 1180 $(( TY + 293 )) mouseup 1; sleep 0.8
CT=$(topval)
check "a box dragged on Crop's preview sets the margins (top: ${CT0:-?} → ${CT:-none})" "[ -n \"$CT\" ] && [ \"$CT\" != \"$CT0\" ]"
# Under the preview, ‹ Page 1 of 4 › (centred, its › at 1192): page 2 shows.
xdotool mousemove 1130 500; for i in 1 2 3 4 5 6; do xdotool click 5; sleep 0.05; done; sleep 0.6
read _ PY <<< "$(at Page 280x700+995+90)"; PY=${PY:-700}   # "of" alone is too short for OCR
PV0=$(ocr 290x40+990+$(( PY - 20 ))); click 1192 $PY 1.5; PV1=$(ocr 290x40+990+$(( PY - 20 ))); shot 40b-crop-page
check "Crop's preview steps to another page ($PV0 → $PV1)" "! echo \"$PV0\" | grep -q 'Page 2 of 4' && echo \"$PV1\" | grep -q 'Page 2 of 4'"
quit


# 41. Shapes (R) ▸ Cloud: three clicks and a double-click on the page draw a cloud (Acrobat's Cloud tool); saved,
# the file has a /Polygon with the cloudy border's intent.
cp "$OUT/text.pdf" "$OUT/cloud.pdf"; launch "$OUT/cloud.pdf"; sleep 1; key r; sleep 0.5
CL=$(at Cloud 1280x75+0+90); [ -n "$CL" ] && click $CL 0.5
click 500 300 0.3; click 700 320 0.3; click 650 480 0.3; xdotool mousemove 520 470 click --repeat 2 --delay 120 1; sleep 1.2; key ctrl+s; sleep 1.5
QC=$(qpdf --qdf --object-streams=disable "$OUT/cloud.pdf" - 2>/dev/null)
check "Shapes ▸ Cloud draws a cloud from clicked vertices (${CL:-no Cloud button})" "echo \"\$QC\" | grep -aq '/Subtype /Polygon' && echo \"\$QC\" | grep -aq '/PolygonCloud'"
quit

# 42. Shapes (R) ▸ Distance (the options bar's second row, with Cloud): two clicks measure the line between them (Acrobat's Measuring tool); saved, the
# file has a measuring line (/Measure) labelled in cm.
cp "$OUT/text.pdf" "$OUT/measure.pdf"; launch "$OUT/measure.pdf"; sleep 1; key r; sleep 0.5
DS=$(at Distance 1280x75+0+90); [ -n "$DS" ] && click $DS 0.5
click 400 600 0.4; click 700 600 1.2; key ctrl+s; sleep 1.5
QM=$(qpdf --qdf --object-streams=disable "$OUT/measure.pdf" - 2>/dev/null)
check "Shapes ▸ Distance measures between two clicks (${DS:-no Distance button})" "echo \"\$QM\" | grep -aq '/Measure' && echo \"\$QM\" | grep -aq ' cm'"
# Calibrate (Acrobat's, Bluebeam's): a 300 px line said to be 3 m; a line half as long then reads 1.50 m (both from
# x 560: on the page at Fit page too, where it starts at 504).
CB=$(at Calibrate 1280x75+0+90); [ -n "$CB" ] && click $CB 0.5
click 560 650 0.4; click 860 650 1.2; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 20 "3 m"; key Return; sleep 0.8
click 560 700 0.4; click 710 700 1.2; key ctrl+s; sleep 1.5
QC=$(qpdf --qdf --object-streams=disable "$OUT/measure.pdf" - 2>/dev/null)
check "Calibrate sets the scale from a known length (${CB:-no Calibrate button}; $(echo "$QC" | grep -ao '/Contents ([^)]*)' | tail -3 | tr '\n' ' '))" "echo \"\$QC\" | grep -aq '1.50 m'"
quit

# 43. Stamp (M) ▸ Image…: a picture chosen (the dialog answered with stamp.png) is placed by a click (Acrobat's
# custom stamp); saved, the file has a stamp drawing that picture; the picture is kept for next time.
convert -size 80x40 xc:'#2a9d4b' "$OUT/stamp.png"; cp "$OUT/text.pdf" "$OUT/stamped.pdf"; launch "$OUT/stamped.pdf"; sleep 1; key m; sleep 0.5
# Image… is right of the stamp list.
answer "$PWD/$OUT/stamp.png"; IM="515 105"; click $IM 0.8; answer ""
click 640 420 1.2; key ctrl+s; sleep 1.5
QS=$(qpdf --qdf --object-streams=disable "$OUT/stamped.pdf" - 2>/dev/null)
check "Stamp ▸ Image… places a picture as a stamp (${IM:-no Image… button})" "echo \"\$QS\" | grep -aq '/Subtype /Stamp' && echo \"\$QS\" | grep -aq '/Subtype /Image'"
# Kept for next time: the picture is in the stamp library, listed with the stamps.
check "a picture stamp is kept in the library ($(ls $OUT/profile/data/Sheaf/stamps 2>/dev/null))" "[ -f $OUT/profile/data/Sheaf/stamps/stamp.png ]"
quit

# 44. Extract ▸ One file per page, the folder chosen (the dialog answered): page 1 of a four-page file is written
# as "<name> - page 1.pdf" in it; a file of that name already there is kept, the page goes to "… (1).pdf".
mkdir -p "$OUT/each"; cp "$OUT/text.pdf" "$OUT/four-pages.pdf"; launch "$OUT/four-pages.pdf"; sleep 1
echo mine > "$OUT/each/four-pages - page 1.pdf"
click 120 230 0.6; click 156 778 0.8; click 17 674 0.3
answer "$PWD/$OUT/each"; click 201 734 2; answer ""
check "Extract writes one file per page in the folder chosen, never over a file there ($(ls $OUT/each 2>/dev/null | tr '\n' ' '))" "grep -qx mine \"$OUT/each/four-pages - page 1.pdf\" && qpdf --check \"$OUT/each/four-pages - page 1 (1).pdf\" >/dev/null 2>&1"
quit

# 45. Home ▸ New blank PDF: named in the Save dialog (the test seam answers it), one blank page, opened in a tab.
launch; answer "$PWD/$OUT/blank.pdf"; NB=$(at New 900x120+300+80); [ -n "$NB" ] && click $NB 2.5
BP=$( [ -f "$OUT/blank.pdf" ] && npages "$OUT/blank.pdf" ); answer ""
check "New blank PDF writes one blank page and opens it (${NB:-no button}; ${BP:-no file} page; $(ntabs) document tab)" "[ \"$BP\" = 1 ] && [ \"\$(ntabs)\" = 1 ]"
quit
# 45b. Home ▸ From clipboard (Acrobat's Create PDF from Clipboard): text put on the clipboard (xclip) becomes a
# new PDF, named in the Save dialog, opened in a tab.
printf 'Pasted from the clipboard' | xclip -selection clipboard; sleep 0.3
launch; answer "$PWD/$OUT/clip.pdf"; FC=$(at From 900x120+300+80); [ -n "$FC" ] && click $FC 1; for _ in $(seq 1 12); do [ "$(ntabs)" = 1 ] && break; sleep 0.5; done; answer ""
# ntabs counts document tabs: the new PDF's is the one.
check "From clipboard makes a PDF of the clipboard's text and opens it (${FC:-no button}; $(ntabs) tabs)" "pdftotext $OUT/clip.pdf - 2>/dev/null | grep -q 'Pasted from the clipboard' && [ \"\$(ntabs)\" = 1 ]"
quit

# 46. Replace text (the selection bar's Replace text…, 7th of its 30 px buttons, the bar 42 px above the line):
# "quick brown" selected, "slow" typed and ↩: saved as a caret holding "slow" and a strike-out grouped with it.
cp "$OUT/text.pdf" "$OUT/replace.pdf"
launch "$OUT/replace.pdf"; key ctrl+9; sleep 1
Q=$(span quick 1020x700+255+90); B=$(span brown 1020x700+255+90)
if [ -n "$Q" ] && [ -n "$B" ]; then
    set -- $Q; SX=$(( $1 + 1 )); SY=$3; set -- $B; EX=$(( $2 - 1 ))
    xdotool mousemove $SX $SY mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( SX + (EX - SX) * i / 5 )) $SY; sleep 0.05; done; xdotool mouseup 1; sleep 1
    shot 46-bar; click $(( SX + 197 )) $(( SY - 36 )) 1; xdotool type --delay 30 "slow"; key Return; sleep 1.5; key ctrl+s; sleep 2
fi
RQ=$(qpdf --qdf --object-streams=disable "$OUT/replace.pdf" - 2>/dev/null)
check "Replace text proposes the words typed: a caret holding them and the selection struck out, grouped" "echo \"\$RQ\" | grep -aq '/Subtype /Caret' && echo \"\$RQ\" | grep -aq '/Contents (slow)' && echo \"\$RQ\" | grep -aq '/RT /Group'"
# Insert text after… (the 8th button): "Page" selected (at the line's start: the bar is not pushed back from the
# window's edge), "red" typed: a caret holding it, nothing more struck out.
F=$(span Page 1020x700+255+90)
if [ -n "$F" ]; then
    set -- $F; FX=$(( $1 + 1 )); FY=$3; FE=$(( $2 - 1 ))
    xdotool mousemove $FX $FY mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( FX + (FE - FX) * i / 5 )) $FY; sleep 0.05; done; xdotool mouseup 1; sleep 1
    click $(( FX + 227 )) $(( FY - 36 )) 1; xdotool type --delay 30 "red"; key Return; sleep 1.5; key ctrl+s; sleep 2
fi
IQ=$(qpdf --qdf --object-streams=disable "$OUT/replace.pdf" - 2>/dev/null)
check "Insert text proposes the words after the selection, nothing struck out ($(echo "$IQ" | grep -ac '/Subtype /StrikeOut') strike-out)" "echo \"\$IQ\" | grep -aq '/Contents (red)' && [ \"\$(echo \"\$IQ\" | grep -ac '/Subtype /Caret')\" = 2 ] && [ \"\$(echo \"\$IQ\" | grep -ac '/Subtype /StrikeOut')\" = 1 ]"
quit

# 47. PDF to HTML or text (wrench, "html or text" typed), Markdown chosen: the .md holds each page's words in order.
cp "$OUT/text.pdf" "$OUT/md.pdf"; launch "$OUT/md.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "html or text"; key Return; sleep 2.5
MD=$(at Markdown 1000x700+250+90); [ -n "$MD" ] && click $MD 0.8
click 1140 755 4
check "PDF to HTML or text writes Markdown with every page in order ($(ls $OUT/md*.md 2>/dev/null))" "grep -q 'Page 1' $OUT/md.md && grep -q 'Page 4' $OUT/md.md"
quit

# 53. Prepare form (F; More ▸ Prepare form): a text field dragged 500,360 → 700,390 on a page no earlier step drew on (plain.pdf; whatever zoom the profile now opens at, ⌘9 and Home) opens its card under it (name
# above its Tooltip; Required and Done found below it): named "Full name", required. A check box (its kind at 372,109) clicked
# at 500,600 is placed selected: Delete removes it; another at 500,650 stays. Saved: the two fields, as set.
cp "$OUT/plain.pdf" "$OUT/prep.pdf"; launch "$OUT/prep.pdf"; key ctrl+9; sleep 1; key Home; sleep 0.5; key f; sleep 0.6
xdotool mousemove 500 360 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( 500 + i * 40 )) $(( 360 + i * 6 )); sleep 0.05; done; xdotool mouseup 1; sleep 1.5
# The card under the field (its rows grew with Format and Copies): the name is the field above "Tooltip".
# A moment for the name field to take the keyboard: the first letter typed at once was lost ("ull name", 28/10).
NM=$(at Tooltip 640x420+400+380); [ -n "$NM" ] && { set -- $NM; click $1 $(( $2 - 34 )) 0.8; }; xdotool key ctrl+a; sleep 0.3; xdotool type --delay 30 "Full name"
RQ=$(at Required 640x420+400+380); [ -n "$RQ" ] && click $RQ 0.3   # the whole check, its label included, toggles
DN=$(at Done 640x420+400+380); [ -n "$DN" ] && click $DN 1.2
click 372 109 0.5; click 500 600 1.2; key Delete; sleep 1; click 500 650 1.2; key ctrl+s; sleep 1.5; shot 53-prepare
PF=$(qpdf --qdf --object-streams=disable "$OUT/prep.pdf" - 2>/dev/null)
check "Prepare form places fields, names one and makes it required, and Delete removes one ($(echo "$PF" | grep -ac '/FT /Btn') check box)" "echo \"\$PF\" | grep -aq '/T (Full name)' && echo \"\$PF\" | grep -aq '/Ff 2' && [ \"\$(echo \"\$PF\" | grep -ac '/FT /Btn')\" = 1 ]"
# Several fields arranged (Acrobat's Align): a third check box at 800,650 (clear of the open card); the one at 500,650 clicked, the new one
# ⇧-clicked: the card says "2 fields selected"; its Left lines the new one up with the first; saved.
key Escape; click 800 650 1.2; click 505 655 0.8; xdotool mousemove 811 661 keydown shift click 1 keyup shift; sleep 1; shot 53b-two-fields
TWO=$(ocr 640x460+400+330); LF=$(at Left 640x460+400+330); [ -n "$LF" ] && click $LF 1.2; key ctrl+s; sleep 1.5
BX=$(qpdf --qdf --object-streams=disable "$OUT/prep.pdf" - 2>/dev/null | python3 -c '
import re, sys
d = sys.stdin.buffer.read().decode("latin-1")
boxes = [o for o in re.split(r"\nendobj", d) if "/FT /Btn" in o]
rects = [[float(n) for n in m.group(1).split()] for o in boxes for m in [re.search(r"/Rect \[\s*([-0-9.\s]+)\]", o)] if m]
print(" ".join(map(str, sorted({round(r[0], 1) for r in rects}))), len(boxes), "w", " ".join(sorted({str(round(r[2] - r[0])) for r in rects})))')
check "Fields ⇧-clicked together are aligned by the card's Left, a check box placed by a click is 14 pt ($TWO; x of the check boxes, count, widths: $BX)" "echo \"$TWO\" | grep -q '2 fields selected' && echo \"$BX\" | grep -Eq '^[0-9.]+ 2 w 14$'"
# Grid (Acrobat's Snap to Grid; options bar): a text field dragged from 503,463 to 663,489 lands on the 9 pt grid.
key Escape; GR=$(at Grid 900x60+300+90); [ -n "$GR" ] && click $GR 0.6; TK=$(at Text 900x60+250+90); [ -n "$TK" ] && click $TK 0.4; shot 53c-grid
xdotool mousemove 503 463 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( 503 + i * 32 )) $(( 463 + i * 5 )); sleep 0.05; done; xdotool mouseup 1; sleep 1.2; key ctrl+s; sleep 1.5
SN=$(qpdf --qdf --object-streams=disable "$OUT/prep.pdf" - 2>/dev/null | python3 -c '
import re, sys
d = sys.stdin.buffer.read().decode("latin-1")
rects = [[float(n) for n in m.group(1).split()] for o in re.split(r"\nendobj", d) if "/FT /Tx" in o for m in [re.search(r"/Rect \[\s*([-0-9.\s]+)\]", o)] if m]
print(len(rects), sum(all(abs(v / 9 - round(v / 9)) < 0.01 for v in r) for r in rects))')
check "Grid on, a field drawn lands on its 9 pt lines (${GR:-no button}; text fields, on the grid: $SN)" "[ \"$SN\" = '2 1' ]"
quit
# Detect fields (982,109) on a flat form: its blanks after "Name:" and "City:" become two text fields.
python3 - "$OUT/flat.pdf" <<'PY'
import sys
c = b"BT /F1 12 Tf 72 700 Td (Name: ______________________) Tj 0 -40 Td (City: ______________________) Tj ET"
objs = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        b"<< /Length %d >>\nstream\n" % len(c) + c + b"\nendstream", b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"]
out, offs = b"%PDF-1.7\n", []
for i, o in enumerate(objs, 1):
    offs.append(len(out)); out += b"%d 0 obj\n" % i + o + b"\nendobj\n"
x = len(out)
out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1) + b"".join(b"%010d 00000 n \n" % o for o in offs)
out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, x)
open(sys.argv[1], "wb").write(out)
PY
cp "$OUT/flat.pdf" "$OUT/flat0.pdf"; launch "$OUT/flat.pdf"; key ctrl+9; sleep 1; key f; sleep 0.6; DF=$(at Detect 900x60+600+90); [ -n "$DF" ] && click $DF 2; key ctrl+s; sleep 1.5
FT=$(qpdf --qdf --object-streams=disable "$OUT/flat.pdf" - 2>/dev/null | grep -ac '/FT /Tx')
check "Detect fields turns a flat form's blanks into text fields (${DF:-no button}; $FT fields)" "[ \"$FT\" = 2 ]"
quit
# The same on a scanned form (a picture of two lines and a box, 144 dpi): its lines and box are found in the pixels.
convert -size 1224x1584 xc:white -fill none -stroke black -strokewidth 2 -draw "line 300,400 900,400" -draw "line 300,500 900,500" -draw "rectangle 300,600 330,630" -units PixelsPerInch -density 144 "$OUT/scan-form.pdf"
cp "$OUT/scan-form.pdf" "$OUT/scan0.pdf"; launch "$OUT/scan-form.pdf"; key ctrl+9; sleep 1; key f; sleep 0.6; DS=$(at Detect 900x60+600+90); [ -n "$DS" ] && click $DS 8; key ctrl+s; sleep 1.5
SQ=$(qpdf --qdf --object-streams=disable "$OUT/scan-form.pdf" - 2>/dev/null)
check "Detect fields finds a scanned form's lines and box ($(echo "$SQ" | grep -ac '/FT /Tx') text, $(echo "$SQ" | grep -ac '/FT /Btn') check box)" "[ \"\$(echo \"\$SQ\" | grep -ac '/FT /Tx')\" = 2 ] && [ \"\$(echo \"\$SQ\" | grep -ac '/FT /Btn')\" = 1 ]"
# Detect fields again (Sol, 26/10: a second run stacked new fields on the first): nothing is added.
DS=$(at Detect 900x60+600+90); [ -n "$DS" ] && click $DS 8; key ctrl+s; sleep 1.5
SQ2=$(qpdf --qdf --object-streams=disable "$OUT/scan-form.pdf" - 2>/dev/null)
check "Detect fields run again adds no field over those found ($(echo "$SQ2" | grep -ac '/FT /Tx') text, $(echo "$SQ2" | grep -ac '/FT /Btn') check box)" "[ \"\$(echo \"\$SQ2\" | grep -ac '/FT /Tx')\" = 2 ] && [ \"\$(echo \"\$SQ2\" | grep -ac '/FT /Btn')\" = 1 ]"
quit
# A document mixing the two (Sol, 26/10: the scan was left out once the drawn page gave fields): all are found.
qpdf --empty --pages "$OUT/flat0.pdf" "$OUT/scan0.pdf" -- "$OUT/mixed-form.pdf"
launch "$OUT/mixed-form.pdf"; key ctrl+9; sleep 1; key f; sleep 0.6; DM=$(at Detect 900x60+600+90); [ -n "$DM" ] && click $DM 8; key ctrl+s; sleep 1.5
MQ=$(qpdf --qdf --object-streams=disable "$OUT/mixed-form.pdf" - 2>/dev/null)
check "Detect fields on a drawn page and a scanned one finds both's ($(echo "$MQ" | grep -ac '/FT /Tx') text, $(echo "$MQ" | grep -ac '/FT /Btn') check box)" "[ \"\$(echo \"\$MQ\" | grep -ac '/FT /Tx')\" = 4 ] && [ \"\$(echo \"\$MQ\" | grep -ac '/FT /Btn')\" = 1 ]"
quit

# 54. Closing what would be lost asks first: a paragraph retyped in Edit text and not applied ("Save changes to
# …?"), and Redact's marks not applied ("Close … without redacting?", its own tab's × at 466,21); Cancel keeps
# the tab.
cp "$OUT/text.pdf" "$OUT/retype.pdf"; launch "$OUT/retype.pdf"; key ctrl+9; sleep 1
key e; sleep 1; QK=$(at quick 1020x700+255+90); [ -n "$QK" ] && click $QK 1.2
xdotool windowfocus --sync "$W" 2>/dev/null; key ctrl+End; xdotool type --delay 10 " zebra"; sleep 0.8; click 296 21 1
RT=$(ocr 700x200+290+250); CN=$(at Cancel 700x300+290+250); [ -n "$CN" ] && click $CN 0.8
check "closing with a paragraph retyped asks first, Cancel keeps it (${RT:0:60}; $(ntabs) document tab)" "echo \"$RT\" | grep -q 'Save changes' && [ \"\$(ntabs)\" = 1 ]"
key Escape; quit
# The same with only its format changed (Bold, 470,110, in the format row): asked about too.
cp "$OUT/text.pdf" "$OUT/reformat.pdf"; launch "$OUT/reformat.pdf"; key ctrl+9; sleep 1
key e; sleep 1; QK=$(at quick 1020x700+255+90); [ -n "$QK" ] && click $QK 1.2
click 470 110 0.8; click 296 21 1
RF=$(ocr 700x200+290+250); CN=$(at Cancel 700x300+290+250); [ -n "$CN" ] && click $CN 0.8
check "closing with a paragraph only made bold asks first (${RF:0:60})" "echo \"$RF\" | grep -q 'Save changes'"
key Escape; quit
cp "$OUT/text.pdf" "$OUT/redact2.pdf"; launch "$OUT/redact2.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "redact"; key Return; sleep 3
P=$(at Page 820x700+140+60); [ -n "$P" ] && { set -- $P; xdotool mousemove $1 $2 click --repeat 2 --delay 120 1; sleep 1; }
click 466 21 1; RC=$(ocr 700x200+290+250); CN=$(at Cancel 700x300+290+250); [ -n "$CN" ] && click $CN 0.8
check "closing Redact with marks not applied asks first (${RC:0:60})" "echo \"$RC\" | grep -q 'without redacting'"
quit

# 60. Text box ▸ Callout (Acrobat's Callout tool): a click at 700,420 (on the page) then "Look here" typed: saved as a callout,
# a FreeText whose line (/CL) ends at the click, the box up and right of it.
cp "$OUT/text.pdf" "$OUT/callout.pdf"; launch "$OUT/callout.pdf"; sleep 1
# Callout ends the format row, 115 px right of More (More fonts…; OCR misses Callout on its grey).
key t; sleep 0.6; MO=$(at More 1280x40+0+90); CO=${MO:+$(set -- $MO; echo $(( $1 + 115 )) $2)}; [ -n "$CO" ] && click $CO 0.5
click 700 420 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; xdotool type --delay 10 "Look here"; key ctrl+Return; sleep 1.2
key v; key ctrl+s; sleep 1.5; shot 60-callout
CQ=$(qpdf --qdf --object-streams=disable "$OUT/callout.pdf" - 2>/dev/null)
check "the Callout option makes a text box with a line to the click (${CO:-no button})" "echo \"\$CQ\" | grep -aq '/IT /FreeTextCallout' && echo \"\$CQ\" | grep -aq '/CL \[' && echo \"\$CQ\" | grep -aq 'Look here'"
quit

# 61. Zoom to area (Z, Acrobat's Marquee Zoom): a box dragged around "quick" fills the view: the word comes back
# at least twice as wide (the zoom stops at 400 % of actual size).
cp "$OUT/text.pdf" "$OUT/zoomarea.pdf"; launch "$OUT/zoomarea.pdf"; sleep 1
Q0=$(span quick 1020x700+255+90)
if [ -n "$Q0" ]; then
    set -- $Q0; W0=$(( $2 - $1 )); key z; sleep 0.4
    xdotool mousemove $(( $1 - 20 )) $(( $3 - 25 )) mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( $1 - 20 + (W0 + 40) * i / 5 )) $(( $3 - 25 + 50 * i / 5 )); sleep 0.05; done; xdotool mouseup 1; sleep 3.5
fi
shot 61-zoom-area; Q1=$(span quick 1280x720+0+80); W1=$( [ -n "$Q1" ] && set -- $Q1 && echo $(( $2 - $1 )) )
check "Zoom to area makes the box dragged fill the view (quick ${W0:-?} → ${W1:-?} px wide)" "[ -n \"$W1\" ] && [ \"$W1\" -ge \$(( ${W0:-1000} * 2 )) ]"
quit

# 62. Document Properties (⌘D) ▸ When opened (Acrobat's Initial View): page 3, Two pages; saved: /OpenAction to page 3
# and /PageLayout /TwoColumnLeft. A copy never opened here opens as it asks: page 3 in view, not page 1.
cp "$OUT/text.pdf" "$OUT/initial.pdf"; launch "$OUT/initial.pdf"; sleep 1
# The page field sits under the "When opened" label, right of its "Page" (the page's own "Page 1" shows behind the sheet).
key ctrl+d; sleep 1; read OX OY <<< "$(at opened 900x700+190+60)"; PG=${OY:+$(( OX + 20 )) $(( OY + 34 ))}; [ -n "$PG" ] && { click $PG 0.4; xdotool type --delay 20 "3"; }
TW=$(at Two 900x700+190+60); [ -n "$TW" ] && click $TW 0.4
# Window ▸ Full screen (Acrobat's Open in Full Screen mode).
FL=$(at Full 900x700+190+60); [ -n "$FL" ] && click $FL 0.4
# Save (white on red) is 71 px right of Cancel, on the sheet's bottom row (read on a small strip: the dimmed
# page around confuses tesseract).
CN=$(at Cancel 460x50+410+720); SV=${CN:+$(set -- $CN; echo $(( $1 + 71 )) $2)}; [ -n "$SV" ] && click $SV 1; key ctrl+s; sleep 1.5; shot 62-initial-view
IV=$(qpdf --qdf --object-streams=disable "$OUT/initial.pdf" - 2>/dev/null)
quit
cp "$OUT/initial.pdf" "$OUT/initial-copy.pdf"; launch "$OUT/initial-copy.pdf"; sleep 2; IO=$(ocr 1020x700+255+90)
check "When opened sets the Initial View and a new copy opens as it asks (${PG:-no page field} ${TW:-no Two}; $(echo "$IO" | cut -c1-60))" "echo \"\$IV\" | grep -aq '/OpenAction' && echo \"\$IV\" | grep -aq '/PageLayout /TwoColumnLeft' && echo \"\$IV\" | grep -aq '/PageMode /FullScreen' && echo \"$IO\" | grep -q 'Page 3' && ! echo \"$IO\" | grep -q 'Page 1'"
quit

# 63. More (1127,61) ▸ Copy an area as a picture (16th item, 1008,519; Acrobat's Snapshot): a box dragged over the
# page puts a PNG on the clipboard.
cp "$OUT/text.pdf" "$OUT/snap.pdf"; launch "$OUT/snap.pdf"; sleep 1
click 1108 61 0.8; click 1008 519 0.8
xdotool mousemove 500 300 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( 500 + i * 30 )) $(( 300 + i * 20 )); sleep 0.05; done; xdotool mouseup 1; sleep 1.5
CT=$(xclip -selection clipboard -t TARGETS -o 2>/dev/null | tr '\n' ' ')
check "Copy an area puts a picture on the clipboard ($CT)" "echo \"$CT\" | grep -q 'image/png'"
quit

# 64. More (1127,61) ▸ Scroll automatically (17th item, 1008,547; Acrobat's): "quick" climbs by itself (60 px a
# second: read after 1.5 s, before it leaves the view); Escape stops it.
cp "$OUT/text.pdf" "$OUT/autoscroll.pdf"; launch "$OUT/autoscroll.pdf"; sleep 1
read _ AY0 <<< "$(at quick 1020x700+255+90)"; click 1108 61 0.8; click 1008 547 1.2
# Read above where it was (the next page's "quick" may have come into view below).
read _ AY1 <<< "$(at quick 1020x$(( ${AY0:-700} - 92 ))+255+90)"; key Escape; sleep 0.5; read _ AY2 <<< "$(at quick 1020x700+255+90)"; sleep 1; read _ AY3 <<< "$(at quick 1020x700+255+90)"
check "Scroll automatically moves the pages up until Escape (quick at ${AY0:-?}, ${AY1:-?}, then ${AY2:-?} = ${AY3:-?})" "[ -n \"$AY1\" ] && [ \"$AY1\" -lt \$(( ${AY0:-0} - 40 )) ] && [ \"$AY2\" = \"$AY3\" ]"
quit

# 65. Signature (S) ▸ Initials (Acrobat's Add Initials): a click opens the pad for initials ("Your initials"),
# "AL" typed and used: kept apart from the signatures (secure store "initials"), placed 60 pt wide.
cp "$OUT/text.pdf" "$OUT/initials.pdf"; launch "$OUT/initials.pdf"; sleep 1; key s; sleep 0.5
IN=$(at Initials 1280x40+0+90); [ -n "$IN" ] && click $IN 0.5; click 640 500 1.2; shot 65-initials-pad
# The field's placeholder is too faint to read: it sits 37 px under "or type them".
TT=$(at type 900x700+190+60); TF=${TT:+$(set -- $TT; echo $1 $(( $2 + 37 )))}; [ -n "$TF" ] && { click $TF 0.4; xdotool type --delay 30 "AL"; sleep 0.6; }
# Use (white on red, unread by OCR) is 66 px right of Cancel, on the pad's last row (the styles typed push the
# pad up and its row down), read on the pad's lower part (x 430–890, y 400–700).
shot 65b-initials-typed
CN=$(at Cancel 460x300+430+400); US=${CN:+$(set -- $CN; echo $(( $1 + 66 )) $2)}; [ -n "$US" ] && click $US 1.2; key v; key ctrl+s; sleep 1.5
IP=$(secure_marks | python3 -c 'import json, sys; i = json.load(sys.stdin).get("initials"); print("kept" if i and i[1] == "AL" else "none")' 2>/dev/null)
IW=$(qpdf --qdf --object-streams=disable "$OUT/initials.pdf" - 2>/dev/null | python3 -c '
import re, sys
d = sys.stdin.buffer.read().decode("latin-1")
r = [[float(n) for n in m.group(1).split()] for o in re.split(r"\nendobj", d) if "PCFillSign" in o or "/Subtype /Stamp" in o for m in [re.search(r"/Rect \[\s*([-0-9.\s]+)\]", o)] if m]
print(" ".join(str(round(x[2] - x[0])) for x in r))')
check "Initials are made on their own pad, kept apart and placed 60 pt wide (${IN:-no Initials}; field ${TF:-?}, Use ${US:-?}; $IP; widths $IW)" "[ \"$IP\" = kept ] && echo \" $IW \" | grep -q ' 60 '"
# Every page (the Signature tool again): one click puts the initials on all four pages.
key s; sleep 0.5; EP=$(at Every 1280x40+0+90); [ -n "$EP" ] && click $EP 0.5; click 640 600 1.5; key v; key ctrl+s; sleep 1.5
EV=$(qpdf --qdf --object-streams=disable "$OUT/initials.pdf" - 2>/dev/null | grep -ac 'PCFillSign')
check "Every page places the initials on each page in one click (${EP:-no Every page}; $EV marks)" "[ \"\${EV:-0}\" -ge 5 ]"
# Its pages field (right of Every page; Acrobat's Place on multiple pages): "even", then a click: on pages 2 and 4 only.
key s; sleep 0.5; [ -n "$EP" ] && { set -- $EP; click $(( $1 + 100 )) $2 0.4; xdotool type --delay 20 "even"; }; click 640 500 1.5; key v; key ctrl+s; sleep 1.5
EV2=$(qpdf --qdf --object-streams=disable "$OUT/initials.pdf" - 2>/dev/null | grep -ac 'PCFillSign')
check "the pages field puts them on the even pages only ($EV → $EV2 marks)" "[ \"\${EV2:-0}\" = \"\$(( \${EV:-0} + 2 ))\" ]"
quit

# 66. Compress ▸ To a size (iLovePDF's target size): 0.001 MB typed, out of reach for text.pdf: the smallest file is
# written and the result says it is as small as it gets.
cp "$OUT/text.pdf" "$OUT/target.pdf"; launch "$OUT/target.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "compress"; key Return; sleep 2
# The "Largest file (MB)" field sits 80 px under the card.
TS=$(at size 280x600+1000+40); [ -n "$TS" ] && { set -- $TS; click $1 $2 0.4; click $1 $(( $2 + 80 )) 0.4; xdotool type --delay 20 "0.001"; }
click 1140 755 4; shot 66-target; TR=$(import -window root png:- | convert - -resize 200% png:- | tesseract stdin stdout 2>/dev/null | tr '\n' ' ')
check "Compress to a size out of reach keeps the smallest and says so (${TS:-no option}; $(ls $OUT/target-compressed*.pdf 2>/dev/null | wc -l) file)" "ls $OUT/target-compressed*.pdf >/dev/null 2>&1 && echo \"$TR\" | grep -q 'as small as it gets'"
quit

# 50. Any colour (Acrobat's Other color): the row's sixth dot (620,110 with the Pen, after the four widths and
# "- -") opens a palette; its orange (the second swatch, 580,145) draws the stroke, saved orange (F28C28).
cp "$OUT/text.pdf" "$OUT/colour.pdf"; launch "$OUT/colour.pdf"
key p; click 620 110 0.8; click 580 145 0.6
xdotool mousemove 500 400 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( 500 + i * 30 )) 410; sleep 0.05; done; xdotool mouseup 1; sleep 1; key ctrl+s; sleep 1.5
OC=$(qpdf --json=2 "$OUT/colour.pdf" | python3 -c 'import json, sys
for v in json.load(sys.stdin)["qpdf"][1].values():
    d = v.get("value", {}) if isinstance(v, dict) else {}
    if d.get("/Subtype") == "/Ink": print(d.get("/C"))')
check "a colour from the palette draws the stroke ($OC)" "echo \"$OC\" | grep -qE '0\.94[0-9]*, 0\.54[0-9]*, 0\.15'"
quit

# 51. Bookmarks (Outline 95,106): + Bookmark on page 1 then on page 3; the second moved up (hover, ⋯ at the
# row's right 199,218 ▸ Move up) comes first in the file's outline.
cp "$OUT/text.pdf" "$OUT/marks.pdf"; launch "$OUT/marks.pdf"; key ctrl+9; sleep 1
click 95 106 0.8; click 55 149 1; key End; sleep 1; key Up; key Up; sleep 0.8; click 55 149 1
xdotool mousemove 150 218; sleep 0.5; click 199 218 0.8; MU=$(at Move 260x200+0+220); [ -n "$MU" ] && click $MU 1; key ctrl+s; sleep 1.5; shot 51-bookmarks
OT=$(qpdf --json=2 "$OUT/marks.pdf" | python3 -c 'import json, sys
o = json.load(sys.stdin)["qpdf"][1]
get = lambda r: o.get("obj:" + r, {}).get("value", {})
root = next(v["value"] for v in o.values() if isinstance(v, dict) and v.get("value", {}).get("/Type") == "/Catalog")
ref = lambda r: r.replace(" 0 R", " 0").replace(" ", " ") if r else r
get = lambda r: o.get("obj:" + r, {}).get("value", {}) if r else {}
out = get(root["/Outlines"]); cur = out.get("/First"); titles = []
while cur:
    d = get(cur); titles.append(d.get("/Title", "")); cur = d.get("/Next")
print(" ".join(t.replace("u:", "") for t in titles))')
check "a bookmark moved up comes first in the outline ($OT)" "echo \"$OT\" | grep -qE '^Page [34] Page 1'"
# Dragged (Acrobat's): the second row (y 218) pressed and drawn up to the first row's top edge, dropped: back first.
xdotool mousemove 100 218 mousedown 1; for y in 212 204 196 188 180 172 169; do xdotool mousemove 100 $y; sleep 0.06; done; xdotool mouseup 1; sleep 1; key ctrl+s; sleep 1.5
OT2=$(qpdf --json=2 "$OUT/marks.pdf" | python3 -c 'import json, sys
o = json.load(sys.stdin)["qpdf"][1]
get = lambda r: o.get("obj:" + r, {}).get("value", {}) if r else {}
root = next(v["value"] for v in o.values() if isinstance(v, dict) and v.get("value", {}).get("/Type") == "/Catalog")
out = get(root["/Outlines"]); cur = out.get("/First"); titles = []
while cur:
    d = get(cur); titles.append(d.get("/Title", "")); cur = d.get("/Next")
print(" ".join(t.replace("u:", "") for t in titles))')
check "a bookmark dragged above another goes before it ($OT → $OT2)" "echo \"$OT2\" | grep -qE '^Page 1 Page [34]'"
quit

# 52. Files panel (211,106): on hover, the trash deletes an attachment and the pencil renames one (Acrobat's
# Attachments pane). Rows from y 185, 34 apart, in name order: annex, budget, contract, notes; trash at 223,
# pencil at 193. notes.txt deleted, budget.txt renamed money.txt; Attach… adds one to the document.
cp "$OUT/withatt.pdf" "$OUT/att2.pdf"; launch "$OUT/att2.pdf"; click 211 106 1
xdotool mousemove 120 287; sleep 0.4; click 223 287 1
xdotool mousemove 120 219; sleep 0.4; click 193 219 0.6; key ctrl+a; xdotool type --delay 20 "money.txt"; key Return; sleep 1
key ctrl+s; sleep 1.5; shot 52-attachments
AL=$(qpdf --list-attachments "$OUT/att2.pdf" 2>/dev/null | tr '\n' ' ')
check "an attachment is deleted and another renamed ($AL)" "! echo \"$AL\" | grep -q notes.txt && echo \"$AL\" | grep -q money.txt && ! echo \"$AL\" | grep -q budget.txt && echo \"$AL\" | grep -q contract.txt"
quit

# 67. More (1127,61) ▸ One page at a time (18th item, 1008,575; Acrobat's Single Page): the page fits the view; one
# wheel notch down shows page 2 alone (its "Page 2" in view, no "Page 1").
cp "$OUT/text.pdf" "$OUT/single-view.pdf"; launch "$OUT/single-view.pdf"; sleep 1
click 1108 61 0.8; click 1008 575 1.5; xdotool mousemove 640 450; xdotool click 5; sleep 1.2; shot 67-one-page
SP=$(ocr 1020x720+255+80)
check "One page at a time turns to page 2 alone with one wheel notch ($(echo "$SP" | cut -c1-60))" "echo \"$SP\" | grep -q 'Page 2' && ! echo \"$SP\" | grep -q 'Page 1' && ! echo \"$SP\" | grep -q 'Page 3'"
# Zoomed in (⌘9, the page wider than the view and taller): a notch scrolls through the page first, page 2 stays
# (Acrobat's; Sol, 26/10: it turned at once).
key ctrl+9; sleep 1; xdotool mousemove 640 450; xdotool click 5; sleep 1.2; SZ=$(ocr 1020x720+255+80)
check "One page at a time zoomed in scrolls within the page before turning it ($(echo "$SZ" | cut -c1-40))" "! echo \"$SZ\" | grep -q 'Page 3'"
quit

# 68. Find and replace (Acrobat's Edit PDF ▸ Find ▸ Replace All): "quick" searched (⌘F, ↩), "slow" typed in the
# panel's Replace with, Replace all: every page's text now says "slow brown fox", none "quick".
cp "$OUT/text.pdf" "$OUT/replace-all.pdf"; launch "$OUT/replace-all.pdf"; sleep 1
key ctrl+f; xdotool type --delay 10 "quick"; key Return; sleep 2
# The field's placeholder is too faint to read: it is the row above the "Replace all" button, across the panel.
RA=$(at all 300x300+0+60); RW=${RA:+$(set -- $RA; echo 100 $(( $2 - 34 )))}
[ -n "$RW" ] && { click $RW 0.4; xdotool type --delay 20 "slow"; click $RA 3; }; key ctrl+s; sleep 2; shot 68-replace-all
RT=$(pdftotext "$OUT/replace-all.pdf" - 2>/dev/null | tr '\n' ' ')
check "Replace all rewrites the words on every page (${RW:-no field} ${RA:-no button}; $(echo "$RT" | cut -c1-60))" "[ \"\$(echo \"$RT\" | grep -o 'slow brown' | wc -l)\" = 4 ] && ! echo \"$RT\" | grep -q quick"
quit

# 69. More (1127,61) ▸ Email… (19th item, 1008,603; Acrobat's Share ▸ Email): a note added and not saved, then
# Email: a copy holding the note goes to the mail app (the test seam names the file attached).
cp "$OUT/text.pdf" "$OUT/mail.pdf"; launch "$OUT/mail.pdf"; sleep 1; rm -f "$OUT/mail-sent"
key n; click 640 400 0.8; xdotool type --delay 10 "mailed note"; key ctrl+Return; sleep 0.8
click 1108 61 0.8; click 1008 603 1.2; MF=$(cat "$OUT/mail-sent" 2>/dev/null)
check "Email attaches the document as it is now (${MF:-nothing attached})" "[ -f \"$MF\" ] && qpdf --qdf --object-streams=disable \"$MF\" - 2>/dev/null | grep -aq 'mailed note'"
quit

# 70. Crop ▸ Change page size (Acrobat's Set Page Boxes ▸ Change Page Size), A5: every page of the output is
# 420 × 595 pt, its text still there.
cp "$OUT/text.pdf" "$OUT/resize.pdf"; launch "$OUT/resize.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "crop"; key Return; sleep 2.5
CP=$(at Change 280x700+1000+40); [ -n "$CP" ] && click $CP 0.6; # OCR misses the papers' names: A5, the fourth, sits 112 px right of and 55 px under "Change".
A5=${CP:+$(set -- $CP; echo $(( $1 + 112 )) $(( $2 + 55 )))}; [ -n "$A5" ] && click $A5 0.4
click 1140 755 4; RZ=$(pdfinfo -f 1 -l 4 "$OUT/resize-cropped.pdf" 2>/dev/null | grep -c 'size: *420 x 595')
check "Change page size makes every page A5 (${CP:-no option} ${A5:-no A5}; $RZ of 4 pages)" "[ \"$RZ\" = 4 ] && pdftotext $OUT/resize-cropped.pdf - 2>/dev/null | grep -q 'Page 4'"
quit

# 71. Rotate ▸ Flip ↔ (a mirror image): the output's "Page 1" ends 72 pt from the right edge instead of starting
# 72 pt from the left.
cp "$OUT/text.pdf" "$OUT/flip.pdf"; launch "$OUT/flip.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "rotate"; key Return; sleep 2.5
FL=$(at Flip 280x700+1000+40); [ -n "$FL" ] && click $FL 0.5; click 1140 755 4
FX=$(pdftotext -bbox -f 1 -l 1 "$OUT/flip-rotated.pdf" - 2>/dev/null | grep -o 'xMax="[0-9.]*"' | tr -dc '0-9.\n' | sort -n | tail -1)
check "Flip ↔ mirrors the pages (${FL:-no Flip}; the text now reaches x ${FX:-?}, 410 before)" "python3 -c 'import sys; sys.exit(0 if float(\"${FX:-0}\") > 500 else 1)'"
quit

# 72. Compare ▸ Side by side (Acrobat's side-by-side view): one.pdf against two.pdf, the result one sheet as wide as
# both pages, the original's colour on the left half and the changed one's on the right.
launch "$OUT/reo/one.pdf" "$OUT/reo/two.pdf"
click 50 21 1; click 67 113 1; click 993 76 0.5; xdotool type --delay 20 "compare"; key Return; sleep 2
click 388 411 1; click 330 481 2; click 600 411 1; click 560 481 2
SB=$(at Side 280x700+1000+90); [ -n "$SB" ] && click $SB 1; answer "$PWD/$OUT/reo/side.pdf"
for try in 1 2; do
    click 1134 756 1; for _ in $(seq 1 20); do ocr 700x60+100+712 | grep -q successfully && break 2; sleep 0.5; done
done
# Written next to the original (Preferences ▸ next to the original, set earlier): the newest comparison.
answer ""; SIDE=$(ls -t "$OUT"/reo/one-comparison*.pdf 2>/dev/null | head -1); cp "$SIDE" "$OUT/reo/side.pdf" 2>/dev/null
SL=$(pdftoppm -r 10 -f 1 -l 1 -png "$OUT/reo/side.pdf" 2>/dev/null | convert - -format "%[pixel:p{12,25}] %[pixel:p{%[fx:w-12],25}]" info: 2>/dev/null)
check "Side by side puts the original left of the changed page on one sheet (${SB:-no option}; $SL)" "pdfinfo $OUT/reo/side.pdf 2>/dev/null | grep -q 'Pages: *1' && [ \"\$(echo '$SL' | cut -d' ' -f1)\" != \"\$(echo '$SL' | cut -d' ' -f2)\" ]"
quit

# 73. Organize ▸ Pages per sheet ▸ Booklet (Acrobat's Booklet): text.pdf's four pages on two sides, the first holding
# pages 4 and 1 (folded, they read in order).
cp "$OUT/text.pdf" "$OUT/booklet.pdf"; launch "$OUT/booklet.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "organize"; key Return; sleep 3
BK=$(at Booklet 280x700+1000+40); [ -n "$BK" ] && click $BK 0.5; click 1140 755 4
BT=$(pdftotext -f 1 -l 1 "$OUT/booklet-organized.pdf" - 2>/dev/null | tr '\n' ' ')
check "Booklet puts two pages to a side in folding order (${BK:-no option}; $(pdfinfo $OUT/booklet-organized.pdf 2>/dev/null | grep Pages); $(echo $BT | cut -c1-60))" "pdfinfo $OUT/booklet-organized.pdf 2>/dev/null | grep -q 'Pages: *2' && echo \"$BT\" | grep -q 'Page 4' && echo \"$BT\" | grep -q 'Page 1'"
quit
# 73b. Organize ▸ One long page (Stirling's Single large page): text.pdf's four pages on a single page.
cp "$OUT/text.pdf" "$OUT/long.pdf"; launch "$OUT/long.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "organize"; key Return; sleep 3
LG=$(at long 280x700+1000+40); [ -n "$LG" ] && click $LG 0.5; click 1140 755 4
check "One long page puts every page on one (${LG:-no option}; $(pdfinfo $OUT/long-organized.pdf 2>/dev/null | grep Pages))" "pdfinfo $OUT/long-organized.pdf 2>/dev/null | grep -q 'Pages: *1' && pdftotext $OUT/long-organized.pdf - | grep -q 'Page 4'"
quit

# 74. Split ▸ Cut each page into parts (a scanned spread made two pages again), Left | right: text.pdf's four pages become
# eight, page 1's first words on the first.
cp "$OUT/text.pdf" "$OUT/spread.pdf"; launch "$OUT/spread.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "split"; key Return; sleep 3
CT=$(at Cut 280x700+1000+40); [ -n "$CT" ] && click $CT 0.6; click 1140 755 4
HV=$(ls -t $OUT/spread-cut*.pdf 2>/dev/null | head -1)
check "Cut each page in two makes eight half pages (${CT:-no option}; ${HV:-no file})" "[ -n \"$HV\" ] && pdfinfo \"$HV\" | grep -q 'Pages: *8' && pdftotext -f 1 -l 1 \"$HV\" - | grep -q 'Page 1'"
quit
# The same, Grid (2 × 2 to start, Stirling's sections): sixteen quarter pages.
cp "$OUT/text.pdf" "$OUT/poster.pdf"; launch "$OUT/poster.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "split"; key Return; sleep 3
CT=$(at Cut 280x700+1000+40); [ -n "$CT" ] && click $CT 0.6; GR=$(at Grid 280x700+1000+40)
# Grid's box hugs its word, which OCR may read with the border ("‘Grid"): then 37 px right of "bottom" (Top / bottom).
[ -z "$GR" ] && read _ BR BY <<< "$(span bottom 280x700+1000+40)" && [ -n "$BY" ] && GR="$(( BR + 37 )) $BY"
[ -n "$GR" ] && click $GR 0.6; click 1140 755 4
GV=$(ls -t $OUT/poster-cut*.pdf 2>/dev/null | head -1)
check "Cut each page into a grid makes sixteen quarter pages (${GR:-no Grid}; ${GV:-no file})" "[ -n \"$GV\" ] && pdfinfo \"$GV\" | grep -q 'Pages: *16'"
quit

# 75. Home ▸ From screenshot (Acrobat's Create PDF ▸ Screen capture): the area taken (the test seam gives stamp.png,
# 80 × 40 px, green) becomes a one-page PDF, named in the Save dialog and opened.
launch; answer "$PWD/$OUT/screenshot.pdf"; FS=$(at screenshot 900x120+300+80); [ -n "$FS" ] && click $FS 1
for _ in $(seq 1 12); do [ "$(ntabs)" = 1 ] && break; sleep 0.5; done; answer ""
SC=$(pdftoppm -r 20 -f 1 -l 1 -png "$OUT/screenshot.pdf" 2>/dev/null | convert - -format "%[pixel:p{5,5}]" info: 2>/dev/null)
check "From screenshot makes a PDF of the area taken and opens it (${FS:-no button}; $SC; $(ntabs) tabs)" "[ \"\$(npages $OUT/screenshot.pdf 2>/dev/null)\" = 1 ] && echo \"$SC\" | grep -q 'srgb(42,15[0-9],7[0-9])' && [ \"\$(ntabs)\" = 1 ]"
quit

# 76. Tabs (browsers', PDF Expert's): a note added and not saved turns the active tab's × (its last 40 px) into a
# dot; ⌃Tab goes to the other tab; ⌘O on Home opens the file dialog (answered: text.pdf opens, a third tab).
cp "$OUT/text.pdf" "$OUT/tab1.pdf"; cp "$OUT/text.pdf" "$OUT/tab2.pdf"; launch "$OUT/tab1.pdf" "$OUT/tab2.pdf"; sleep 1.5
corner() { import -window root -crop 40x24+440+9 png:- 2>/dev/null | md5sum | cut -c1-8; }
xdotool mousemove 640 500; sleep 0.4; C0=$(corner)
key n; click 640 400 0.8; xdotool type --delay 10 "dot"; key ctrl+Return; sleep 0.8; xdotool mousemove 640 500; sleep 0.4; C1=$(corner)
key ctrl+Tab; sleep 0.8; C2=$(corner)
click 40 21 1; answer "$PWD/$OUT/text.pdf"; key ctrl+o; sleep 2.5; answer ""; NT=$(ntabs)
check "an unsaved tab shows a dot, ⌃Tab steps, ⌘O opens (corner $C0 → $C1 → $C2; $NT tabs)" "[ \"$C0\" != \"$C1\" ] && [ \"$C1\" != \"$C2\" ] && [ \"$NT\" = 3 ]"
quit

# 77. Redact's list (Acrobat's Redaction panel): "quick" on page 1 marked by a double-click, the view sent to the last
# page (the wheel), then the mark's row clicked in the panel: page 1 is back in view.
cp "$OUT/text.pdf" "$OUT/redact3.pdf"; launch "$OUT/redact3.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "redact"; key Return; sleep 3
Q=$(at quick 820x700+140+60); [ -n "$Q" ] && { set -- $Q; xdotool mousemove $1 $2 click --repeat 2 --delay 120 1; sleep 1; }
# Scrolled away by the wheel (Redact's view takes no End key).
xdotool mousemove 500 400; for _ in $(seq 1 40); do xdotool click 5; sleep 0.02; done; sleep 1; V0=$(ocr 820x700+140+60)
# The row's "Page 1", under "Marked for redaction" (the intro above says "quickly").
ROW=$(at Page 280x300+1000+480); [ -n "$ROW" ] && click $ROW 1.2; V1=$(ocr 820x700+140+60)
check "a mark's row in Redact's list shows it on its page (${ROW:-no row}; $(echo $V0 | cut -c1-20) → $(echo $V1 | cut -c1-20))" "! echo \"$V0\" | grep -q 'Page 1' && echo \"$V1\" | grep -q 'Page 1'"
quit

# 78. Rotate view (Acrobat's View ▸ Rotate View): ⇧⌘+ turns the pages shown a quarter, "quick" no longer reads
# across; ⇧⌘− turns them back. The file is not changed (nothing to save).
cp "$OUT/text.pdf" "$OUT/turned.pdf"; launch "$OUT/turned.pdf"; sleep 1
# Tesseract reads the turned word too: it reads narrow then (its letters run down).
wide() { [ -n "$1" ] && set -- $1 && echo $(( $2 - $1 )); }
R0=$(span quick 1020x700+255+90); key ctrl+shift+plus; sleep 2; R1=$(span quick 1020x700+255+90); shot 78-turned
key ctrl+shift+minus; sleep 2; R2=$(span quick 1020x700+255+90); M0=$(md5sum < "$OUT/text.pdf"); M1=$(md5sum < "$OUT/turned.pdf")
W0=$(wide "$R0"); W1=$(wide "$R1"); W2=$(wide "$R2")
check "Rotate view turns the pages shown and back, the file untouched (quick ${W0:-?} → ${W1:-?} → ${W2:-?} px wide)" "[ \"\${W0:-0}\" -gt 20 ] && [ \"\${W1:-0}\" -lt \$(( \${W0:-0} / 2 )) ] && [ \"\${W2:-0}\" -gt 20 ] && [ \"$M0\" = \"$M1\" ]"
quit

