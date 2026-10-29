# Sourced by dev/scripts/gui-smoke.sh, in name order: shares its helpers, profile and variables.
# Steps 79 on: tab and document menus, properties, forms, preferences, the later reader features
# (alone in a shard of gui-parallel.sh, with the helpers every part defines).

# 79. A document tab's menu (PDF Expert's File menu): Rename… (the Save dialog answered with named.pdf in the same
# folder) renames the file under the open tab; Move to Trash then sends it to the trash and closes the tab.
cp "$OUT/text.pdf" "$OUT/toname.pdf"; launch "$OUT/toname.pdf"; sleep 1
answer "$PWD/$OUT/named.pdf"; read AX _ <<< "$(at toname 1280x40+0+0)"; xdotool mousemove "${AX:-0}" 21 click 3; sleep 0.8
RN=$(at Rename 1280x400+0+0); [ -n "$RN" ] && click $RN 1.5; answer ""
NM=$( [ -f "$OUT/named.pdf" ] && [ ! -e "$OUT/toname.pdf" ] && echo renamed )
read AX _ <<< "$(at named 1280x40+0+0)"; xdotool mousemove "${AX:-0}" 21 click 3; sleep 0.8
# Read on the menu alone (it opens at the pointer, 280 wide): beside it the page's large first line ran into
# "Move to Trash" and tesseract lost the item, then "Move to New Window" was the first Move.
MT=$(at Move "270x400+$(( ${AX:-10} - 10 ))+0"); [ -n "$MT" ] && click $MT 1.5
check "a tab's menu renames its file and moves it to the trash (${RN:-no Rename} ${NM:-not renamed}; ${MT:-no Move}; $(ntabs) tabs)" "[ \"$NM\" = renamed ] && [ ! -e \"$OUT/named.pdf\" ] && [ \"\$(ntabs)\" = 0 ]"
quit

# 80. Document properties (⌘D) ▸ Copy report (Acrobat's PDF info): the clipboard holds the description, the facts and
# the fonts, each said embedded or not (text.pdf: Helvetica, one of the standard fonts, not embedded).
cp "$OUT/text.pdf" "$OUT/report.pdf"; launch "$OUT/report.pdf"; sleep 1; key ctrl+d; sleep 2
# Copy report starts the sheet's bottom row (read on that strip: the dimmed page around confuses tesseract).
CR=$(at Copy 300x44+400+722); [ -n "$CR" ] && click $CR 1
RP=$(xclip -selection clipboard -o 2>/dev/null | tr '\n' '|')
check "Copy report puts the properties and the fonts on the clipboard (${CR:-no button}; $(echo "$RP" | cut -c1-60))" "echo \"\$RP\" | grep -q '^report.pdf|' && echo \"\$RP\" | grep -q 'Pages: 4' && echo \"\$RP\" | grep -q 'Helvetica (not embedded)'"
quit

# 81. Home ▸ From clipboard with PDF files copied (a file manager's Copy: text/uri-list): they make the new PDF, merged.
qpdf "$OUT/text.pdf" --pages . 1 -- "$OUT/single.pdf"   # part 3's one-page fixture, made here for a shard of its own
printf 'file://%s\r\nfile://%s\r\n' "$PWD/$OUT/text.pdf" "$PWD/$OUT/single.pdf" | xclip -selection clipboard -t text/uri-list; sleep 0.3
launch; answer "$PWD/$OUT/clipfiles.pdf"; FC=$(at From 900x120+300+80); [ -n "$FC" ] && click $FC 1; for _ in $(seq 1 12); do [ -f "$OUT/clipfiles.pdf" ] && break; sleep 0.5; done; answer ""
check "From clipboard merges the PDF files copied (${FC:-no button}; $(npages $OUT/clipfiles.pdf 2>/dev/null) pages)" "[ \"\$(npages $OUT/clipfiles.pdf 2>/dev/null)\" = 5 ]"
quit

# 82. A thumbnail's menu ▸ Insert a file's pages before… (Acrobat's Before): blank.pdf goes before page 1, so the
# document's first page is now the blank one and "Page 1" its second.
convert -size 612x792 xc:white "$OUT/blank.pdf"; cp "$OUT/text.pdf" "$OUT/before.pdf"; launch "$OUT/before.pdf"; sleep 1
answer "$PWD/$OUT/blank.pdf"; xdotool mousemove 120 230 click 3; sleep 0.8; BF=$(at before 300x400+20+200); [ -n "$BF" ] && click $BF 1.5; answer ""; key ctrl+s; sleep 1.5
check "Insert a file's pages before page 1 (${BF:-no item}; $(npages $OUT/before.pdf) pages, page 2: $(first_text "$OUT/before.pdf" 2))" "[ -z \"\$(first_text $OUT/before.pdf 1)\" ] && [ \"\$(first_text $OUT/before.pdf 2)\" = 'Page 1' ]"
quit

# 83. A text box typed in place, then another tool picked in the rail (the Pen, 282,348): the text is added, not
# thrown away (Sol, 26/10).
cp "$OUT/text.pdf" "$OUT/switch.pdf"; launch "$OUT/switch.pdf"; sleep 1
key t; sleep 0.5; click 700 500 0.8; xdotool type --delay 20 "Kept on switch"; click 282 348 1; key ctrl+s; sleep 1.5
check "text typed in place is kept when another tool is picked" "qpdf --qdf --object-streams=disable $OUT/switch.pdf - 2>/dev/null | grep -aq 'Kept on switch'"
quit

# 84. Shapes (R) ▸ "- -" (Acrobat's Dashed line style): a rectangle drawn is saved dashed (/BS /S /D).
cp "$OUT/text.pdf" "$OUT/dashed.pdf"; launch "$OUT/dashed.pdf"; sleep 1; key r; sleep 1
# "- -" sits just left of the opacity's "100 %" (tesseract reads it merged with the width dots before it, and
# the opacity as "100" or "10%"); the Shapes options take two rows.
OP=$(at 10 1280x90+0+80); DD=$([ -n "$OP" ] && set -- $OP && echo "$(( $1 - 48 )) $2"); [ -n "$DD" ] && click $DD 0.4
xdotool mousemove 520 420 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( 520 + i * 30 )) $(( 420 + i * 20 )); sleep 0.05; done; xdotool mouseup 1; sleep 1; key ctrl+s; sleep 1.5
DQ=$(qpdf --qdf --object-streams=disable "$OUT/dashed.pdf" - 2>/dev/null)
check "a dashed rectangle is saved dashed (${DD:-no button})" "echo \"\$DQ\" | grep -aq '/S /D'"
quit

# 85. Organize ▸ Odd, then even (Acrobat's odd-and-even rule): text.pdf's pages come out 1, 3, 2, 4.
cp "$OUT/text.pdf" "$OUT/oddeven.pdf"; launch "$OUT/oddeven.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "organize"; key Return; sleep 3
OE=$(at Odd 280x700+1000+40); [ -n "$OE" ] && click $OE 0.5; click 1140 755 4
check "Organize ▸ Odd, then even orders the pages 1, 3, 2, 4 (${OE:-no option}; page 2: $(first_text "$OUT/oddeven-organized.pdf" 2))" "[ \"\$(first_text $OUT/oddeven-organized.pdf 2)\" = 'Page 3' ] && [ \"\$(first_text $OUT/oddeven-organized.pdf 3)\" = 'Page 2' ]"
quit

# 86. A bookmark's ⋯ menu ▸ Bold, then ▸ Set destination to this view at 200 % (Acrobat's bookmark Appearance and Set
# Destination): saved, its /F holds bold (2) and its /Dest is /XYZ with a zoom of 2.
cp "$OUT/text.pdf" "$OUT/markstyle.pdf"; launch "$OUT/markstyle.pdf"; key ctrl+9; sleep 1
click 95 106 0.8; click 55 149 1
xdotool mousemove 150 182; sleep 0.5; click 199 182 0.8; BO=$(at Bold 260x400+0+180); [ -n "$BO" ] && click $BO 1
key ctrl+0; sleep 0.8; xdotool mousemove 150 182; sleep 0.5; click 199 182 0.8; SD=$(at Set 260x400+0+180); [ -n "$SD" ] && click $SD 1
key ctrl+s; sleep 1.5
BQ=$(qpdf --qdf --object-streams=disable "$OUT/markstyle.pdf" - 2>/dev/null | python3 -c '
import re, sys
d = sys.stdin.buffer.read().decode("latin-1")
o = [b for b in re.split(r"\nendobj", d) if "/Title" in b and "/Parent" in b]
print(" ".join(("F" + m.group(1)) if (m := re.search(r"/F (\d+)", b)) else "F0" for b in o), " ".join(re.sub(r"\s+", " ", m.group(1)) for b in o for m in [re.search(r"/Dest \[([^\]]*)\]", b)] if m))')
check "a bookmark made bold and pointed at a view keeps both (${BO:-no Bold} ${SD:-no Set}; $BQ)" "echo \"$BQ\" | grep -q 'F2' && echo \"$BQ\" | grep -q 'XYZ'"
quit

# 87. A thumbnail's menu ▸ Replace with a file's pages… from a longer file (text.pdf itself, four pages): the strip under
# the pages asks from which page (Acrobat's pages … of the file); 3 typed: page 1 is now text.pdf's page 3.
cp "$OUT/text.pdf" "$OUT/repfrom.pdf"; launch "$OUT/repfrom.pdf"; sleep 1
answer "$PWD/$OUT/text.pdf"; xdotool mousemove 120 230 click 3; sleep 0.8; RP=$(at Replace 300x400+20+200); [ -n "$RP" ] && click $RP 1.5; answer ""
# The field, Cancel and Replace make the strip's last row, 44 px under the "from page" that ends its sentence
# (Cancel, bold on grey, and the faint placeholder are not read): the field at x 40, Replace at x 200.
CA=$(at from 248x200+0+560); [ -n "$CA" ] && { set -- $CA; click 40 $(( $2 + 44 )) 0.4; xdotool type --delay 20 "3"; click 200 $(( $2 + 44 )) 1.5; }
key ctrl+s; sleep 1.5
check "Replace with a longer file's pages takes the ones chosen (${RP:-no item} ${CA:-no strip}; page 1: $(first_text "$OUT/repfrom.pdf" 1))" "[ \"\$(first_text $OUT/repfrom.pdf 1)\" = 'Page 3' ]"
quit

# 88. A tab's menu ▸ Move to New Window (PDF Expert's, a browser's): the tab closes and a second Sheaf opens the file
# in a window of its own (two Sheaf windows on screen). Then, the first window raised, its other tab's menu ▸ Merge All
# Windows (macOS's Window menu): the second window hands its document over and quits; two tabs here again.
cp "$OUT/text.pdf" "$OUT/mover.pdf"; cp "$OUT/text.pdf" "$OUT/stayer.pdf"; launch "$OUT/stayer.pdf" "$OUT/mover.pdf"; sleep 1.5
read AX _ <<< "$(at mover 1280x40+0+0)"; xdotool mousemove "${AX:-0}" 21 click 3; sleep 0.8
MW=$(at New 1280x400+0+0); [ -n "$MW" ] && click $MW 0.5
for _ in $(seq 1 40); do [ "$(xdotool search --name '^Sheaf$' 2>/dev/null | wc -l)" -ge 2 ] && break; sleep 0.25; done; sleep 1.5
NW=$(xdotool search --name '^Sheaf$' 2>/dev/null | wc -l); MP=$(pgrep -f "mover.pdf" | grep -v "^$APP_PID$" | head -1); NT=$(ntabs)
check "Move to New Window opens the file in a second window and closes its tab (${MW:-no item}; $NW windows, $NT tab left)" "[ \"$NW\" -ge 2 ] && [ -n \"$MP\" ] && [ \"$NT\" = 1 ]"
xdotool windowraise "$W"; sleep 0.5
read AX _ <<< "$(at stayer 1280x40+0+0)"; xdotool mousemove "${AX:-0}" 21 click 3; sleep 0.8
MG=$(at Merge 1280x400+0+0); [ -n "$MG" ] && click $MG 0.5
for _ in $(seq 1 40); do [ -z "$MP" ] || ! kill -0 "$MP" 2>/dev/null && break; sleep 0.25; done; sleep 1
GONE=$( [ -n "$MP" ] && ! kill -0 "$MP" 2>/dev/null && echo quit )
check "Merge All Windows brings the other window's document here and closes it (${MG:-no item}; ${GONE:-still open}; $(ntabs) tabs)" "[ \"$GONE\" = quit ] && [ \"\$(ntabs)\" = 2 ]"
[ -n "$MP" ] && kill "$MP" 2>/dev/null
quit

# 89. Prepare form: a text field drawn, its card's Copies 3 down × 1 across ▸ Make (Acrobat's Create Multiple Copies):
# two more separate fields under it, saved (three /T in all). The down box is right of the "Copies" label.
cp "$OUT/text.pdf" "$OUT/copies.pdf"; launch "$OUT/copies.pdf"; key ctrl+9; sleep 1; key f; sleep 0.6
xdotool mousemove 500 300 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( 500 + i * 40 )) $(( 300 + i * 6 )); sleep 0.05; done; xdotool mouseup 1; sleep 1.5
CP=$(at Copies 400x400+490+330); [ -n "$CP" ] && { set -- $CP; click $(( $1 + 48 )) $2 0.4; xdotool type --delay 20 "3"; }
MK=$(at Make 400x400+490+330); [ -n "$MK" ] && click $MK 1.2; key ctrl+s; sleep 1.5
CQ=$(qpdf --qdf --object-streams=disable "$OUT/copies.pdf" - 2>/dev/null | grep -ac '/T (Text1')
check "Copies down make separate fields under the one selected (${CP:-no row} ${MK:-no Make}; ${CQ:-0} fields)" "[ \"\${CQ:-0}\" = 3 ]"
quit

# 90. Home ▸ From scanner (Acrobat's Create PDF ▸ Scanner): the page scanned (the test seam gives stamp.png, green)
# becomes a one-page PDF, named in the Save dialog and opened.
convert -size 80x40 xc:'#2a9d4b' "$OUT/stamp.png"; launch; answer "$PWD/$OUT/scanned.pdf"; FN=$(at scanner 1000x120+250+80); [ -n "$FN" ] && click $FN 1
for _ in $(seq 1 12); do [ "$(ntabs)" = 1 ] && break; sleep 0.5; done; answer ""
SN=$(pdftoppm -r 20 -f 1 -l 1 -png "$OUT/scanned.pdf" 2>/dev/null | convert - -format "%[pixel:p{5,5}]" info: 2>/dev/null)
check "From scanner makes a PDF of the page scanned and opens it (${FN:-no button}; $SN; $(ntabs) tabs)" "[ \"\$(npages $OUT/scanned.pdf 2>/dev/null)\" = 1 ] && echo \"$SN\" | grep -q 'srgb(42,15[0-9],7[0-9])' && [ \"\$(ntabs)\" = 1 ]"
quit

# 91. A form's Export data… (Acrobat's Export Data, as CSV: a header row of field names, a row of values), then a
# blank copy's Import data… of that file: the value typed in the first is in the second, saved.
cp "$OUT/form.pdf" "$OUT/data-a.pdf"; cp "$OUT/form.pdf" "$OUT/data-b.pdf"; launch "$OUT/data-a.pdf"; key ctrl+9
click 640 300 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; key ctrl+a; xdotool type --delay 10 "Exported value"; key Return; sleep 1
answer "$PWD/$OUT/data.csv"; EX=$(at Export 1280x40+0+90); [ -n "$EX" ] && click $EX 1.5; answer ""
quit
launch "$OUT/data-b.pdf"; key ctrl+9
answer "$PWD/$OUT/data.csv"; IM=$(at Import 1280x40+0+90); [ -n "$IM" ] && click $IM 1.5; answer ""; key ctrl+s; sleep 1.5
DV=$(qpdf --json=2 --json-key=acroform "$OUT/data-b.pdf" | python3 -c "import json,sys;print([f.get('value') for f in json.load(sys.stdin)['acroform']['fields']])")
check "form data exported to CSV and imported into a blank copy (${EX:-no Export} ${IM:-no Import}; $(head -c 60 $OUT/data.csv 2>/dev/null | tr '\n' '|'); $DV)" "grep -q 'Exported value' $OUT/data.csv 2>/dev/null && echo \"$DV\" | grep -q 'Exported value'"
quit

# 92. More ▸ Overlay a PDF… (PDF Expert's overlay): stamp-text.pdf chosen (one Letter page, "Overlay mark" near its
# foot, clear of text.pdf's line: text drawn on text.pdf's own words is read mixed with them) is drawn over each page
# of a copy of text.pdf; saved, page 2 shows its own text and the overlay's.
printf '%%PDF-1.4\n1 0 obj <</Type /Catalog /Pages 2 0 R>> endobj\n2 0 obj <</Type /Pages /Kids [3 0 R] /Count 1>> endobj\n3 0 obj <</Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources <</Font <</F1 5 0 R>>>> /Contents 4 0 R>> endobj\n4 0 obj <</Length 44>> stream\nBT /F1 24 Tf 72 100 Td (Overlay mark) Tj ET\nendstream endobj\n5 0 obj <</Type /Font /Subtype /Type1 /BaseFont /Helvetica>> endobj\ntrailer <</Size 6 /Root 1 0 R>>\n%%%%EOF\n' > "$OUT/stamp-text.raw"
qpdf "$OUT/stamp-text.raw" "$OUT/stamp-text.pdf" 2>/dev/null
cp "$OUT/text.pdf" "$OUT/overlay.pdf"; launch "$OUT/overlay.pdf"; sleep 1
answer "$PWD/$OUT/stamp-text.pdf"; click 1210 61 0.8; click 1008 659 0.8; OV=$(at Overlay 300x400+880+80); [ -n "$OV" ] && click $OV 1.5; answer ""; key ctrl+s; sleep 1.5
O2=$(pdftotext -f 2 -l 2 "$OUT/overlay.pdf" - 2>/dev/null | tr '\n' ' '); S1=$(pdftotext -f 1 -l 1 "$OUT/stamp-text.pdf" - 2>/dev/null | tr '\n' ' ' | awk '{print $1, $2}')
check "Overlay a PDF draws its page over each page (${OV:-no item}; overlay: ${S1:-none}; page 2: $(echo "$O2" | cut -c1-60))" "[ \"$S1\" = 'Overlay mark' ] && echo \"$O2\" | grep -q 'Page 2 the quick' && echo \"$O2\" | grep -qF \"$S1\""
quit

# 93. A tool's job done (the test seam announces every one; a real run, those past 10 s): a system notification
# names the tool and the file made (Organize ▸ Odd, then even, step 85).
check "a finished job sends a system notification ($(grep -m1 Organize $OUT/notified 2>/dev/null))" "grep -q 'Organize PDF finished: oddeven-organized.pdf is ready' $OUT/notified"

# 94. More ▸ Read aloud (Acrobat's Read Out Loud): from the page shown to the end, each page's text spoken (the test
# seam writes what the voice is given).
cp "$OUT/text.pdf" "$OUT/aloud.pdf"; launch "$OUT/aloud.pdf"; sleep 1
click 1210 61 0.8; click 1008 659 0.8; RD=$(at Read 300x400+880+80); [ -n "$RD" ] && click $RD 3
check "Read aloud gives every page's text to the voice (${RD:-no item}; $(tr '\n' ' ' < $OUT/spoken 2>/dev/null | cut -c1-60))" "grep -q 'Page 1' $OUT/spoken && grep -q 'Page 4' $OUT/spoken"
quit

# 95. A thumbnail's menu ▸ Insert from scanner… (Acrobat's Insert ▸ From Scanner; the test seam scans stamp.png) puts
# the page scanned after page 1, then Insert from clipboard (text copied) a page of that text after it: six pages.
convert -size 80x40 xc:'#2a9d4b' "$OUT/stamp.png"; cp "$OUT/text.pdf" "$OUT/inserted.pdf"; launch "$OUT/inserted.pdf"; sleep 1
xdotool mousemove 120 230 click 3; sleep 0.8; IS=$(at scanner 300x420+20+200); [ -n "$IS" ] && click $IS 2
printf 'Pasted words' | xclip -selection clipboard; sleep 0.3
xdotool mousemove 120 230 click 3; sleep 0.8; IC=$(at clipboard 300x420+20+200); [ -n "$IC" ] && click $IC 2; key ctrl+s; sleep 1.5
check "Insert from scanner and from clipboard add their pages after page 1 (${IS:-no scanner item} ${IC:-no clipboard item}; $(npages $OUT/inserted.pdf) pages; page 2: $(first_text "$OUT/inserted.pdf" 2))" "[ \"\$(npages $OUT/inserted.pdf)\" = 6 ] && [ \"\$(first_text $OUT/inserted.pdf 2)\" = 'Pasted words' ]"
quit

# 96. More ▸ Save a flattened copy… (PDF Expert's Export ▸ Flattened Copy): a form filled, its copy has the value drawn
# into the page and no field left; the document itself keeps its field.
cp "$OUT/form.pdf" "$OUT/toflat.pdf"; launch "$OUT/toflat.pdf"; key ctrl+9
click 640 300 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; key ctrl+a; xdotool type --delay 10 "Drawn in"; key Return; sleep 1
answer "$PWD/$OUT/flat-copy.pdf"; click 1210 61 0.8; click 1008 659 0.8; FC=$(at flattened 300x400+880+80); [ -n "$FC" ] && click $FC 2; answer ""
NF=$(qpdf --json=2 --json-key=acroform "$OUT/flat-copy.pdf" 2>/dev/null | python3 -c "import json,sys;print(len(json.load(sys.stdin)['acroform']['fields']))" 2>/dev/null)
check "Save a flattened copy draws the value in and leaves no field (${FC:-no item}; ${NF:-?} fields; $(pdftotext $OUT/flat-copy.pdf - 2>/dev/null | tr '\n' ' ' | cut -c1-40))" "[ \"\${NF:-1}\" = 0 ] && pdftotext $OUT/flat-copy.pdf - | grep -q 'Drawn in' && [ \"\$(qpdf --json=2 --json-key=acroform $OUT/toflat.pdf | python3 -c 'import json,sys;print(len(json.load(sys.stdin)[\"acroform\"][\"fields\"]))')\" -ge 1 ]"
quit

# 97. More ▸ Loupe (Acrobat's Loupe tool): the pointer over "Page 1", the panel in the view's bottom-right corner
# shows those words three times larger.
cp "$OUT/text.pdf" "$OUT/loupe.pdf"; launch "$OUT/loupe.pdf"; sleep 1
LW=$(at Page 1020x300+255+80); click 1210 61 0.8; click 1008 659 0.8; LP=$(at Loupe 300x400+880+80); [ -n "$LP" ] && click $LP 0.8
# The pointer ends on the word's middle: magnified three times, the whole word fits the panel (at the view's
# 163 %, 20 px to its right cut the P off).
[ -n "$LW" ] && { set -- $LW; xdotool mousemove $(( $1 - 10 )) $2; sleep 0.3; xdotool mousemove $1 $2; sleep 1.5; }
LO=$(ocr 400x280+860+500)
check "the loupe shows the words under the pointer, magnified (${LP:-no item}; $(echo "$LO" | cut -c1-40))" "echo \"$LO\" | grep -q 'Page'"
quit

# 98. Document properties (⌘D) ▸ Save report… (Acrobat's PDF info report, saved): a JSON file with the facts and fonts.
cp "$OUT/text.pdf" "$OUT/report2.pdf"; launch "$OUT/report2.pdf"; sleep 1; key ctrl+d; sleep 2
answer "$PWD/$OUT/report.json"; SR=$(at Save 400x44+400+722); [ -n "$SR" ] && click $SR 1; answer ""
RJ=$(python3 -c 'import json,sys; v=json.load(open(sys.argv[1])); print(v["file"], v["facts"].get("Pages"), v["fonts"][0]["name"])' "$OUT/report.json" 2>/dev/null)
check "Save report writes the properties as JSON (${SR:-no button}; $RJ)" "[ \"$RJ\" = 'report2.pdf 4 Helvetica' ]"
quit

# 99. Preferences ▸ Open PDFs with Sheaf by default (Acrobat's and PDF Expert's prompt): the system is asked (the test
# seam records the request).
# Its button heads the Documents section, mid-page since Preferences gained sections: scrolled to a few rows at a
# time, then read in the strip under that title (Identity's note also says "PDFs").
launch; prefs_open; MD=
for _ in $(seq 1 10); do DT=$(at Documents 900x620+200+90); [ -n "$DT" ] && { set -- $DT; MD=$(at PDFs 900x60+200+$(( $2 + 8 ))); break; }; xdotool mousemove 700 500 click --repeat 3 5; sleep 0.5; done
[ -n "$MD" ] && click $MD 1
check "Preferences makes Sheaf the default PDF app (${MD:-no button})" "[ \"\$(cat $OUT/made-default 2>/dev/null)\" = default ]"
quit

# 100. Home ▸ From clipboard with a PDF itself on the clipboard (application/pdf, as Preview and Acrobat copy pages):
# that PDF becomes the new one.
qpdf "$OUT/text.pdf" --pages . 1 -- "$OUT/single.pdf"; xclip -selection clipboard -t application/pdf < "$OUT/single.pdf"; sleep 0.3
launch; answer "$PWD/$OUT/clippdf.pdf"; FP=$(at From 900x120+300+80); [ -n "$FP" ] && click $FP 1; for _ in $(seq 1 12); do [ -f "$OUT/clippdf.pdf" ] && break; sleep 0.5; done; answer ""
check "From clipboard takes a PDF copied as data (${FP:-no button}; $(npages $OUT/clippdf.pdf 2>/dev/null) pages, $(first_text "$OUT/clippdf.pdf" 1 2>/dev/null))" "[ \"\$(npages $OUT/clippdf.pdf 2>/dev/null)\" = 1 ] && [ \"\$(first_text $OUT/clippdf.pdf 1)\" = 'Page 1' ]"
quit

# 101. Merge ▸ Pages: pb.pdf's three page cards all dropped (the fifth card's × three times), then Files and Pages
# again: the order made is kept, pb.pdf's pages do not come back (Sol, 26/10: they did).
mkdir -p "$OUT/reo"; convert -size 200x260 xc:'#dd2222' xc:'#22aa22' xc:'#2222dd' xc:'#dddd22' "$OUT/reo/pa.pdf"; convert -size 200x260 xc:'#111111' xc:'#777777' xc:'#bbbbbb' "$OUT/reo/pb.pdf"
launch "$OUT/reo/pa.pdf" "$OUT/reo/pb.pdf"
click 50 21 1; click 67 113 1; click 271 197 1.5; click 494 405 1; click 460 527 2
PG=$(at Pages 280x400+1000+60); [ -n "$PG" ] && click $PG 2
for _ in 1 2 3; do xdotool mousemove $(( 41 + 124 * 4 + 54 )) 140; sleep 0.4; click $(( 41 + 124 * 4 + 93 )) 94 1; done
FI=$(at Files 280x400+1000+60); [ -n "$FI" ] && click $FI 1.5; PG2=$(at Pages 280x400+1000+60); [ -n "$PG2" ] && click $PG2 2
# The cards' pictures (124 px apart from x 95, y 150): pa.pdf's four colours, and no fifth card (the area's grey).
px_at() { import -window root -crop "1x1+$1+150" png:- 2>/dev/null | convert - -format '%[pixel:p{0,0}]' info: 2>/dev/null | tr -d ' '; }
CARDS="$(px_at 95) $(px_at 219) $(px_at 343) $(px_at 467) $(px_at 591)"; EMPTY=$(px_at 900)
check "Merge keeps a file's pages dropped through Files and back (${FI:-no Files} ${PG2:-no Pages}; cards $CARDS, area $EMPTY)" "[ \"$(echo $CARDS | wc -w)\" = 5 ] && [ \"$(echo $CARDS | cut -d' ' -f5)\" = \"$EMPTY\" ] && [ \"$(echo $CARDS | cut -d' ' -f4)\" != \"$EMPTY\" ]"
quit

# 102. A path on the command line that names no file (a PDF moved): said once the window shows, not dropped silently.
launch "$OUT/moved-away.pdf"; NF=$(ocr 1280x800+0+0)
check "a missing file asked on the command line is said ($(echo "$NF" | grep -o 'Not found[^|]*' | head -1))" "echo \"$NF\" | grep -q 'Not found: moved-away.pdf'"
quit

# 103. ⇧⌘S into a folder that cannot be written: the save fails, and the tab is still the original (Sol, 26/10: it
# took the new name first): opening the original again shows its tab, no second one.
mkdir -p "$OUT/locked"; chmod 555 "$OUT/locked"; cp "$OUT/text.pdf" "$OUT/keepme.pdf"; launch "$OUT/keepme.pdf"; sleep 1
answer "$PWD/$OUT/locked/copy.pdf"; key ctrl+shift+s; sleep 2; click 40 21 1; answer "$PWD/$OUT/keepme.pdf"; key ctrl+o; sleep 2; answer ""
KT=$(ntabs); chmod 755 "$OUT/locked"
check "a failed Save As leaves the tab the original's ($KT tab)" "[ \"$KT\" = 1 ] && [ ! -e \"$OUT/locked/copy.pdf\" ]"
quit

# 104. The page grid (More ▸ Page grid): the yellow page (the last) dragged onto the left half of the red one (the
# first) goes before it (Acrobat's Organize Pages): saved, the first page is yellow.
convert -size 300x400 xc:'#E02020' xc:'#20A020' xc:'#2040E0' xc:'#E0C020' "$OUT/griddrag.pdf"; launch "$OUT/griddrag.pdf"; sleep 1
click 1210 61 0.8; click 1008 267 1.5
GY=226; GR=$(first_x r $GY); GB=$(first_x b $GY)
# Yellow is red and green high: the last card, right of the blue one (cards are about as wide as blue's offset).
GYW=${GB:+$(( GB + (GB - ${GR:-0}) / 2 + 20 ))}
if [ -n "$GR" ] && [ -n "$GYW" ]; then
    xdotool mousemove $GYW $GY mousedown 1; for i in 1 2 3 4 5 6 7 8; do xdotool mousemove $(( GYW - (GYW - GR - 10) * i / 8 )) $GY; sleep 0.06; done; xdotool mouseup 1; sleep 1.5
fi
key ctrl+s; sleep 1.5
P1=$(pdftoppm -r 10 -f 1 -l 1 -png "$OUT/griddrag.pdf" 2>/dev/null | convert - -format "%[pixel:p{5,5}]" info: 2>/dev/null)
check "a page dragged in the page grid goes where it is dropped (red ${GR:-?}, from ${GYW:-?}; page 1 $P1)" "echo \"$P1\" | grep -qE 'srgb\\((22[0-9]|2[0-4][0-9]),(19[0-9]|20[0-9]),'"
quit

# 105. More ▸ More tools ▸ Remove all images (Acrobat's): image.pdf's pictures go, its text stays; saved.
cp "$OUT/image.pdf" "$OUT/noimages.pdf"; launch "$OUT/noimages.pdf"; sleep 1
click 1210 61 0.8; click 1008 659 0.8; RI=$(at Remove 300x400+880+80); [ -n "$RI" ] && click $RI 1.5; key ctrl+s; sleep 1.5
NI=$(pdfimages -list "$OUT/noimages.pdf" 2>/dev/null | tail -n +3 | wc -l)
check "Remove all images takes every picture out (${RI:-no item}; $NI left)" "[ \"$NI\" = 0 ]"
quit

# 106. More ▸ More tools ▸ Reflow (Acrobat's): the page's text across the window, larger with A+.
cp "$OUT/text.pdf" "$OUT/reflow.pdf"; launch "$OUT/reflow.pdf"; sleep 1
click 1210 61 0.8; click 1008 659 0.8; RF=$(at Reflow 300x400+880+80); [ -n "$RF" ] && click $RF 1; RT=$(ocr 1020x300+255+90)
check "Reflow shows the page's text on the window's width (${RF:-no item}; $(echo "$RT" | cut -c1-60))" "echo \"$RT\" | grep -q 'Reflow' && echo \"$RT\" | grep -q 'quick brown fox'"
quit

# 107. Auto-Complete (Acrobat's): "Smoke Test" typed in step 6's form is remembered; in a copy, "Sm" typed in the field
# offers it under the field, and a click on it fills the field (saved).
cp "$OUT/form.pdf" "$OUT/autocomplete.pdf"; launch "$OUT/autocomplete.pdf"; key ctrl+9
click 640 300 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; key ctrl+a; xdotool type --delay 10 "Remembered entry"; key Return; sleep 1; key ctrl+s; sleep 1
click 640 300 0.8; xdotool windowfocus --sync "$W" 2>/dev/null; key ctrl+a; xdotool type --delay 30 "Rem"; sleep 1
AC=$(at Remembered 1280x700+0+90); [ -n "$AC" ] && click $AC 1; key ctrl+s; sleep 1.5
AV=$(qpdf --json=2 --json-key=acroform "$OUT/autocomplete.pdf" | python3 -c "import json,sys;print([f.get('value') for f in json.load(sys.stdin)['acroform']['fields']])")
check "a form field offers an entry typed before, and a click fills it (${AC:-no suggestion}; $AV)" "echo \"$AV\" | grep -q 'Remembered entry'"
quit

# 108. Layers panel (Acrobat's Layers pane; the fifth tab, 218,106 (five share the row), shown when the document has layers): a
# page with a red square in layer Red and a blue one in layer Blue, Blue hidden by the file. A click on Red hides
# it, Show all shows both; the file keeps its own choice.
python3 - "$OUT/layers-raw.pdf" <<'PY'
import sys
c = b"/OC /R BDC 1 0 0 rg 60 300 200 200 re f EMC /OC /B BDC 0 0 1 rg 340 300 200 200 re f EMC"
objs = [b"<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [4 0 R 5 0 R] /D << /Order [4 0 R 5 0 R] /OFF [5 0 R] >> >> >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Contents 6 0 R /Resources << /Properties << /R 4 0 R /B 5 0 R >> >> >>",
        b"<< /Type /OCG /Name (Red) >>", b"<< /Type /OCG /Name (Blue) >>",
        b"<< /Length %d >>\nstream\n" % len(c) + c + b"\nendstream"]
open(sys.argv[1], "wb").write(b"%PDF-1.7\n" + b"".join(b"%d 0 obj\n" % (i + 1) + o + b"\nendobj\n" for i, o in enumerate(objs)) + b"trailer\n<< /Root 1 0 R >>\n%%EOF\n")
PY
qpdf "$OUT/layers-raw.pdf" "$OUT/layers.pdf" 2>/dev/null; cp "$OUT/layers.pdf" "$OUT/layers-before.pdf"
launch "$OUT/layers.pdf"; sleep 1
share() { import -window root -crop 1000x700+260+90 png:- 2>/dev/null | convert - -fuzz 25% -fill black +opaque "$1" -fill white -opaque "$1" -format "%[fx:mean>0.002]" info: 2>/dev/null; }
L0="$(share '#ff0000')$(share '#0000ff')"
click 218 106 1; LR=$(at Red 300x300+0+120); [ -n "$LR" ] && click $LR 1.5
L1="$(share '#ff0000')$(share '#0000ff')"
LS=$(at Show 300x200+0+110); [ -n "$LS" ] && click $LS 1.5
L2="$(share '#ff0000')$(share '#0000ff')"
check "Layers: as the file sets them, Red hidden by a click, Show all (red,blue: $L0 → $L1 → $L2; ${LR:-no Red row})" "[ \"$L0\" = 10 ] && [ \"$L1\" = 00 ] && [ \"$L2\" = 11 ] && cmp -s \"$OUT/layers.pdf\" \"$OUT/layers-before.pdf\""
quit

# 109. More ▸ More tools ▸ Accessibility check (Acrobat's Full Check): text.pdf has no title, so Title fails; its
# row's Fix sets one (the file's name) and the check runs again; saved, the file has a title shown in the window.
cp "$OUT/text.pdf" "$OUT/a11y.pdf"; launch "$OUT/a11y.pdf"; sleep 1
click 1210 61 0.8; click 1008 659 0.8; AK=$(at Accessibility 300x500+880+80); [ -n "$AK" ] && click $AK 3
A1=$(ocr 640x560+320+120 | tr -s ' ')
# The Fix on the Title row: the row's y, the Fix button's x (the buttons line up on the right).
read _ TY <<< "$(at Title 600x520+330+150)"; read FX _ <<< "$(at Fix 640x560+320+120)"
[ -n "$TY" ] && [ -n "$FX" ] && click $FX $TY 4
key Escape; key ctrl+s; sleep 1.5
TT=$(pdfinfo "$OUT/a11y.pdf" 2>/dev/null | sed -n 's/^Title: *//p')
check "the accessibility check lists failed rules and its Fix gives the document a title (${AK:-no item}; $(echo "$A1" | grep -o '[0-9]* failed' | head -1); $TT)" "echo \"$A1\" | grep -q 'failed' && [ -n \"$TT\" ] && qpdf --qdf --object-streams=disable \"$OUT/a11y.pdf\" - 2>/dev/null | grep -aq '/DisplayDocTitle true'"
quit

# 110. More ▸ More tools ▸ Copy the file (PDF Expert's Share ▸ Copy): the document as it is now, a highlight
# not saved yet included, put on the clipboard as a file: read back from the X clipboard the way a
# file manager pastes it (GNOME's x-special/gnome-copied-files: "copy" and the file's URI).
cp "$OUT/text.pdf" "$OUT/share.pdf"; launch "$OUT/share.pdf"; sleep 1; printf x | xclip -selection clipboard
key h; read QX QY <<< "$(at quick 1020x700+255+90)"; [ -n "$QY" ] && { xdotool mousemove $(( QX - 30 )) $QY mousedown 1 mousemove $(( QX + 60 )) $QY mouseup 1; sleep 1; }; key v
click 1210 61 0.8; click 1008 659 0.8; CF=$(at Copy 300x700+880+80); [ -n "$CF" ] && click $CF 1.5
SF=$(xclip -o -selection clipboard -t x-special/gnome-copied-files 2>/dev/null | sed -n '1{/^copy$/!q};2{s#^file://##;s/%20/ /g;p}')
check "Copy the file puts the document as it is now on the clipboard (${CF:-no item}; $(basename "$SF"))" "[ -f \"$SF\" ] && qpdf --qdf --object-streams=disable \"$SF\" - 2>/dev/null | grep -aq '/Subtype /Highlight' && ! qpdf --qdf --object-streams=disable \"$OUT/share.pdf\" - 2>/dev/null | grep -aq '/Subtype /Highlight'"
quit

# 111. More ▸ More tools ▸ Rulers (Acrobat's): rulers along the page's top and left edge (#F2F2F2 at
# 90 % over the white page, #F3F3F3); a click on the top ruler makes an upright guide (#00A6D6) across
# the page, a second click on its mark takes it away.
cp "$OUT/text.pdf" "$OUT/rulers.pdf"; launch "$OUT/rulers.pdf"; sleep 1
click 1210 61 0.8; click 1008 659 0.8; RU=$(at Rulers 300x700+880+80); [ -n "$RU" ] && click $RU 1.5
# The rulers' box: where #F3F3F3 is in the view (the corner the two rulers share is the page's).
RB=$(import -window root -crop 1000x700+260+90 png:- 2>/dev/null | convert - +repage -fuzz 1% -fill black -opaque '#f3f3f3' -fill white +opaque black -negate -trim -format '%X %Y' info: 2>/dev/null | tr -d '+')
read RX RY <<< "$RB"
# A guide is one pixel wide: a few hundred pixels of the view.
guide() { import -window root -crop 1000x700+260+90 png:- 2>/dev/null | convert - -fuzz 12% -fill black +opaque '#00a6d6' -fill white -opaque '#00a6d6' -format "%[fx:mean>0.0002]" info: 2>/dev/null; }
G0=$(guide)
[ -n "$RY" ] && click $(( 260 + RX + 150 )) $(( 90 + RY + 7 )) 1
G1=$(guide)
[ -n "$RY" ] && click $(( 260 + RX + 150 )) $(( 90 + RY + 7 )) 1
G2=$(guide)
check "a click on the top ruler makes a guide, a second takes it away (${RU:-no item}; ruler at ${RB:-none}; guide $G0 → $G1 → $G2)" "[ \"$G0\" = 0 ] && [ \"$G1\" = 1 ] && [ \"$G2\" = 0 ]"
quit

# 200. More ▸ More tools ▸ Scrolling, split view, hand ▸ Scroll pages across (PDF Expert's Horizontal scrolling): the
# pages in one strip, a page fitting the view's height, page 1 (red) at the strip's start; → brings page 2 (green) to the middle,
# and a mouse's wheel moves along the strip.
convert -size 300x400 xc:'#E02020' xc:'#20A020' xc:'#2040E0' xc:'#E0C020' "$OUT/across.pdf"; launch "$OUT/across.pdf"; sleep 1
# More (1127,61) ▸ More tools… (1008,659) ▸ Scrolling, split view, hand ›: the 21st entry, 28 px apart from 100, so
# at 660 too; then the entry of the small menu starting with the word $1, found by OCR.
layout_item() { click 1210 61 0.8; click 1008 659 0.8; click 1008 660 0.8; local i; i=$(at "$1" 340x300+840+70); [ -n "$i" ] && click $i 1.2; echo "${i:-none}"; }
# A pixel's colour at (x, 420) as #RRGGBB (8 bits: colour_at reads 16-bit colours as #RRRRGG on this ImageMagick).
c8() { import -window root -crop "1x1+$1+420" -depth 8 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}' | head -1; }
greenish() { python3 -c 'import sys; v = sys.argv[1].lstrip("#"); r, g = int(v[0:2], 16), int(v[2:4], 16); sys.exit(0 if g > r + 80 else 1)' "$1"; }
viewsig() { import -window root -crop 1000x700+260+90 png:- 2>/dev/null | md5sum | cut -c1-8; }
HI=$(layout_item across); sleep 0.5
H0=$(c8 500); key Right; sleep 1; H1=$(c8 768)
S0=$(viewsig); xdotool mousemove 768 420 click --repeat 4 5; sleep 1; S1=$(viewsig)
check "pages run across: page 1 in the middle, → page 2, the wheel moves along ($HI; $H0 → $H1; $S0 → $S1)" "reddish '${H0:-#000000}' && greenish '${H1:-#000000}' && [ \"$S0\" != \"$S1\" ]"
quit

# 201. … ▸ Split view side by side (PDF Expert's Split View): both panes on page 1; End moves the first pane only,
# to page 4 (yellow); with Sync scroll on, End moves the second pane with it.
cp "$OUT/across.pdf" "$OUT/split.pdf"; launch "$OUT/split.pdf"; sleep 1
yellowish() { python3 -c 'import sys; v = sys.argv[1].lstrip("#"); r, g, b = (int(v[k:k + 2], 16) for k in (0, 2, 4)); sys.exit(0 if r > 180 and g > 150 and b < 100 else 1)' "$1"; }
SI=$(layout_item side); sleep 1
L0=$(c8 510); R0=$(c8 1020)
key End; sleep 1; L1=$(c8 510); R1=$(c8 1020)
key Home; sleep 1; SY=$(at Sync 400x44+880+84); [ -n "$SY" ] && click $SY 0.8
key End; sleep 1; L2=$(c8 510); R2=$(c8 1020)
check "split view: two panes, each scrolled on its own, then together ($SI; $L0 $R0 → $L1 $R1 → sync ${SY:-none} $L2 $R2)" "reddish '${L0:-#000000}' && reddish '${R0:-#000000}' && yellowish '${L1:-#000000}' && reddish '${R1:-#000000}' && yellowish '${L2:-#000000}' && yellowish '${R2:-#000000}'"
quit

# 202. The hand tool (⇧H, PDF Expert's Pan & Zoom): its note shows, a plain drag moves the page (no text selected),
# ⌥ + drag up zooms in (Acrobat's Dynamic Zoom), Escape puts it away.
cp "$OUT/text.pdf" "$OUT/hand.pdf"; launch "$OUT/hand.pdf"; sleep 1
key shift+h; sleep 0.5; HT=$(ocr 1020x80+260+700)
P0=$(viewsig); xdotool mousemove 760 500 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove 760 $(( 500 - i * 30 )); sleep 0.05; done; xdotool mouseup 1; sleep 1; P1=$(viewsig)
xdotool keydown alt mousemove 760 500 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove 760 $(( 500 - i * 20 )); sleep 0.08; done; xdotool mouseup 1 keyup alt; sleep 1.5; P2=$(viewsig)
key Escape; sleep 0.5; HE=$(ocr 1020x80+260+700)
check "the hand tool drags and ⌥-drag zooms the pages, Escape ends it ($P0 → $P1 → $P2; $(echo "$HT" | cut -c1-30) / $(echo "$HE" | cut -c1-20))" "echo \"$HT\" | grep -q 'Hand tool' && ! echo \"$HE\" | grep -q 'Hand tool' && [ \"$P0\" != \"$P1\" ] && [ \"$P1\" != \"$P2\" ]"
quit

# 203. Split view, each pane its own (PDF Expert's): a press in the other pane makes it the active one and goes on
# to it (text dragged over there is selected and copied), and ⌘+ then zooms that pane only.
cp "$OUT/text.pdf" "$OUT/split2.pdf"; launch "$OUT/split2.pdf"; sleep 1
SI=$(layout_item side); sleep 1.5
read QX QY <<< "$(at quick 510x700+770+90)"
echo -n | xclip -selection clipboard >/dev/null 2>&1
[ -n "$QY" ] && { xdotool mousemove $(( QX - 30 )) $QY mousedown 1; for i in 1 2 3 4; do xdotool mousemove $(( QX + i * 20 )) $QY; sleep 0.05; done; xdotool mouseup 1; sleep 0.8; key ctrl+c; }
SC=$(xclip -o -selection clipboard 2>/dev/null | head -1)
cp "$OUT/across.pdf" "$OUT/split3.pdf"; quit; launch "$OUT/split3.pdf"; sleep 1
layout_item side >/dev/null; sleep 1.5
redshare() { import -window root -crop 500x1+$1+420 png:- 2>/dev/null | convert - -fx '(r>0.6&&g<0.4&&b<0.4)?1:0' -format '%[fx:mean]' info: 2>/dev/null; }
ZL0=$(redshare 262); ZR0=$(redshare 772)
click 1020 420 0.8; key ctrl+equal; key ctrl+equal; sleep 1.5
ZL1=$(redshare 262); ZR1=$(redshare 772)
check "split view: a drag in the other pane selects there, and each pane zooms on its own ($SI; quick at ${QX:-?}; copied '$(echo "$SC" | cut -c1-20)'; red $ZL0 $ZR0 → $ZL1 $ZR1)" "echo \"$SC\" | grep -q 'quick' && python3 -c 'import sys; l0, r0, l1, r1 = map(float, sys.argv[1:]); sys.exit(0 if abs(l0 - r0) < 0.02 and abs(l1 - l0) < 0.02 and r1 > l1 + 0.03 else 1)' ${ZL0:-0} ${ZR0:-1} ${ZL1:-0} ${ZR1:-0}"
quit

# 204. … ▸ Split with across.pdf (PDF Expert's split of two documents): text.pdf's tab shows across.pdf's red page
# in the other pane, named; a click there brings across.pdf's tab forward with text.pdf in its other pane, each
# half still showing what it showed.
cp "$OUT/text.pdf" "$OUT/split4.pdf"; launch "$OUT/split3.pdf" "$OUT/split4.pdf"; sleep 3
WI=$(layout_item with); sleep 1.5
# The other pane's name: a small chip (white on dark) 12 px into the pane's top-left corner, read as one line
# (the whole strip with its red page and buttons does not OCR; nor does the grey canvas beside the page: the chip
# alone, 80 × 26 px).
chip() { import -window root -crop "80x26+$(( $1 + 10 ))+92" png:- 2>/dev/null | convert - +repage -resize 300% png:- | tesseract stdin stdout --psm 7 2>/dev/null | tr '\n' ' '; }
D0=$(c8 1020); N0=$(chip 767)
click 1020 420 1.5
D1=$(c8 1020); L1=$(c8 510); N1=$(chip 250)
check "split with another document, then its tab forward with this one beside it ($WI; $D0 '$(echo "$N0" | cut -c1-20)' → $L1 $D1 '$(echo "$N1" | cut -c1-20)')" "reddish '${D0:-#000000}' && echo \"\$N0\" | grep -q 'split3' && reddish '${D1:-#000000}' && ! reddish '${L1:-#FF0000}' && echo \"\$N1\" | grep -q 'split4'"
quit

# 210. Edit ▸ one object picked ▸ Order ▸ To front (Acrobat's Arrange ▸ Bring to Front): a red square under a blue
# one (their overlap at 150–200 pt) is picked by a press on its own corner (fit width: 612 pt shown 1000 px wide
# from x 264, y 100) and brought to the front; saved, the overlap is red. The card's button is found by OCR.
python3 - "$OUT/order.pdf" <<'PY'
import sys
objs = ["<< /Type /Catalog /Pages 2 0 R >>", "<< /Type /Pages /Kids [4 0 R] /Count 1 >>"]
c = "1 0 0 rg 100 500 100 100 re f 0 0 1 rg 150 450 100 100 re f"
objs.append(f"<< /Length {len(c)} >>\nstream\n{c}\nendstream")
objs.append("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 3 0 R >>")
b = b"%PDF-1.7\n"; offs = []
for i, o in enumerate(objs, 1):
    offs.append(len(b)); b += f"{i} 0 obj\n{o}\nendobj\n".encode()
x = len(b)
b += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode() + b"".join(f"{n:010d} 00000 n \n".encode() for n in offs)
b += f"trailer\n<< /Size {len(objs)+1} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n".encode()
open(sys.argv[1], "wb").write(b)
PY
# The colour (r g b) at page point $2,$3 of $1 drawn at 72 dpi.
pixel() { pdftoppm -r 72 -f 1 -l 1 "$1" 2>/dev/null | python3 -c "
import sys
d = sys.stdin.buffer.read(); parts = d.split(maxsplit=4); w, h = int(parts[1]), int(parts[2]); px = parts[4]
x, y = $2, 792 - $3; i = (y * w + x) * 3; print(px[i], px[i + 1], px[i + 2])"; }
O0=$(pixel "$OUT/order.pdf" 175 525)
launch "$OUT/order.pdf"; key ctrl+9; sleep 1; key e; sleep 1.5
click $(( 320 + 120 * 944 / 612 )) $(( 134 + (792 - 580) * 944 / 612 )) 1; shot 210-picked
# The card in the view's top-right corner: "To front" is its first button (OCR may join its two words).
TF=$(at To 680x120+600+118); [ -n "$TF" ] || TF=$(at front 680x120+600+118); [ -n "$TF" ] && click $TF 1.5; shot 210-front; key ctrl+s; sleep 1.5
O1=$(pixel "$OUT/order.pdf" 175 525)
check "Order ▸ To front brings the picked square over the other (${TF:-no To front button}; overlap $O0 → $O1)" "[ \"$O0\" = '0 0 255' ] && [ \"$O1\" = '255 0 0' ]"
quit

# 211. Edit ▸ a paragraph open ▸ U and • (Acrobat's Underline and Bulleted list) apply to it, not only to new text:
# applied (⌘↩) and saved, the page draws a line under it and the paragraph starts with a bullet. U and • follow
# Mono in the row (U 129, • 183 px right of Mono, as step 2c's places).
# Lines (… l, then S) the file draws: counted before and after.
strokes() { qpdf --qdf --object-streams=disable "$1" - 2>/dev/null | grep -acE '[0-9.-]+ [0-9.-]+ l( S)?$'; }
cp "$OUT/text.pdf" "$OUT/underline.pdf"; UL0=$(strokes "$OUT/underline.pdf"); launch "$OUT/underline.pdf"; key ctrl+9; sleep 1
key e; sleep 1; QK=$(at quick 1020x700+255+90); [ -n "$QK" ] && click $QK 1.2
MX=$(at Mono 1280x40+0+90 | cut -d' ' -f1); MX=${MX:-409}
click $(( MX + 129 )) 110 0.4; click $(( MX + 183 )) 110 0.4; key ctrl+Return; sleep 1.5; key Escape; key v; key ctrl+s; sleep 1.5
UL=$(strokes "$OUT/underline.pdf")
UB=$(pdftotext -f 1 -l 1 "$OUT/underline.pdf" - 2>/dev/null | grep -c '•')
check "U and • underline and bullet the paragraph open (${QK:-no paragraph}; $UL0 → $UL lines, $UB bullets)" "[ \"$UL\" -gt \"$UL0\" ] && [ \"$UB\" -ge 1 ]"
quit

# 212. Edit ▸ a paragraph open ▸ More fonts… (Acrobat's font list): the computer's open fonts listed (DejaVu,
# Liberation… on this system); the first that can be embedded (those PdfCraft may not embed read "— not
# embeddable", dimmed) picked, applied with Done and saved, the page embeds it (a Type0 font).
cp "$OUT/text.pdf" "$OUT/morefonts.pdf"; launch "$OUT/morefonts.pdf"; key ctrl+9; sleep 1
key e; sleep 1; QK=$(at quick 1020x700+255+90); [ -n "$QK" ] && click $QK 1.2
# The middle of the list's first row not ending "not embeddable" (OCR lines of the list, under the button).
first_embeddable() { local x=$1 y=$2; import -window root -crop "260x300+$(( x - 20 ))+$(( y + 22 ))" png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null \
  | awk -F'\t' -v ox=$(( x - 20 )) -v oy=$(( y + 22 )) '$1 == 5 { k = $3 "/" $4 "/" $5; t[k] = t[k] " " $12; if (!(k in top)) { top[k] = $8; h[k] = $10; order[++n] = k } }
      END { for (i = 1; i <= n; i++) { k = order[i]; if (t[k] !~ /embeddable/ && t[k] ~ /[A-Za-z]/) { printf "%d %d\n", ox + 40, oy + (top[k] + h[k] / 2) / 2; exit } } }'; }
shot 212-open; MF=$(at More 1280x40+0+90); [ -n "$MF" ] && { click $MF 4; shot 212-list; FE=$(first_embeddable $MF); [ -n "$FE" ] && click $FE 1.5; shot 212-picked; }
DN=$(at Done 1020x700+255+90); if [ -n "$DN" ]; then click $DN 2; else key ctrl+Return; sleep 2; fi; key Escape; key v; key ctrl+s; sleep 1.5
FT=$(pdffonts -f 1 -l 1 "$OUT/morefonts.pdf" 2>/dev/null | tail -n +3 | awk '{print $1}' | tr '\n' ' ')
check "More fonts… sets the paragraph in a font of the computer's, embedded (${MF:-no More fonts button}; row ${FE:-none}; $FT)" "pdffonts -f 1 -l 1 \"$OUT/morefonts.pdf\" 2>/dev/null | grep -q 'CID TrueType\|Type 0\|CID Type 0C'"
quit

# 213. Shapes (R) ▸ Loupe and Mask (Preview's): an oval dragged with Loupe on becomes a loupe, a rectangle dragged
# with Mask on a mask; saved, the file holds both (PdfCraft's /PCLoupe and /PCMask comments).
cp "$OUT/text.pdf" "$OUT/loupe.pdf"; launch "$OUT/loupe.pdf"; sleep 1; key r; sleep 0.5
LP=$(at Loupe 1280x40+0+82); [ -n "$LP" ] && click $LP 0.5; drag 500 300 120 120
MK=$(at Mask 1280x40+0+82); [ -n "$MK" ] && click $MK 0.5; drag 450 500 200 100
key ctrl+s; sleep 1.5
QL=$(qpdf --qdf --object-streams=disable "$OUT/loupe.pdf" - 2>/dev/null)
check "Shapes ▸ Loupe and Mask add a loupe and a mask (${LP:-no Loupe button}; ${MK:-no Mask button})" "echo \"\$QL\" | grep -aq '/PCLoupe ' && echo \"\$QL\" | grep -aq '/PCMask '"
quit

# 214. Shapes (R) ▸ Article (Acrobat's Article tool): two boxes dragged make one article; the second box clicked is
# picked and Delete takes it alone; saved, the catalog's /Threads holds the article with one box (a bead).
cp "$OUT/text.pdf" "$OUT/article.pdf"; launch "$OUT/article.pdf"; sleep 1; key r; sleep 0.5
AR=$(at Article 1280x40+0+90); [ -n "$AR" ] && click $AR 0.5
drag 350 250 250 150; drag 650 250 250 150; click 760 320 0.8; key Delete; sleep 1; key ctrl+s; sleep 1.5
QA=$(qpdf --qdf --object-streams=disable "$OUT/article.pdf" - 2>/dev/null)
NB=$(echo "$QA" | grep -ac '/Type /Bead')
check "Shapes ▸ Article chains boxes into an article; a box picked is deleted alone (${AR:-no Article button}; $NB bead)" "echo \"\$QA\" | grep -aq '/Threads' && [ \"$NB\" = 1 ]"
quit

# 220. The menu bar (C-platform-014; Acrobat's and Preview's menus), shown on request (the steps above click where
# a window without it puts things): File to Help along the top; File lists Save with its shortcut; Window ▸ Home
# from a reader tab shows Home.
export SHEAF_TEST_MENU_BAR=1
cp "$OUT/text.pdf" "$OUT/menubar.pdf"; launch "$OUT/menubar.pdf"; sleep 1
MB=$(ocr 700x34+0+0)
FX=$(at File 400x34+0+0); [ -n "$FX" ] && click $FX 0.8
FM=$(ocr 420x420+0+24); key Escape; sleep 0.4
read WX _ <<< "$(at Window 700x34+0+0)"
[ -n "$WX" ] && click $WX 14 0.8
HM=$(at Home 320x160+$(( ${WX:-20} - 20 ))+24); [ -n "$HM" ] && click $HM 1.2
HR=$(at Recents 400x400+0+40)
check "the menu bar shows File to Help, File lists Save with Ctrl+S, Window ▸ Home goes Home ($MB; $(echo "$FM" | grep -o 'Ctrl+S' | head -1); ${HM:-no Home item}; ${HR:-no Recents})" "echo \"$MB\" | grep -q 'File' && echo \"$MB\" | grep -q 'Help' && echo \"$FM\" | grep -q 'Ctrl+S' && [ -n \"$HR\" ]"
quit
unset SHEAF_TEST_MENU_BAR

# Preferences scrolled until a word shows in the page (the platform sections are below Signatures).
# Once found, read again until the place holds: the scroll is still easing when the word first shows
# (rows came out 42 px low on 27/10).
pref_find() { local f="" g; for _ in $(seq 1 25); do f=$(at "$1" 1020x700+250+90); [ -n "$f" ] && break; xdotool mousemove 760 500 click 5 click 5 click 5; sleep 0.3; done; for _ in 1 2 3 4 5; do [ -n "$f" ] || break; sleep 0.4; g=$(at "$1" 1020x700+250+90); [ "$g" = "$f" ] && break; f=$g; done; echo "$f"; }
# SR keeps Reset all's place before the search: the rows found then start 45 px below the field,
# 42 px apart (row 1 at SR - 48, row 2 at SR - 6).
# Preferences ▸ Keyboard shortcuts' search field, found from the section's title ("shortcuts"; its "Keyboard" starts
# left of the OCR's crop): the field 40 px below it, then what is typed is searched. SR is 133 px below the title,
# the rows counted from it (Reset all's place moved when Preferences ▸ Interface gained its Language row). The
# wheel's scroll eases out after the word shows: measured again once still.
shortcut_search() { local t f; read _ t <<< "$(pref_find shortcuts)"; sleep 1; read _ f <<< "$(at shortcuts 1020x700+250+90)"; t=${f:-$t}
    SR=0; [ -n "$t" ] && click 500 $(( t + 40 )) 0.5; xdotool type --delay 20 "$1"; sleep 0.8
    # The list found is below the field: the page scrolled on a little (where it can) so its rows show.
    xdotool mousemove 760 500 click 5 click 5; sleep 0.8
    read _ f <<< "$(at shortcuts 1020x700+250+90)"; t=${f:-$t}; [ -n "$t" ] && SR=$(( t + 133 )); }
# A row of the shortcuts list found by a word of its name: its keys box (right-aligned, x 1157) clicked.
# Starting to record can scroll the page (near its end): when "Press the keys…" does not show, the row is found
# again and clicked once more.
keybox() { local y; read _ y <<< "$(at "$1" 1020x700+250+90)"; [ -n "$y" ] && click 1157 "$y" 0.5
    if [ -n "$y" ] && [ -z "$(at Press 1020x700+250+90)" ]; then read _ y <<< "$(at "$1" 1020x700+250+90)"; [ -n "$y" ] && click 1157 "$y" 0.5; fi
    echo "${y:-no $1 row}"; }
# The preferences file, written even when no earlier step made it.
prefs_set() { python3 -c 'import json, os, sys; f = sys.argv[1]; p = json.load(open(f)) if os.path.exists(f) else {}; exec(sys.argv[2]); os.makedirs(os.path.dirname(f), exist_ok=True); json.dump(p, open(f, "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" "$1"; }
# The steps below change the platform preferences: put back as they were.
platform_reset() { python3 -c 'import json, sys; p = json.load(open(sys.argv[1])); p.pop("platform", None); json.dump(p, open(sys.argv[1], "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null; }
# Saved actions left by the batch steps (35x) put "Custom tools" in the reader's toolbar and on the Tools page,
# which moves every later click there: the steps that click those bars start without any (28/10).
actions_reset() { rm -f "$PWD/$OUT/profile/data/Sheaf/actions.json"; }

# 221. Preferences ▸ Keyboard shortcuts (C-preferences-013, Stirling's Hotkeys): "find" searched, Find's Ctrl+F
# clicked and Ctrl+Alt+F pressed: kept in the preferences; Find Next given Ctrl+S is refused (Save has it).
launch; key ctrl+comma; sleep 0.8
shortcut_search "find"
# The rows' places come from the search field's (OCR found "Find…" and "Next" a row off while the
# list was still laying out): Find… first, Find Next second.
KF=$(( SR - 48 )); KG=$(( SR - 6 ))
# Recording starts on the box's click; under load a click can land before the list has settled, so the keys
# are pressed again (box clicked anew, from Find…'s row read on screen) until the preferences hold them.
kp() { python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("platform", {}).get("shortcuts"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null; }
for _ in 1 2 3; do
    [ "$SR" -gt 48 ] && click 1157 $KF 0.5; sleep 0.5; key ctrl+alt+f; sleep 1.5
    kp | grep -q "'find'" && break
    key Escape; sleep 0.5; read _ FY <<< "$(at Next 1020x700+250+90)"; case "$FY" in ''|*[!0-9]*) ;; *) KG=$FY; KF=$(( FY - 42 ));; esac
done
[ "$KF" -gt 0 ] && click 1157 $KG 0.5; key ctrl+s; sleep 1.2; shot 221-shortcuts
# Quotes and other shell characters are taken out of the read text: the grammar line above the
# shortcuts quotes "a" and "an", which broke the check below.
KE=$(ocr 1020x700+250+90 | tr -d '"`$\\')
# The preferences are written a moment after the keys (up to 3 s under load).
for _ in 1 2 3 4 5 6; do
    KP=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("platform", {}).get("shortcuts"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
    echo "$KP" | grep -q "'find'" && break; sleep 0.5
done
check "a shortcut recorded in Preferences is kept, one already used is refused (rows at $KF, $KG; $KP; $(echo "$KE" | grep -o 'already [A-Za-z ▸]*' | head -1))" "echo \"$KP\" | grep -q \"'find', 'Control+Alt+F'\" && ! echo \"$KP\" | grep -q 'find-next' && echo \"$KE\" | grep -q 'already'"
quit; platform_reset

# 222. Check spelling while typing (C-edit-013, C-reader-060; Acrobat's and Preview's): a note typed with two
# misspelt words shows them under the card with corrections, Ignore and Learn; the correction clicked goes in
# the note (saved). Needs a Hunspell English dictionary (hunspell-en-us) or /usr/share/dict/words.
cp "$OUT/text.pdf" "$OUT/spelling.pdf"; launch "$OUT/spelling.pdf"; sleep 1
key n; click 640 400 0.8; xdotool type --delay 15 "A mispelt sentnce here "; sleep 1.5
# The card comes once the check has run (the first check also loads the grammar rules): waited for.
for _ in $(seq 1 12); do SP=$(ocr 1280x710+0+90); echo "$SP" | grep -q 'Learn' && break; sleep 0.5; done
SC=$(at sentence 1280x710+0+90); [ -n "$SC" ] && click $SC 0.8
key ctrl+Return; sleep 0.6; key ctrl+s; sleep 1.5
SN=$(qpdf --qdf --object-streams=disable "$OUT/spelling.pdf" - 2>/dev/null | grep -ao '/Contents ([^)]*)' | tail -1)
check "misspelt words show under the note with corrections, and one clicked goes in the note (${SC:-no sentence chip}; $SN)" "echo \"$SP\" | grep -q 'Ignore' && echo \"$SP\" | grep -q 'Learn' && echo \"$SN\" | grep -q 'sentence here'"
quit

# 223. Preferences ▸ Interface ▸ Right to left (C-platform-004, iLovePDF's right-to-left interface): Home's
# sidebar moves to the window's right edge, back on the left with Left to right.
launch; key ctrl+comma; sleep 0.8
RL=$(pref_find Right); [ -n "$RL" ] && click $RL 0.8
read RX _ <<< "$(at Recents 1280x300+0+40)"
LR=$(at Left 1280x710+0+90); [ -n "$LR" ] && click $LR 0.8
read LX _ <<< "$(at Recents 1280x300+0+40)"
check "right to left puts Home's sidebar on the right, left to right back on the left (Recents at x ${RX:-?}, then ${LX:-?})" "[ \"${RX:-0}\" -gt 1050 ] && [ \"${LX:-9999}\" -lt 250 ]"
quit; platform_reset

# 224. A document right to left (C-platform-004): the tab bar starts on the right (Home at the right edge, the
# document's tab to its left), and the side panel goes to the right of the pages: the page's words move left by
# about the panel's width from where they were left to right.
cp "$OUT/text.pdf" "$OUT/mirrored.pdf"; launch "$OUT/mirrored.pdf"; sleep 1
read QL _ <<< "$(at quick 1280x700+0+90)"; quit
prefs_set 'p.setdefault("platform", {})["direction"] = 2'
launch "$OUT/mirrored.pdf"; sleep 1
read HX _ <<< "$(at Home 1280x40+0+0)"; read DX _ <<< "$(at mirrored 1280x40+0+0)"; read QR _ <<< "$(at quick 1280x700+0+90)"
check "right to left mirrors the tab bar and puts the side panel right of the pages (Home at x ${HX:-?}, the document at ${DX:-?}; 'quick' at ${QL:-?} then ${QR:-?})" "[ \"${HX:-0}\" -gt 1100 ] && [ -n \"$DX\" ] && [ \"$DX\" -lt \"$HX\" ] && [ -n \"$QL\" ] && [ \"${QR:-9999}\" -lt \"$(( QL - 150 ))\" ]"
quit; platform_reset

# 225. A reader tool's letter changed (C-preferences-013): Preferences ▸ Keyboard shortcuts, "highlight" searched,
# its H clicked and Q pressed; in a document Q then picks Highlight (a drag over the words highlights them, saved).
launch; key ctrl+comma; sleep 0.8
shortcut_search "highlight"
# The key's box holds "H" alone (at() matches the start of a word, and "Highlight" starts with H).
at_word() { import -window root -crop 1020x700+250+90 png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' -v w="$1" '$12 == w { printf "%d %d\n", 250 + ($7 + $9 / 2) / 2, 90 + ($8 + $10 / 2) / 2; exit }'; }
# As in 221: the box's click can land before the list settles under load, so the letter is given again
# until the preferences hold it.
tk() { python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("platform", {}).get("tool_keys"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null; }
for _ in 1 2 3; do KH=$(keybox Highlight); sleep 0.8; key q; sleep 1.5; tk | grep -q "'highlight'" && break; key Escape; sleep 0.5; done
TK=$(tk)
quit
cp "$OUT/text.pdf" "$OUT/toolkey.pdf"; launch "$OUT/toolkey.pdf"; sleep 1
key q; read QX QY <<< "$(at quick 1020x700+255+90)"; [ -n "$QY" ] && { xdotool mousemove $(( QX - 30 )) $QY mousedown 1 mousemove $(( QX + 60 )) $QY mouseup 1; sleep 1; }; key v; key ctrl+s; sleep 1.5
check "a tool's letter changed in Preferences picks it on the pages (row at $KH; $TK)" "echo \"$TK\" | grep -q \"'highlight', 'q'\" && qpdf --qdf --object-streams=disable \"$OUT/toolkey.pdf\" - 2>/dev/null | grep -aq '/Subtype /Highlight'"
quit; platform_reset

# 226. Edit ▸ Copy, Select All and Paste from the menu bar (C-platform-014): on the reader's pages, Select All then
# Copy puts the page's text on the clipboard; in a text field (Preferences ▸ Identity ▸ Your name, "Ada"), Select
# All, Copy, then Paste at its end makes "AdaAda" (kept as the author's name).
menu() { local m; read m _ <<< "$(at "$1" 700x34+0+0)"; [ -n "$m" ] && click $m 14 0.8; local it; it=$(at "$2" 320x520+$(( ${m:-20} - 20 ))+24); [ -n "$it" ] && click $it 0.8; echo "${it:-no $2}"; }
export SHEAF_TEST_MENU_BAR=1
cp "$OUT/text.pdf" "$OUT/editmenu.pdf"; launch "$OUT/editmenu.pdf"; sleep 1
echo -n | xclip -selection clipboard >/dev/null 2>&1
E1=$(menu Edit Select); E2=$(menu Edit Copy); sleep 0.4
CP=$(xclip -o -selection clipboard 2>/dev/null | head -1)
key ctrl+comma; sleep 0.8
# Identity's field, 32 px under its label "Your name" ("Your" is read with the label's bold); emptied first.
read _ YY <<< "$(pref_find name)"; [ -n "$YY" ] && click 500 $(( YY + 32 )) 0.5; key ctrl+a; key BackSpace; xdotool type --delay 20 "Ada"; sleep 0.3
E3=$(menu Edit Select); E4=$(menu Edit Copy); key End; E5=$(menu Edit Paste); sleep 0.6
AU=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("author"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
check "Edit ▸ Select All, Copy, Paste act on the pages and on a text field ($E1 $E2 $E5; clipboard '$(echo "$CP" | cut -c1-20)'; author $AU)" "echo \"$CP\" | grep -q 'quick brown fox' && [ \"$AU\" = AdaAda ]"
quit
unset SHEAF_TEST_MENU_BAR
python3 -c 'import json, sys; p = json.load(open(sys.argv[1])); p["author"] = None; json.dump(p, open(sys.argv[1], "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json"

# 227. Edit ▸ Check Spelling… (F7; C-edit-013, Acrobat's Check Spelling in comments and form fields): a note "teh cat
# sat teh" left on the page; the pass shows "teh" with its corrections, "the" clicked and Change All: both go, the
# pass ends (nothing else misspelt) and the note, saved, reads "the cat sat the".
cp "$OUT/text.pdf" "$OUT/checkpass.pdf"; launch "$OUT/checkpass.pdf"; sleep 1
key n; click 640 400 0.8; xdotool type --delay 15 "teh cat sat teh"; key ctrl+Return; sleep 0.8; key v
key F7; sleep 1.2
# The texts are scanned on a worker: the first word may take a moment to show (its red note and the
# underlined word do not OCR; the comment it is in and Change to do).
for _ in 1 2 3 4 5 6; do CW=$(ocr 1020x700+250+90); echo "$CW" | grep -q 'Change' && break; sleep 0.5; done; shot 227-dialog
TH=$(at_word the); [ -n "$TH" ] && click $TH 0.4
# Change All, the button before Change: 96 px right of Ignore (its 13 px "All" OCRs glued, "Changeall").
CH=$(at Ignore 1020x700+250+90); CH=${CH:+$(set -- $CH; echo $(( $1 + 96 )) $2)}; [ -n "$CH" ] && click $CH 1.2; shot 227-after-change
CD=$(ocr 1020x700+250+90)
DN=$(at Done 1020x700+250+90); [ -n "$DN" ] && click $DN 0.6
key ctrl+s; sleep 1.5
CN=$(qpdf --qdf --object-streams=disable "$OUT/checkpass.pdf" - 2>/dev/null | grep -ao '/Contents ([^)]*)' | tail -1)
check "Check Spelling goes to a misspelt word in a comment and Change corrects it (${TH:-no 'the'}; ${CH:-no Change}; $(echo "$CW" | grep -o 'Comment on page [0-9]*' | head -1); $(echo "$CD" | grep -o 'check complete' | head -1); $CN)" "echo \"$CW\" | grep -q 'Comment on page 1' && echo \"$CW\" | grep -q 'Change to' && echo \"$CD\" | grep -q 'complete' && echo \"$CN\" | grep -q 'the cat sat the'"
quit

# 240. Kept signatures and initials belong to authenticated local storage, never prefs.json.
# The existing signature/initials steps exercised save, restart and reuse. Their helpers read
# the encrypted local store too; smoke inspection needs Python cryptography.
no_plain_marks() {
    python3 - "$PWD/$OUT/profile/data/Sheaf/prefs.json" <<'PYMARKS'
import json, sys
p = json.load(open(sys.argv[1]))
assert not any(k in p for k in ("signature_name", "signature_strokes", "signatures", "initials"))
PYMARKS
}
has_secure_marks() { secure_marks | python3 -c 'import json,sys; p=json.load(sys.stdin); assert p.get("signatures") or p.get("initials")'; }
# A signature kept in this part (the shard starts with none): S, a click, drawn on the pad, Use (as step 2b).
cp "$OUT/text.pdf" "$OUT/marks.pdf"; launch "$OUT/marks.pdf"; sleep 1
key Escape; key s; click 700 500 1
xdotool mousemove 520 420 mousedown 1; for i in $(seq 0 20); do xdotool mousemove $((520 + i * 12)) $((420 + (i % 3) * 8)); done; xdotool mouseup 1; sleep 0.3
click 818 555 1.2; key ctrl+s; sleep 1.5
quit
check "signatures and initials are absent from plaintext preferences" "no_plain_marks"
check "kept marks exist in the system credential store" "has_secure_marks"
# 250. Acrobat's JavaScript Console: Ctrl+J opens it for the active document. Run's output must
# appear in the output pane, not only in the source editor. No document mutation for this check.
cp "$OUT/text.pdf" "$OUT/js-console.pdf"; launch "$OUT/js-console.pdf"; key ctrl+j
JS_SOURCE_X=500; JS_SOURCE_Y=230
click "$JS_SOURCE_X" "$JS_SOURCE_Y"; key ctrl+a
xdotool type --clearmodifiers --delay 4 "console.println('JSCONSOLEPASS');"
# Run (white on red) does not OCR, "Run selection" does: Run is 203 px left of Scripts.
JR=$(at Scripts 700x500+290+100); JR=${JR:+$(set -- $JR; echo $(( $1 - 203 )) $2)}; [ -n "$JR" ] && click $JR 2
JO=$(ocr 680x200+300+425)
check "JavaScript console runs against the active PDF and shows its output (${JR:-no Run})" "echo \"$JO\" | grep -q JSCONSOLEPASS"
quit

# 251. Suggested actions export through the same serializer import accepts (no handwritten JSON
# fixture). Importing the exported suggestion creates a distinct saved action.
launch
AC=$(at Actions 210x400+0+30); [ -n "$AC" ] && click $AC
CA=$(at Confidential 230x600+235+150); [ -n "$CA" ] && click $CA
answer "$PWD/$OUT/action-export.json"
EX=$(at Export 1000x260+220+35); [ -n "$EX" ] && click $EX
IM=$(at Import 1000x260+220+35); [ -n "$IM" ] && click $IM
check "suggested action exports and imports as an independent saved action (${CA:-no suggestion})" "python3 -c 'import json; a=json.load(open(\"$OUT/action-export.json\")); b=json.load(open(\"$OUT/profile/data/Sheaf/actions.json\")); assert a[\"steps\"][0][\"tool\"] == \"add_watermark\"; assert a[\"steps\"][0][\"argument\"] == \"CONFIDENTIAL\"; assert any(x[\"name\"] != a[\"name\"] and x[\"steps\"] == a[\"steps\"] for x in b)'"
quit

# 252. Stirling's local file library: Add copies the input, then Save a copy exports the stored
# version. The library must own different bytes on disk, not only remember the source path.
launch
LI=$(at Library 210x400+0+30); [ -n "$LI" ] && click $LI
answer "$PWD/$OUT/text.pdf"
# "Add files…" is the primary button, white on the accent.
LA=$(at_on_accent Add 1000x160+220+35); [ -n "$LA" ] && click $LA 2
answer "$PWD/$OUT/library-export.pdf"
LE=$(at Save 600x300+650+100); [ -n "$LE" ] && click $LE
check "Library keeps a private copy and exports it (${LA:-no Add}; ${LE:-no Save copy})" "cmp -s \"$OUT/text.pdf\" \"$OUT/library-export.pdf\" && python3 -c 'import pathlib,json; root=pathlib.Path(\"$OUT/profile/data/Sheaf/library\"); items=[json.loads(p.read_text()) for p in root.glob(\"*/entry.json\")]; assert any(e[\"name\"] == \"text.pdf\" and len(e[\"versions\"]) == 1 for e in items)'"
quit

# 253. Acrobat's Create PDF Portfolio: both native dialogs may use the same debug answer. The
# original PDF stays intact, the unique output gets a Collection and its original embedded file.
cp "$OUT/text.pdf" "$OUT/portfolio-source.pdf"
launch
AC=$(at Actions 210x400+0+30); [ -n "$AC" ] && click $AC 1
answer "$PWD/$OUT/portfolio-source.pdf"
PC=$(at Create 1050x260+220+35); [ -n "$PC" ] && click $PC 1
# The dialog's bottom row (OCR reads its buttons on that strip only).
PA=$(at Add 700x44+295+504); [ -n "$PA" ] && click $PA 1.5
# Save portfolio… (white on red) does not OCR: 149 px right of Add files…'s "Add".
PW=${PA:+$(set -- $PA; echo $(( $1 + 149 )) $2)}; [ -n "$PW" ] && click $PW 3
check "Create portfolio bundles the original instead of merging (${PC:-no Create}; ${PA:-no Add}; ${PW:-no Save})" "qpdf --qdf --object-streams=disable \"$OUT/portfolio-source (1).pdf\" - 2>/dev/null | grep -aq '/Collection' && cmp -s \"$OUT/text.pdf\" \"$OUT/portfolio-source.pdf\""
quit
check "kept marks exist in authenticated local storage" "has_secure_marks"

# 205. Split view's Sync scroll and zoom (PDF Expert's): with it on, ⌘= in the first pane zooms the other too; the
# divider dragged 150 px left moves the end of the first pane's red row (the first pixel that is not red) with it.
cp "$OUT/across.pdf" "$OUT/split5.pdf"; launch "$OUT/split5.pdf"; sleep 1
layout_item side >/dev/null; sleep 1.5
SY=$(at Sync 400x44+880+84); [ -n "$SY" ] && click $SY 0.8
click 510 420 0.5; ZA0=$(redshare 262); ZB0=$(redshare 772)
key ctrl+equal; key ctrl+equal; sleep 1.5; ZA1=$(redshare 262); ZB1=$(redshare 772)
redend() { import -window root -crop 980x1+300+420 -depth 8 txt:- 2>/dev/null | python3 -c '
import re, sys
for line in sys.stdin:
    m = re.match(r"(\d+),0:.*#([0-9A-Fa-f]{6})", line)
    r, g, b = (int(m[2][k:k + 2], 16) for k in (0, 2, 4)) if m else (0, 0, 0)
    if m and not (r > 150 and g < 100 and b < 100):
        print(int(m[1])); break'; }
# The divider is that first pixel that is not red (its 1 px line, the grip 3 px either side): pressed there.
E0=$(redend); DV=$(( 300 + ${E0:-468} ))
xdotool mousemove $DV 500 mousedown 1; for i in 1 2 3 4 5; do xdotool mousemove $(( DV - i * 30 )) 500; sleep 0.05; done; xdotool mouseup 1; sleep 1.5
# The pointer off the divider: hovered, its line takes the accent red and the red row would not end there.
xdotool mousemove 1000 650; sleep 0.5; E1=$(redend)
check "Sync scroll and zoom zooms both panes; the divider moves (${SY:-no button}; $ZA0 $ZB0 → $ZA1 $ZB1; red row ends at ${E0:-?} → ${E1:-?})" "python3 -c 'import sys; a0, b0, a1, b1, e0, e1 = map(float, sys.argv[1:]); sys.exit(0 if a1 > a0 + 0.03 and b1 > b0 + 0.03 and e1 < e0 - 100 else 1)' ${ZA0:-1} ${ZB0:-1} ${ZA1:-0} ${ZB1:-0} ${E0:-0} ${E1:-9999}"
quit

# 206. … ▸ Split with another document, the other pane's text dragged over (PDF Expert's): the drag goes on to that
# document, whose tab comes forward; ⌘C then copies the words selected there.
cp "$OUT/text.pdf" "$OUT/split6t.pdf"; cp "$OUT/across.pdf" "$OUT/split6a.pdf"; launch "$OUT/split6t.pdf" "$OUT/split6a.pdf"; sleep 3
layout_item with >/dev/null; sleep 1.5
read QX QY <<< "$(at quick 510x700+770+90)"
echo -n | xclip -selection clipboard >/dev/null 2>&1
[ -n "$QY" ] && { xdotool mousemove $(( QX - 30 )) $QY mousedown 1; for i in 1 2 3 4; do xdotool mousemove $(( QX + i * 20 )) $QY; sleep 0.05; done; xdotool mouseup 1; sleep 0.8; key ctrl+c; }
SC6=$(xclip -o -selection clipboard 2>/dev/null | head -1)
check "a drag in another document's pane selects there (quick at ${QX:-?} ${QY:-?}; copied '$(echo "$SC6" | cut -c1-20)')" "echo \"$SC6\" | grep -q 'quick'"
quit

# 207. … ▸ Split with another document, ⌘ + wheel over the other pane: that document comes forward and zooms around
# the pointer (its word "quick" grows), as over the active pane.
launch "$OUT/split6t.pdf" "$OUT/split6a.pdf"; sleep 3
layout_item with >/dev/null; sleep 1.5
read QA QB QY <<< "$(span quick 510x700+770+90)"
[ -n "$QY" ] && { xdotool mousemove $QA $QY keydown ctrl; for i in 1 2 3; do xdotool click 4; sleep 0.1; done; xdotool keyup ctrl; sleep 1.5; }
read RA RB RY <<< "$(span quick 510x700+770+90)"
check "⌘ + wheel over another document's pane zooms that document (quick ${QA:-?}–${QB:-?} → ${RA:-?}–${RB:-?})" "[ -n \"$QB\" ] && [ -n \"$RB\" ] && [ \$(( RB - RA )) -gt \$(( (QB - QA) * 11 / 10 )) ]"
quit

# 207b. … ▸ Split with another document, a double-click on a word of the other pane (C-reader-045): its tab comes
# forward at the first click's release, and the second makes it a double-click there: the word alone is selected,
# ⌘C copies "quick" (the tab change split the pair; the second was a plain click).
launch "$OUT/split6t.pdf" "$OUT/split6a.pdf"; sleep 3
layout_item with >/dev/null; sleep 1.5
read QX QY <<< "$(at quick 510x700+770+90)"
echo -n | xclip -selection clipboard >/dev/null 2>&1
[ -n "$QY" ] && { xdotool mousemove $QX $QY click --repeat 2 --delay 120 1; sleep 0.8; key ctrl+c; }
SC7=$(xclip -o -selection clipboard 2>/dev/null | head -1)
shot 207b-double-click
check "a double-click on another document's pane selects the word there (quick at ${QX:-?} ${QY:-?}; copied '$SC7')" "echo \"$SC7\" | grep -qx ' *quick *'"
quit
# 228. Edit ▸ Undo from the menu bar (C-platform-014) acts on what has the keyboard: in Preferences ▸ Your name,
# "Ada" typed then Edit ▸ Undo leaves the field empty (the menu is enabled there); on a document's pages, a note
# added then Edit ▸ Undo takes the note away (saved: no note). The menu shows Undo's Ctrl+Z.
export SHEAF_TEST_MENU_BAR=1
launch; key ctrl+comma; sleep 0.8
YN=$(pref_find Your); read YX YY <<< "$YN"; [ -n "$YY" ] && click $(( YX + 40 )) $(( YY + 30 )) 0.5; xdotool type --delay 20 "Ada"; sleep 0.4
read MX _ <<< "$(at Edit 700x34+0+0)"; [ -n "$MX" ] && click $MX 14 0.8
UM=$(ocr 420x520+$(( ${MX:-20} - 20 ))+24); key Escape; sleep 0.3
# The field again has the keyboard (Escape closed the menu), then Edit ▸ Undo.
[ -n "$YY" ] && click $(( YX + 40 )) $(( YY + 30 )) 0.3
U1=$(menu Edit Undo); sleep 0.8
AU=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("author") or "")' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
quit
cp "$OUT/text.pdf" "$OUT/undomenu.pdf"; launch "$OUT/undomenu.pdf"; sleep 1
key n; click 640 420 0.8; xdotool type --delay 10 "undo me"; key ctrl+Return; sleep 0.8; key v
U2=$(menu Edit Undo); sleep 0.8; key ctrl+s; sleep 1.5
UN=$(qpdf --qdf --object-streams=disable "$OUT/undomenu.pdf" - 2>/dev/null | grep -ac '/Subtype /Text')
check "Edit ▸ Undo undoes the focused field's typing and, on the pages, the PDF's last edit ($U1 $U2; author '$AU'; notes $UN; menu: $(echo "$UM" | grep -o 'Undo Ctrl+Z' | head -1))" "[ -z \"$AU\" ] && [ \"$UN\" = 0 ] && echo \"$UM\" | grep -q 'Ctrl+Z'"
quit
unset SHEAF_TEST_MENU_BAR

# 229. A Tools page tool given keys that open it (C-preferences-013, Stirling's tool hotkeys): Preferences ▸ Keyboard
# shortcuts, "merge" searched, Merge PDF's None clicked and Ctrl+Alt+M pressed; on Home, Ctrl+Alt+M opens Merge PDF.
launch; key ctrl+comma; sleep 0.8
shortcut_search "merge pdf"
KM=$(keybox Merge); key ctrl+alt+m; sleep 0.8
LK=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("platform", {}).get("launch_keys"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
key Escape; key ctrl+alt+m; sleep 1.2
MT=$(ocr 1280x80+0+0)
check "keys given to a Tools page tool open it (row at $KM; $LK; tab bar: $(echo "$MT" | cut -c1-60))" "echo \"$LK\" | grep -q \"'merge', 'Control+Alt+M'\" && echo \"$MT\" | grep -q 'Merge'"
quit; platform_reset

# 230. Right to left beyond Home and the reader's bars (C-platform-004): Preferences' rows read from the right
# (Rulers measure in: Centimetres left of Inches), and the reader's tool options bar sits at the top right of the
# pages with its options reversed (Shapes: Fill on the right of the bar's left half, the bar past the middle).
prefs_set 'p.setdefault("platform", {})["direction"] = 2'
launch; key ctrl+comma; sleep 0.8
CM=$(pref_find Centimetres); read CX _ <<< "$CM"; read IX _ <<< "$(at Inches 1280x710+0+90)"
quit
cp "$OUT/text.pdf" "$OUT/rtlbar.pdf"; launch "$OUT/rtlbar.pdf"; sleep 1
key r; sleep 0.6; read FX _ <<< "$(at Fill 1280x120+0+80)"
check "right to left mirrors Preferences' rows and the tool options bar (Centimetres x ${CX:-?} < Inches ${IX:-?}; Fill at x ${FX:-?})" "[ -n \"$CX\" ] && [ -n \"$IX\" ] && [ \"$CX\" -lt \"$IX\" ] && [ \"${FX:-0}\" -gt 640 ]"
quit; platform_reset

# 310. A text file opened with Sheaf (C-reader-064, Stirling's non-PDF viewer): it opens in the reader to browse, its
# lines on the page, as a preview PDF in Sheaf's temporary folder (the .txt itself unchanged), not refused; a note at the
# top of the pages says it is a preview of notes.txt (C-reader-064, Sol's gap: no preview banner).
printf 'Shopping list\nMilk and bread\n' > "$OUT/notes.txt"
launch "$OUT/notes.txt"; sleep 3
TB=$(ocr 1280x80+0+0); PG=$(ocr 900x500+190+90); PB=$(import -window root -crop 700x40+400+86 png:- 2>/dev/null | convert - -negate -resize 200% png:- | tesseract stdin stdout --psm 7 2>/dev/null | tr "\n" " ")
shot 310-text-preview
check "a preview says so on the page ($(echo "$PB" | grep -o 'Preview of[^:]*' | head -1))" "echo \"$PB\" | grep -q 'Preview of notes.txt'"
check "a .txt opens in the reader (tab: $(echo "$TB" | grep -o 'notes[^ ]*' | head -1); page: $(echo "$PG" | grep -o 'Shopping list' | head -1))" "echo \"$PG\" | grep -q 'Shopping list' && echo \"$PG\" | grep -q 'Milk' && ls \"$OUT\"/profile/tmp/sheaf-previews/*/notes.txt.pdf >/dev/null 2>&1 && [ \"\$(cat \"$OUT/notes.txt\")\" = \"\$(printf 'Shopping list\nMilk and bread')\" ]"
quit

# 311. Search ▸ In a folder… (C-reader-037, Acrobat's Advanced Search ▸ All PDF documents in a folder) reads every
# PDF of the folder: "quick" is found in the copy of text.pdf that sorts after 600 damaged PDFs and one locked by a
# password; the sheet counts only the PDF read, and lists the 601 not searched with why, as Acrobat lists the
# documents it skipped (the locked one, 0-locked.pdf, first).
mkdir -p "$OUT/many"; for i in $(seq -w 1 600); do printf '%%PDF-1.4 empty\n' > "$OUT/many/a$i.pdf"; done
cp "$OUT/text.pdf" "$OUT/many/z-last.pdf"; qpdf --encrypt pw pw 256 -- "$OUT/text.pdf" "$OUT/many/0-locked.pdf"
launch "$OUT/text.pdf"
key ctrl+f; xdotool type --delay 20 "quick"; key Return; sleep 1.5
answer "$PWD/$OUT/many"
FS=$(at folder 300x30+0+160); [ -n "$FS" ] && click $FS 4
FT=$(ocr 800x560+240+100 | tr '\n' ' ' | tr -s ' ')
shot 311-folder-search
check "In a folder… counts the PDF read and lists the 601 not searched with why (link at ${FS:-?}; sheet: $(echo "$FT" | cut -c1-220))" "echo \"$FT\" | grep -q 'found in 1 PDF\. 601 PDFs could not be searched' && echo \"$FT\" | grep -q 'z-last' && echo \"$FT\" | grep -q '0-locked.pdf Protected by a password' && echo \"$FT\" | grep -q 'Damaged or not a PDF'"
# The app quit and the dialog answer cleared: left running, its window took the next steps' keys (round 30's
# 320–363 failed so), and later file dialogs all answered this folder.
answer ""; quit
# 320. More ▸ More tools ▸ Encrypt with certificates… (C-security-007, Acrobat's Encrypt with Certificate): my digital
# ID added with its password (full rights) and a colleague's certificate (read only by default), saved as an
# encrypted copy (Adobe.PubSec, one envelope per permission set). Opened again, the copy asks for the digital ID it
# was sent to: a wrong password is refused, the right one opens it in place (its tab named after it, its words shown).
# My ID is an ECC one (P-256, key agreement: Acrobat accepts them), the colleague's RSA: the copy holds both kinds of
# CMS recipient, and opening it in place goes through EC key agreement.
CD="$PWD/$OUT/certs"; mkdir -p "$CD"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout "$CD/me.key" -out "$CD/me.pem" -days 2 -subj "/CN=Smoke me" -addext keyUsage=digitalSignature,keyAgreement >/dev/null 2>&1
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$CD/colleague.key" -out "$CD/colleague.pem" -days 2 -subj "/CN=Smoke colleague" -addext keyUsage=digitalSignature,keyEncipherment >/dev/null 2>&1
openssl pkcs12 -export -passout pass:smokepw -inkey "$CD/me.key" -in "$CD/me.pem" -out "$CD/me.p12" >/dev/null 2>&1
cp "$OUT/text.pdf" "$OUT/certsrc.pdf"; rm -f "$OUT/certenc.pdf"; launch "$OUT/certsrc.pdf"; sleep 1
click 1210 61 0.8; click 1008 659 0.8
for i in 1 2 3 4 5 6; do xdotool mousemove 1000 500 click 5; sleep 0.1; done; sleep 0.5
EC=$(at Encrypt 300x700+880+80); [ -n "$EC" ] && click $EC 1.2
# The sheet is 760 px wide in the middle of the window, as tall as its content: its first row 81 px under its
# title (the password field at x 705, Add my digital ID… 507, Add certificate… 348); Save encrypted copy… 131 px
# right of Close, found once the recipients are listed.
read _ ETY <<< "$(at Encrypt 760x740+260+60)"; ETY=${ETY:-215}
click 705 $(( ETY + 81 )) 0.4; xdotool type --delay 20 smokepw
answer "$CD/me.p12"; click 507 $(( ETY + 81 )) 1.5
read _ ETY <<< "$(at Encrypt 760x740+260+60)"; ETY=${ETY:-215}
answer "$CD/colleague.pem"; click 348 $(( ETY + 81 )) 1
read _ ETY <<< "$(at Encrypt 760x740+260+60)"; ETY=${ETY:-215}
shot 320-recipients
RL=$(ocr 760x420+260+$(( ETY - 8 )))
read CLX CLY <<< "$(at Close 300x560+700+240)"   # (the whole sheet stops being read above its last rows)
answer "$PWD/$OUT/certenc.pdf"; [ -n "$CLX" ] && click $(( CLX + 131 )) $CLY 4; answer ""
quit
ENV=$(python3 -c 'import re, sys; b = open(sys.argv[1], "rb").read(); m = re.search(rb"/Recipients\s*\[(.*?)\]", b, re.S); print(len(re.findall(rb"<", m.group(1))) if m and b"Adobe.PubSec" in b else 0)' "$OUT/certenc.pdf" 2>/dev/null)
launch "$OUT/certenc.pdf"; sleep 2
xdotool type --delay 20 wrongpw; key Return; sleep 3
shot 320-wrong; WR=$(ocr 380x40+455+432)
key ctrl+a; xdotool type --delay 20 smokepw; key Return; sleep 4
shot 320-opened
TB=$(ocr 1280x80+0+0); QK=$(at quick 1020x700+250+90)
check "Encrypt with certificates writes a copy for two recipients that opens in place with the digital ID (recipients: $(echo "$RL" | grep -o 'Smoke [a-z]*' | sort -u | tr '\n' ' '); envelopes ${ENV:-none}; wrong password: $(echo "$WR" | grep -o 'Wrong password' | head -1); tab: $(echo "$TB" | cut -c1-50); quick at ${QK:-nowhere})" "[ \"${ENV:-0}\" = 2 ] && echo \"$RL\" | grep -q 'Smoke colleague' && echo \"$RL\" | grep -q 'Smoke me' && echo \"$WR\" | grep -q 'Wrong password' && echo \"$TB\" | grep -q certenc && [ -n \"$QK\" ]"
quit

# 321. Preferences ▸ Signatures ▸ Create a digital ID… (C-sign-013, Acrobat's Add Digital ID ▸ A new digital ID): a
# name and a password typed twice, Create and save: a self-signed .p12 (it opens with that password) and its .cer
# beside it, issued to that name with key encipherment (usable for certificate encryption), kept in the list of IDs.
rm -f "$OUT/smoke-id.p12" "$OUT/smoke-id.cer"
launch; key ctrl+comma; sleep 0.8
read CX CY <<< "$(pref_find Create)"; [ -n "$CY" ] && click $CX $CY 0.8
# The form's labels start at x 234, left of pref_find's crop: the same scroll and find, from x 226. Each
# field is found from its own label once the form has opened and the page has stopped moving (the Name
# field was clicked at a place computed from the button, before the form had settled: left empty, 28/10).
form_find() { local f=""; for _ in $(seq 1 10); do f=$(at "$1" 1040x700+226+90); [ -n "$f" ] && break; xdotool mousemove 760 500 click 5 click 5 click 5; sleep 0.3; done; echo "$f"; }
sleep 0.8
# "Name" alone also starts other words on the page: the Name field is the one just above the
# Organizational unit label.
read _ NY <<< "$(form_find Organizational)"; [ -n "$NY" ] && { click 700 $(( NY - 31 )) 0.6; xdotool type --delay 30 "Smoke Signer"; }
read _ PY <<< "$(form_find Password)"; [ -n "$PY" ] && { click 700 $(( PY + 30 )) 0.6; xdotool type --delay 30 smokepw1; }
read _ QY <<< "$(form_find Confirm)"; [ -n "$QY" ] && { click 700 $(( QY + 30 )) 0.6; xdotool type --delay 30 smokepw1; }
shot 321-form
# Create and save… sits under the "Use it for certificate encryption too" box.
xdotool mousemove 760 500 click 5 click 5 click 5; sleep 0.4
answer "$PWD/$OUT/smoke-id.p12"; read _ SY <<< "$(form_find Use)"; [ -n "$SY" ] && click 310 $(( SY + 34 )) 0.5
for _ in $(seq 1 30); do [ -f "$OUT/smoke-id.cer" ] && break; sleep 0.5; done; sleep 0.5; answer ""
IDS=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("digital_ids"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
SUBJ=$(openssl pkcs12 -in "$OUT/smoke-id.p12" -passin pass:smokepw1 -nokeys 2>/dev/null | openssl x509 -noout -subject -ext keyUsage 2>/dev/null | tr '\n' ' ')
check "Create a digital ID writes a self-signed .p12 and its .cer, kept in the list (${CY:+button at $CY}; $SUBJ; ids $IDS)" "[ -f \"$OUT/smoke-id.cer\" ] && echo \"$SUBJ\" | grep -q 'CN *= *Smoke Signer' && echo \"$SUBJ\" | grep -q 'Key Encipherment' && echo \"$IDS\" | grep -q smoke-id.p12"
quit

# 322. Prepare form ▸ Barcode (C-forms-011, Acrobat's barcode field): its error correction set to Quartile on the
# options bar, a text field and a barcode field placed; back on the page, "Ada" typed in the text field: the barcode
# is drawn again with it (a QR Code: dark modules filling its square) and saved with Acrobat's /PMD, the data in its value.
cp "$OUT/plain.pdf" "$OUT/barcode.pdf"; launch "$OUT/barcode.pdf"; key ctrl+9; sleep 1; key Home; sleep 0.5; key f; sleep 0.6
BK=$(at Barcode 1280x60+0+80); [ -n "$BK" ] && click $BK 0.6
EK=$(at Error 1280x60+0+80); [ -n "$EK" ] && click $EK 0.4
EC=$(ocr 1280x60+0+80)
TK=$(at Text 1280x60+0+80); [ -n "$TK" ] && click $TK 0.5
click 500 300 1.2; key Escape; sleep 0.3
[ -n "$BK" ] && click $BK 0.5
click 700 400 1.5; key Escape; sleep 0.3; key v; sleep 0.5
click 540 311 0.6; xdotool type --delay 30 "Ada"; key Tab; sleep 1.5
shot 322-barcode
# The barcode's square (108 pt from its top-left corner at 700,400, at the zoom ⌘9 set): dark share of its pixels.
DK=$(import -window root -crop 100x100+705+405 png:- 2>/dev/null | convert - -colorspace gray -threshold 50% -format '%[fx:1-mean]' info: 2>/dev/null)
key ctrl+s; sleep 1.5
BQ=$(qpdf --qdf --object-streams=disable "$OUT/barcode.pdf" - 2>/dev/null)
check "a barcode field draws the form's data as a QR Code and follows the typing (${BK:-no Barcode kind}; $(echo "$EC" | grep -o 'Error correction: [A-Za-z]*'); dark ${DK:-?}; $(echo "$BQ" | grep -ao '/ECC [0-9]' | head -1); value $(echo "$BQ" | grep -ao '/V (Text1[^)]*)' | head -1))" "echo \"$EC\" | grep -q 'Quartile' && echo \"\$BQ\" | grep -aq '/Symbology /QRCode' && echo \"\$BQ\" | grep -aq '/ECC 2' && echo \"\$BQ\" | grep -aq '/V (Text1.nAda)' && python3 -c 'import sys; sys.exit(0 if 0.25 < float(sys.argv[1]) < 0.75 else 1)' ${DK:-0}"
quit
# The AI steps' provider: a mock OpenAI-compatible server (smoke/ai-mock.py) on a free local port, set as
# Preferences ▸ AI ▸ OpenAI-compatible server; each request it gets is logged in $OUT/ai-requests.jsonl.
ai_mock() { ai_stop; rm -f "$OUT/ai-port" "$OUT/ai-requests.jsonl" "$OUT/profile/data/Sheaf/ai-spaces.json"; touch "$OUT/ai-requests.jsonl"; python3 dev/scripts/smoke/ai-mock.py "$OUT/ai-port" "$OUT/ai-requests.jsonl" >/dev/null 2>&1 & AI_MOCK=$!; for _ in $(seq 1 20); do [ -s "$OUT/ai-port" ] && break; sleep 0.2; done; prefs_set "p['ai'] = {'provider': 'compatible', 'base_urls': {'compatible': 'http://127.0.0.1:$(cat "$OUT/ai-port")/v1'}, 'notice_seen': ['compatible'] if '${1:-}' == 'seen' else []}"; }
ai_stop() { [ -n "${AI_MOCK:-}" ] && kill "$AI_MOCK" 2>/dev/null; AI_MOCK=; prefs_set "p.pop('ai', None)"; }
# ✦ AI Assistant, left of the wrench in the reader's toolbar.
# (the pointer then leaves the button, so its tooltip does not cover the panel)
ai_open() { click 1090 61 0.3; xdotool mousemove 600 450; sleep 1.2; }
ai_text() { ocr 380x700+900+100; }
ai_wait() { for _ in $(seq 1 30); do ai_text | grep -q "$1" && break; sleep 0.5; done; }
# Clicks the first word of $1 seen in the panel, else its other words ("Main points": Main, then points).
ai_chip() { local c w; for w in $1; do c=$(at "$w" 380x700+900+100); [ -n "$c" ] && break; done; [ -n "$c" ] && click $c 0.5; echo "${c:-no $1}"; }
# The last word starting with $1 in the panel (a message's button, under its text).
ai_last() { import -window root -crop 380x700+900+100 png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' -v w="$1" '{ t = $12; sub(/^[^A-Za-z0-9]+/, "", t) } index($12, w) == 1 || index(t, w) == 1 { p = sprintf("%d %d", 900 + ($7 + $9 / 2) / 2, 100 + ($8 + $10 / 2) / 2) } END { print p }'; }
ai_requests() { wc -l < "$OUT/ai-requests.jsonl" 2>/dev/null || echo 0; }

# 360. The AI panel (C-ai-001, C-ai-002; PDF Expert's AI Copilot, Acrobat's AI Assistant): ✦ in the toolbar opens it
# beside the pages, naming the provider; the first request waits on the note that only the text is sent (OK
# sends it); a question typed is answered from the pages: the request carries page 1's text and the question.
actions_reset
ai_mock
cp "$OUT/text.pdf" "$OUT/ai.pdf"; launch "$OUT/ai.pdf"; sleep 1
ai_open; T1=$(ai_text)
# The question field: the panel's bottom row, left of Ask (the panel is anchored to the window's bottom right).
click 1040 768 0.4
xdotool type --delay 15 "Who jumps?"; key Return; sleep 1
N0=$(ai_requests)
# OK sits left of Turn off AI (white on red, which OCR does not read).
read TX TY <<< "$(at Turn 380x500+900+100)"; [ -n "$TY" ] && click $(( TX - 60 )) $TY 0.5
ai_wait Mockanswer; T2=$(ai_text); shot ai-360
SEEN=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["ai"]["notice_seen"])' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
tail -1 "$OUT/ai-requests.jsonl" > "$OUT/ai-last.json" 2>/dev/null
check "the AI panel names the provider, holds the first request for the note, then answers from the pages, streamed (note: $(echo "$T1" | grep -o 'never the file' | head -1); held: $N0 requests; seen $SEEN; answer: $(echo "$T2" | grep -o 'Mockanswer[a-z ]*' | head -1))" \
    "echo \"$T1\" | grep -q 'Assistant' && echo \"$T1\" | grep -q 'never the file' && [ \"$N0\" = 0 ] && echo \"$SEEN\" | grep -q compatible && echo \"$T2\" | grep -q Mockanswer && grep -q 'Who jumps' $OUT/ai-last.json && grep -q 'p. 1' $OUT/ai-last.json && grep -q 'quick brown fox' $OUT/ai-last.json && grep -q '\"stream\": true' $OUT/ai-last.json"
quit; ai_stop

# 361. Summarize and translate (C-ai-003, C-ai-004; PDF Expert's): the Main points chip and Translate (French) each
# send the pages' text with their own request, and their answers show in the panel.
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; launch "$OUT/ai.pdf"; sleep 1
ai_open; N1=$(ai_requests)
P1=$(ai_chip "Main points"); ai_wait Mockpoint
T2M=$(ai_text)
# The tools fold once the conversation starts: Tools in the panel's header unfolds them.
read TX TY <<< "$(at Tools 380x60+900+90)"; [ -n "$TY" ] && click $TX $TY 0.6
P2=$(ai_chip Translate); ai_wait renard; T3=$(ai_text); shot ai-361
RQ=$(tail -2 "$OUT/ai-requests.jsonl" 2>/dev/null | tr -d "'")
check "Main points and Translate ask the provider and show the answers (chips $P1 / $P2; requests $N1 → $(ai_requests); $(echo "$T3" | grep -o 'Mockpoint\|renard' | tr '\n' ' '))" \
    "echo \"$T2M\" | grep -q Mockpoint && echo \"$T3\" | grep -q renard && echo '$RQ' | grep -q 'main points' && echo '$RQ' | grep -q 'Translate into English'"
quit; ai_stop

# 362. Slides (C-ai-005, Acrobat's Generate presentation): Slides (PDF) asks for an outline, saves it as a deck
# (the dialog answered $OUT/ai-deck.pdf) of one 16:9 page per slide holding the outline's titles.
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; rm -f "$OUT/ai-deck.pdf"; echo "$PWD/$OUT/ai-deck.pdf" > "$OUT/dialog-answer"
launch "$OUT/ai.pdf"; sleep 1
ai_open; S1=$(ai_chip "(PDF"); for _ in $(seq 1 30); do [ -s "$OUT/ai-deck.pdf" ] && break; sleep 0.5; done; sleep 0.5; shot ai-362
DP=$(pdfinfo "$OUT/ai-deck.pdf" 2>/dev/null | awk '/^Pages/ { print $2 } /^Page size/ { print $3 "x" $5 }' | tr '\n' ' ')
DT=$(pdftotext "$OUT/ai-deck.pdf" - 2>/dev/null | tr '\n' ' ')
check "Slides (PDF) writes the outline as a 16:9 deck (chip $S1; $DP; $(echo "$DT" | cut -c1-40))" "[ \"\$(echo $DP | cut -d' ' -f1)\" = 2 ] && echo '$DP' | grep -q '960x540' && echo \"$DT\" | grep -q 'Mock deck' && echo \"$DT\" | grep -q 'Second slide'"
quit; rm -f "$OUT/dialog-answer"; ai_stop

# 363. AI spaces and Classify (C-ai-006, C-ai-007; Acrobat's AI spaces, Stirling's Classify): New space holds the
# open PDF, Add PDFs… a second; Classify space with "Animals, Invoices" asks once per PDF and groups them; the space
# is kept in ai-spaces.json with its two files.
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; cp "$OUT/text.pdf" "$OUT/ai-two.pdf"; echo "$PWD/$OUT/ai-two.pdf" > "$OUT/dialog-answer"
launch "$OUT/ai.pdf"; sleep 1
# New space, right of the spaces list; then Add PDFs… (the space's row, under it).
ai_open; NS=$(ai_chip New); sleep 0.8; A1=$(ai_chip Add); sleep 1; shot ai-363a
CF=$(at Categories 380x700+900+100); read CX CY <<< "$CF"; [ -n "$CY" ] && click $CX $CY 0.4; xdotool type --delay 15 "Animals, Invoices"
N2=$(ai_requests); C1=$(ai_chip Classify); ai_wait 'Animals:'; T4=$(ai_text); shot ai-363
SP=$(python3 -c 'import json, sys; s = json.load(open(sys.argv[1])); print(s[0]["name"], len(s[0]["files"]))' "$PWD/$OUT/profile/data/Sheaf/ai-spaces.json" 2>/dev/null)
# Stirling keeps the class in each PDF: both files carry Category Animals (pdfinfo's custom metadata), and the
# space lists them with it.
sleep 1.5; T4B=$(ai_text)
L1=$(pdfinfo -custom "$OUT/ai.pdf" 2>/dev/null | awk -F': *' '/^Category/ { print $2 }'); L2=$(pdfinfo -custom "$OUT/ai-two.pdf" 2>/dev/null | awk -F': *' '/^Category/ { print $2 }')
check "a space holds two PDFs and Classify sorts them, the class kept in each PDF (new $NS, add $A1, classify $C1; requests $N2 → $(ai_requests); space: $SP; $(echo "$T4" | grep -o 'Animals: [a-z.,-]*' | head -1); labels '$L1' '$L2'; list: $(echo "$T4B" | grep -o 'ai.pdf . Animals' | head -1))" \
    "[ \"$SP\" = 'Space 2' ] && [ \$(( \$(ai_requests) - $N2 )) = 2 ] && echo \"$T4\" | grep -q 'Animals: ai' && [ \"$L1\" = Animals ] && [ \"$L2\" = Animals ] && echo \"$T4B\" | grep -q 'pdf . Animals'"
# Stirling's category filter: chips with their number of files above the list ("All 2", "Animals 2").
CH=$(echo "$T4B" | grep -oE '(All|Animals) ?2' | tr '\n' ' ')
check "the space's categories are chips with their counts (chips: ${CH:-none})" "echo \"$CH\" | grep -q All && echo \"$CH\" | grep -q Animals"
quit; rm -f "$OUT/dialog-answer"; ai_stop
# 300. Right to left on Home's Tools page and the reader's print sheet (C-platform-004, iLovePDF's right-to-left
# interface): the Tools page's title row reads from the right (the search field left of Custom tools), and the print sheet's
# options sit right of its preview (Copies past the middle, the preview's note left of it).
prefs_set 'p.setdefault("platform", {})["direction"] = 2'
launch; sleep 0.5
TL=$(at Tools 300x300+980+40); [ -n "$TL" ] && click $TL 0.8
read SX _ <<< "$(at Search 1280x80+0+40)"; read UX _ <<< "$(at Custom 1280x80+0+40)"; shot 300a-rtl-tools
quit
cp "$OUT/text.pdf" "$OUT/rtlprint.pdf"; launch "$OUT/rtlprint.pdf"; sleep 1
key ctrl+p; sleep 1; shot 300b-rtl-print
# The title read on the sheet's top row alone (on the whole sheet tesseract now misses it and finds "Printer's
# default" first, which also starts with Print).
read CX _ <<< "$(at Copies 1280x700+0+60)"; read PX _ <<< "$(at Print 720x40+280+206)"
key Escape; sleep 0.3
check "right to left mirrors the Tools page's title row and the print sheet (search x ${SX:-?} < Custom tools ${UX:-?}; Copies at x ${CX:-?}, Print title at ${PX:-?})" "[ -n \"$SX\" ] && [ -n \"$UX\" ] && [ \"$SX\" -lt \"$UX\" ] && [ \"${CX:-0}\" -gt 700 ] && [ \"${PX:-0}\" -gt 900 ]"
quit; platform_reset

# 301. Preferences ▸ Printing (C-preferences-010, iLovePDF's Printer preferences: paper size, grayscale): Grey,
# Two-sided and A4 chosen are kept, and the print sheet opens with them (Grey and On red); the sheet's Colour then
# Make default writes colour back.
settings_val() { python3 -c 'import json, sys; p = json.load(open(sys.argv[1])); print(eval(sys.argv[2]))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" "$1" 2>/dev/null; }
settings_reset() { python3 -c 'import json, sys; p = json.load(open(sys.argv[1])); p.pop("settings", None); json.dump(p, open(sys.argv[1], "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null; }
# The accent (a selected button's accent text; Sheaf blue by default since the redesign, W1 29/10/2026)
# anywhere in a 40×10 box left of a word's centre.
red_at() { import -window root -crop "40x10+$(( $1 - 30 ))+$(( $2 - 5 ))" -depth 8 txt:- 2>/dev/null | grep -o '#[0-9A-F]\{6\}' | python3 -c 'import sys; sys.exit(0 if any(int(c[5:7], 16) > 150 and int(c[1:3], 16) < 90 and int(c[3:5], 16) < 130 for c in sys.stdin.read().split()) else 1)'; }
launch; key ctrl+comma; sleep 0.8
G=$(pref_find Grey); [ -n "$G" ] && click $G 0.6
T=$(at Two-sided 1020x700+250+90); [ -n "$T" ] && click $T 0.6
# "A4" alone does not OCR: the segment left of Letter (their centres 98 px apart).
read LX LY <<< "$(at Letter 1020x700+250+90)"; A=; [ -n "$LX" ] && A="$(( LX - 98 )) $LY" && click $A 0.6
P1=$(settings_val 'p["settings"]["print"]')
quit
cp "$OUT/text.pdf" "$OUT/printdef.pdf"; launch "$OUT/printdef.pdf"; sleep 1
key ctrl+p; sleep 1; shot 301-print-defaults
# The chosen buttons are a tint with red labels (BAMA `on`): Grey, two-sided On, Colour and Make default are
# found by their words.
read MX MY <<< "$(at Make 1280x700+0+60)"; read GX GY <<< "$(at Grey 1280x700+0+60)"
read OX OY <<< "$(at On 1280x700+0+60)"; read CX CY <<< "$(at Colour 1280x700+0+60)"
GR=no; OR=no
if [ -n "$MX" ] && [ -n "$GX" ] && [ -n "$CX" ]; then
    red_at $GX $GY && GR=yes; [ -n "$OX" ] && red_at $OX $OY && OR=yes
    CL="$CX $CY"; click $CL 0.4; MD="$MX $MY"; click $MD 0.6
fi
P2=$(settings_val 'p["settings"]["print"]["gray"]')
key Escape; sleep 0.3
check "print defaults are kept and the sheet opens with them ($P1; Grey at ${GX:-?},${GY:-?} red $GR, On at ${OX:-?},${OY:-?} red $OR; Colour ${CL:-?}, Make default ${MD:-?}: gray $P2)" "echo \"$P1\" | grep -q \"'two_sided': True\" && echo \"$P1\" | grep -q \"'paper': 1\" && echo \"$P1\" | grep -q \"'gray': True\" && [ $GR = yes ] && [ $OR = yes ] && [ \"$P2\" = False ]"
quit; settings_reset

# 302. Preferences ▸ Security (C-preferences-014, Acrobat's Enhanced Security and Trust Manager): a document's web
# link asks first ("Open a page of example.org?"); Block with Remember keeps the site blocked, and the next click
# opens no question; a folder added as a trusted location is kept.
launch "$OUT/links.pdf"; key ctrl+9; sleep 0.8
read VX VY <<< "$(at Visit 1280x700+0+90)"; [ -n "$VX" ] && click $VX $VY 1
Q1=$(ocr 700x300+290+250); shot 302a-link-question
# Block, bold in its frame, does not OCR: 193 px right of and 43 px below "Remember for this site".
read RX RY <<< "$(at Remember 1280x700+0+90)"; R="${RX:-} ${RY:-}"; B=
if [ -n "$RX" ]; then click $RX $RY 0.4; B="$(( RX + 193 )) $(( RY + 43 ))"; click $B 0.6; fi
BL=$(settings_val 'p["settings"]["trust"]["blocked_sites"]')
[ -n "$VX" ] && click $VX $VY 1
Q2=$(ocr 700x300+290+250)
quit
mkdir -p "$OUT/trusted"; answer "$PWD/$OUT/trusted"
launch; key ctrl+comma; sleep 0.8
AF=$(pref_find "folder…"); [ -z "$AF" ] && AF=$(pref_find Add); [ -n "$AF" ] && click $AF 1
LOC=$(settings_val 'p["settings"]["trust"]["locations"]'); shot 302b-trusted
answer ""
check "a web link asks first, a blocked site stays blocked, a trusted folder is kept ($(echo "$Q1" | cut -c1-50); Remember ${R:-?}, Block ${B:-?}: $BL; then $(echo "$Q2" | cut -c1-30); $LOC)" "echo \"$Q1\" | grep -q 'example.org' && echo \"$BL\" | grep -q \"'example.org'\" && ! echo \"$Q2\" | grep -q 'Open a page' && echo \"$LOC\" | grep -q 'trusted'"
quit; settings_reset

# 303. Preferences ▸ Startup (C-preferences-012, iLovePDF's "Open iLovePDF automatically after you log into the
# computer"): Open Sheaf writes the login item (XDG autostart, pointing at the app), Don't open it removes it.
launch; key ctrl+comma; sleep 0.8
# The row's label ("When I log into the computer") found by "log", the choices 34 px under it: Open Sheaf 5 px left
# of "log", Don't open it 112 px right.
read FX FY <<< "$(pref_find log)"; shot 303-startup; OS=; DO=
[ -n "$FX" ] && OS="$(( FX - 5 )) $(( FY + 34 ))" && DO="$(( FX + 112 )) $(( FY + 34 ))"
[ -n "$OS" ] && click $OS 0.8
AUTO="$OUT/profile/config/autostart/sheaf.desktop"; A1=$(grep -c "^Exec=.*sheaf-app-under-test" "$AUTO" 2>/dev/null)
[ -n "$DO" ] && click $DO 0.8
A2=no; [ -e "$AUTO" ] && A2=yes
check "Open Sheaf at login writes the autostart entry and Don't open it removes it (Open at ${OS:-?}: Exec lines ${A1:-0}; Don't at ${DO:-?}: still there $A2)" "[ \"${A1:-0}\" = 1 ] && [ $A2 = no ]"
quit; settings_reset

# 304. Help ▸ Report an Issue… (C-platform-003, iLovePDF's Contact us): the message typed and Save report… writes
# one zip with report.txt holding the subject, the message and the app's details (Sheaf's version).
export SHEAF_TEST_MENU_BAR=1
launch; sleep 0.5
HX=$(at Help 700x34+0+0); [ -n "$HX" ] && click $HX 0.8
RI=$(at Report 420x300+0+24); [ -n "$RI" ] && click $RI 1
xdotool type --delay 20 "Saving freezes on big files"; sleep 0.4; shot 304-report
answer "$PWD/$OUT/report.zip"
# Save report… read on the dialog's lower half (the whole screen's OCR misses its button row).
SR=$(at Save 580x350+350+400); [ -n "$SR" ] && click $SR 1.5
answer ""
RT=$(unzip -p "$OUT/report.zip" report.txt 2>/dev/null | tr '\n' ' ')
check "Report an Issue saves the report as a zip (${RI:-no menu item}; $(echo "$RT" | cut -c1-90))" "echo \"$RT\" | grep -q 'Report a problem: Saving freezes on big files' && echo \"$RT\" | grep -q 'Sheaf [0-9]'"
quit
unset SHEAF_TEST_MENU_BAR
# 340. Acrobat's Optimizer: PDF 1.3 transparency flattening followed by Fast Web View.
# Locate controls in the actual options scroller, then inspect the produced PDF, not just the UI.
optimize_option() { local found=; for _ in $(seq 1 16); do found=$(at "$1" 310x640+965+90); [ -n "$found" ] && break; xdotool mousemove 1160 500 click 5 click 5; sleep 0.3; done; echo "$found"; }
cp "$OUT/text.pdf" "$OUT/web.pdf"; launch "$OUT/web.pdf"; sleep 1
click $WRENCH_X 61 1; click 990 76 0.4; xdotool type --delay 20 "compress"; key Return; sleep 2
OA=$(optimize_option Advanced); [ -n "$OA" ] && click $OA 0.5
OF=$(optimize_option Flatten); [ -n "$OF" ] && click $OF 0.5
OW=$(optimize_option Fast); [ -n "$OW" ] && click $OW 0.5
shot 340-optimizer
click 1140 755 5
for _ in $(seq 1 20); do [ -f "$OUT/web-compressed.pdf" ] && break; sleep 0.5; done
check "340 optimizer saves flattened Fast Web View ($OA; $OF; $OW)" "[ -n \"$OF\" ] && [ -n \"$OW\" ] && head -c 2048 \"$OUT/web-compressed.pdf\" | grep -aq /Linearized && head -c 8 \"$OUT/web-compressed.pdf\" | grep -q %PDF-1.3 && qpdf --check-linearization \"$OUT/web-compressed.pdf\" >/dev/null 2>&1 && [ \"\$(npages \"$OUT/web-compressed.pdf\")\" = 4 ] && pdftotext \"$OUT/web-compressed.pdf\" - | grep -q 'Page 1'"
quit

# 341. Acrobat's display colour management: choose a real monitor ICC, keep it, reset to sRGB.
launch; key ctrl+comma; sleep 0.8
IC=$(pref_find monitor); answer "/usr/share/color/icc/ghostscript/a98.icc"; [ -n "$IC" ] && click $IC 1; answer ""
IP=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("display_profile"))' "$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
shot 341-display-profile
IS=$(at sRGB 1020x700+250+90); [ -n "$IS" ] && click $IS 0.5
IR=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("display_profile"))' "$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
check "341 display ICC persists and resets to sRGB ($IC; $IP; $IR)" "[ \"$IP\" = '/usr/share/color/icc/ghostscript/a98.icc' ] && [ \"$IR\" = None ]"
quit

# 342. Acrobat's Output Preview: the captured page has selectable process ink plates.
optimize_print_tool() { click 1210 61 0.8; click 1008 659 0.8; local found=; for _ in $(seq 1 12); do found=$(at "$1" 300x690+880+85); [ -n "$found" ] && break; xdotool mousemove 1050 530 click 5 click 5; sleep 0.3; done; [ -n "$found" ] && click $found 1; echo "$found"; }
INK_SOURCE=$(sha256sum "$OUT/text.pdf" | cut -d" " -f1)
launch "$OUT/text.pdf"; PP=$(optimize_print_tool preflight)
PI=$(at Ink); [ -n "$PI" ] && click $PI 1
# Read the complete button column below the selected-plate heading, then choose Black.
# The old x=0..250 crop cut words in half and clicked "Black shows ink" in the help.
for _ in $(seq 1 20); do PB=$(at Black 180x180+195+175); [ -n "$PB" ] && break; sleep 0.5; done
PM=$(at Magenta 180x180+195+175); PY=$(at Yellow 180x180+195+175)
[ -n "$PB" ] && click $PB 0.5
PC=$(at Cyan 180x180+195+175)
# Only the page image: exclude the coloured plate buttons and include its top text.
optimize_ink_dark() { import -window root -crop 500x300+450+180 -depth 8 png:- 2>/dev/null | convert - -threshold 25% -format "%[fx:1-mean]" info:; }
INK_DARK=$(optimize_ink_dark); key Up; sleep 0.5; INK_YELLOW=$(optimize_ink_dark); key Down; sleep 0.5; INK_RETURN=$(optimize_ink_dark)
shot 342-ink-plates
check "342 output preview lists real CMYK plates ($PP; $PI; $PC; $PM; $PY; $PB)" "[ -n \"$PC\" ] && [ -n \"$PM\" ] && [ -n \"$PY\" ] && [ -n \"$PB\" ] && python3 -c \"assert float('$INK_DARK') > float('$INK_YELLOW') + 0.0003 and float('$INK_RETURN') > float('$INK_YELLOW') + 0.0003\" && [ \"$INK_SOURCE\" = \"\$(sha256sum \"$OUT/text.pdf\" | cut -d' ' -f1)\" ]"
# 342b. Several inks together (C-optimize-012, Sol: only one isolated plate could be shown): "All ticked inks"
# shows the page in its inks; Black's box cleared, the black text is gone; ticked again, it is back.
read AX AY <<< "$(at All 220x260+190+170)"; [ -n "$AY" ] && click $AX $AY 0.8
ALL_ON=$(optimize_ink_dark)
# Each tick redraws the composite (a 1200 px page, slower in a debug build): its effect is waited for.
read _ BY <<< "$PB"
xdotool mousemove 600 600; sleep 0.3; click 207 ${BY:-0} 0.5
for _ in $(seq 1 12); do ALL_NO_BLACK=$(optimize_ink_dark); python3 -c "import sys; sys.exit(0 if float('$ALL_NO_BLACK') < float('$ALL_ON') / 4 else 1)" && break; sleep 0.5; done
xdotool mousemove 600 600; sleep 0.3; click 207 ${BY:-0} 0.5
for _ in $(seq 1 12); do ALL_BACK=$(optimize_ink_dark); python3 -c "import sys; sys.exit(0 if abs(float('$ALL_BACK') - float('$ALL_ON')) < 0.002 else 1)" && break; sleep 0.5; done
shot 342b-inks-together
check "342b the ticked inks show together, Black cleared takes the black text away ($AY; $ALL_ON → $ALL_NO_BLACK → $ALL_BACK)" "python3 -c \"assert float('$ALL_ON') > 0.002 and float('$ALL_NO_BLACK') < float('$ALL_ON') / 4 and abs(float('$ALL_BACK') - float('$ALL_ON')) < 0.002\""
key Escape; quit

# 343. Acrobat's Flattener Preview: compare the working page and actual PDF 1.3 conversion.
FLAT_SOURCE=$(sha256sum "$OUT/text.pdf" | cut -d" " -f1)
launch "$OUT/text.pdf"; PF=$(optimize_print_tool hairlines)
FP=
for _ in $(seq 1 14); do FP=$(at Flattener 700x660+0+80); [ -n "$FP" ] && break; xdotool mousemove 400 500 click 5 click 5; sleep 0.3; done
[ -n "$FP" ] && click $FP 1
for _ in $(seq 1 20); do FA=$(at Flattened 180x180+195+175); [ -n "$FA" ] && break; sleep 0.5; done
F_ORIGINAL=$(ocr 660x500+382+180)
[ -n "$FA" ] && click $FA 0.5
F_FLATTENED=$(ocr 660x500+382+180)
FO=$(at Original 180x180+195+175)
shot 343-flattener-preview
check "343 flattener preview compares original and converted page ($PF; $FP; $FO; $FA)" "[ -n \"$FO\" ] && [ -n \"$FA\" ] && echo \"$F_ORIGINAL\" | grep -q 'Page 1' && echo \"$F_FLATTENED\" | grep -q 'Page 1' && [ \"$FLAT_SOURCE\" = \"\$(sha256sum \"$OUT/text.pdf\" | cut -d' ' -f1)\" ]"
key Escape; quit

# 345. Acrobat's Save as Other ▸ PDF/E (C-convert-019): the Production sheet's Save PDF/E copy… writes the picture
# document as PDF/E-1 (PDF 1.6, identified in its metadata) beside it; the working document is unchanged.
images_pdf_e="$OUT/pdfe-src.pdf"; convert -size 300x200 xc:'#2a9c4b' "$images_pdf_e" 2>/dev/null
rm -f "$OUT/pdfe-out.pdf"; launch "$images_pdf_e"; PF=$(optimize_print_tool hairlines)
PE=
for _ in $(seq 1 16); do PE=$(at "PDF/E" 700x660+0+80); [ -n "$PE" ] && break; xdotool mousemove 400 500 click 5 click 5; sleep 0.3; done
read SX SY <<< "$(for _ in 1; do import -window root -crop 700x660+0+80 png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' '$12=="copy…" || $12=="copy..." {print int(($7+$9/2)/2), int(80+($8+$10/2)/2)}' | tail -1; done)"
answer "$PWD/$OUT/pdfe-out.pdf"; [ -n "$SY" ] && click $(( SX - 60 )) $SY 1
for _ in $(seq 1 30); do [ -s "$OUT/pdfe-out.pdf" ] && break; sleep 0.5; done; answer ""
shot 345-pdfe
PV=$(python3 -c 'import sys; d=open(sys.argv[1],"rb").read(); print(d[:8].decode("latin1"), b"<pdfe:ISO_PDFEVersion>PDF/E-1" in d)' "$OUT/pdfe-out.pdf" 2>/dev/null)
check "345 Save PDF/E copy writes PDF/E-1 beside the document ($PF; ${PE:-no PDF/E}; button ${SY:-none}; $PV)" "echo \"$PV\" | grep -q '%PDF-1.6 True'"
key Escape; quit

# 346. Acrobat's Trap Presets (C-optimize-014, Sol: no trap setting or calculation): the Production sheet's Save
# trapped plates… writes one TIFF per ink where the yellow page runs under the cyan box's edge (1 px at 300 dpi,
# Default's 0.25 pt), and Preview traps opens the plates titled with the preset.
printf '%s\n' '<< /PageSize [72 72] >> setpagedevice' '0 0 1 0 setcmykcolor 0 0 72 72 rectfill' '1 0 0 0 setcmykcolor 18 18 36 36 rectfill' showpage > "$OUT/trap-src.ps"
gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=pdfwrite -o "$OUT/trap-src.pdf" "$OUT/trap-src.ps"
rm -rf "$OUT/traps"; mkdir -p "$OUT/traps"
trap_sheet() { PF=$(optimize_print_tool hairlines); TP=; for _ in $(seq 1 16); do TP=$(at traps 700x660+0+80); [ -n "$TP" ] && break; xdotool mousemove 400 500 click 5; sleep 0.3; done; }
launch "$OUT/trap-src.pdf"; trap_sheet
read TX TY <<< "$(import -window root -crop 700x660+0+80 png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' '$12=="plates…" || $12=="plates..." {print int(($7+$9/2)/2), int(80+($8+$10/2)/2)}' | tail -1)"
answer "$PWD/$OUT/traps"; [ -n "$TY" ] && click $(( TX - 40 )) $TY 1
for _ in $(seq 1 40); do [ -s "$OUT/traps/trap-src p1 Black.tif" ] && break; sleep 0.5; done; answer ""
TRAP_Y=$(convert "$OUT/traps/trap-src p1 Yellow.tif" -crop 1x1+75+150 -depth 8 gray:- 2>/dev/null | od -An -tu1 | tr -d ' ')
TRAP_IN=$(convert "$OUT/traps/trap-src p1 Yellow.tif" -crop 1x1+150+150 -depth 8 gray:- 2>/dev/null | od -An -tu1 | tr -d ' ')
trap_sheet; shot 346-trap-sheet; [ -n "$TP" ] && click $TP 1
TT=; for _ in $(seq 1 30); do TT=$(ocr 900x80+180+40 | grep -o 'Traps (Default)'); [ -n "$TT" ] && break; sleep 0.5; done
for _ in $(seq 1 40); do [ -n "$(at Cyan 180x180+195+175)" ] && break; sleep 0.5; done
shot 346-traps
check "346 trapped plates saved, yellow 1 px under the cyan edge; Preview traps opens ($PF; ${TP:-no button}; ${TY:-no save}; yellow at edge ${TRAP_Y:-none}, inside ${TRAP_IN:-none}; $(ls "$OUT/traps" | wc -l) files; ${TT:-no title})" "[ \"$(ls "$OUT/traps" | wc -l)\" = 4 ] && [ \"${TRAP_Y:-255}\" -lt 64 ] && [ \"${TRAP_IN:-0}\" -gt 192 ] && [ -n \"$TT\" ]"
key Escape; quit

# 344. iLovePDF's searchable OCR: automatic backend from the real options panel, with local packs.
convert -size 1000x200 xc:white -font DejaVu-Sans -pointsize 64 -fill black -gravity center -annotate 0 "Sheaf OCR test 2026" "$OUT/fast-ocr-input.png"
convert "$OUT/fast-ocr-input.png" "$OUT/fast-ocr-input.pdf"
launch "$OUT/fast-ocr-input.pdf"; sleep 1
click $WRENCH_X 61 1; click 990 76 0.4; xdotool type --delay 20 "ocr"; key Return; sleep 2
OR=$(optimize_option Require)
shot 344-ocr-options
click 1140 755 4
for _ in $(seq 1 30); do [ -f "$OUT/fast-ocr-input-ocr.pdf" ] && break; sleep 0.5; done
OT=$(pdftotext "$OUT/fast-ocr-input-ocr.pdf" - 2>/dev/null | tr '\n' ' ')
check "344 automatic OCR writes searchable text ($OR; $OT)" "[ -n \"$OR\" ] && echo \"$OT\" | grep -q 'Sheaf OCR' && qpdf --check \"$OUT/fast-ocr-input-ocr.pdf\" >/dev/null 2>&1"
quit

# 350. Stirling Folder Scanning's outputFileName survives import, and Export Stirling retains
# it in a portable file. Inspection checks both the saved action and the exported pipeline.
# First-launch rendering can outlast launch()'s initial draw. Wait for the actual Actions
# header before using its controls; repeating the navigation click is safe and idempotent.
batch_actions() {
    local point
    for _ in $(seq 1 20); do
        point=$(at Import 140x60+220+180)
        [ -n "$point" ] && return 0
        point=$(at Actions 190x220+0+40)
        [ -n "$point" ] && click $point 0.5
        sleep 0.3
    done
    return 1
}
python3 - "$OUT/350-pipeline.json" <<'PY350'
import json, sys
json.dump({"name":"Naming review", "pipeline":[{"operation":"rotate","parameters":{"angle":90}}],
           "outputDir":"{outputFolder}", "outputFileName":"ready-{filename}-{pipelineName}"}, open(sys.argv[1], "w"))
PY350
launch
batch_actions
answer "$PWD/$OUT/350-pipeline.json"
IM=$(at Import 140x60+220+180); [ -n "$IM" ] && click $IM 1
answer "$PWD/$OUT/350-export.json"
EX=$(at Export 200x50+1080+215); [ -n "$EX" ] && click $EX 1
shot 350-stirling-naming
check "350 Stirling output naming survives import and export (${IM:-no Import}; ${EX:-no Export})" "python3 -c 'import json; a=json.load(open(\"$OUT/350-export.json\")); b=json.load(open(\"$OUT/profile/data/Sheaf/actions.json\")); assert a[\"outputFileName\"] == \"ready-{filename}-Naming review\"; assert any(x[\"output\"][\"prefix\"] == \"ready-\" and x[\"output\"][\"suffix\"] == \"-Naming review\" for x in b)'"
quit

# 351. Acrobat's suggested action becomes an independent saved copy; the Suggested filter
# keeps the original and Saved actions contains the copy after reopening the app.
launch
batch_actions
# The placeholder is deliberately pale; target the calibrated field after the header is ready.
SH="330 322"; click $SH 0.3
xdotool type --clearmodifiers --delay 10 "confidential watermark"; sleep 0.5
QS=$(ocr 220x400+220+335)
check "351 suggested-action search matches words in its name and description (${SH:-no Search})" "echo \"$QS\" | grep -q 'Confidential' && ! echo \"$QS\" | grep -q 'Distribution'"
CA=$(at Mark 220x400+220+335); [ -n "$CA" ] && click $CA
SA=$(at Save 320x100+450+470); [ -n "$SA" ] && click $SA 1
shot 351-suggested-copy
check "351 Save a copy keeps the suggested watermark configuration (${CA:-no suggestion}; ${SA:-no Save})" "python3 -c 'import json; a=json.load(open(\"$OUT/profile/data/Sheaf/actions.json\")); assert any(x[\"name\"] == \"Mark as Confidential (2)\" and x[\"steps\"][0][\"argument\"] == \"CONFIDENTIAL\" for x in a)'"
# A populated search field no longer shows its placeholder; reuse its observed position.
# A suggestion's full limitations stay readable, rather than hidden in a one-line description.
[ -n "$SH" ] && click $SH 0.3
key ctrl+a; xdotool type --clearmodifiers --delay 10 "accessibility"; sleep 0.5
AS=$(at Accessibility 220x400+220+335); [ -n "$AS" ] && click $AS 0.8
DS=$(ocr 790x100+460+375); shot 351-suggestion-description
check "351 suggestion description shows its refusal and certification limits (${AS:-no suggestion})" "echo \"$DS\" | grep -q 'Refuses' && echo \"$DS\" | grep -q 'certification'"
quit
launch
batch_actions
# The list filter is the first combobox in the left column; Home then Down twice selects Saved.
click 340 286; key Home; key Down; key Down; key Return
LS=$(ocr 220x400+220+335); shot 351-saved-filter
check "351 Saved actions filter restores the copied suggestion after restart" "echo \"$LS\" | grep -q 'Confidential' && ! echo \"$LS\" | grep -q 'Suggested'"
quit

# 352. Acrobat's custom command applies Keep pages 2-3 to the real four-page reader. The
# command must be enabled, save the two pages, and remain one ordinary undoable reader edit.
python3 - "$OUT/profile/data/Sheaf/actions.json" <<'PY352'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1]); p.parent.mkdir(parents=True, exist_ok=True)
a = json.loads(p.read_text()) if p.exists() else []
a.append({"version":1,"name":"Reader page tools","custom_tool":True,"steps":[{"tool":"keep_pages","argument":"2-3","label":"Keep middle pages"}]})
p.write_text(json.dumps(a))
PY352
cp "$OUT/text.pdf" "$OUT/352-custom.pdf"
launch "$OUT/352-custom.pdf"
# Custom tools: More ▸ More tools… ▸ Custom tools (the toolbar button went with W3).
optimize_print_tool Custom >/dev/null; sleep 0.8
AP=$(at Apply 220x55+345+380); [ -n "$AP" ] && click $AP 1.5
shot 352-custom-pages
key ctrl+s; sleep 1.5
P2=$(qpdf --show-npages "$OUT/352-custom.pdf" 2>/dev/null)
T2=$(first_text "$OUT/352-custom.pdf" 1); T3=$(first_text "$OUT/352-custom.pdf" 2)
key ctrl+z; sleep 1; key ctrl+s; sleep 1.5
P4=$(qpdf --show-npages "$OUT/352-custom.pdf" 2>/dev/null)
shot 352-custom-undone
check "352 custom page-range command applies to the open PDF and undoes (${AP:-no Apply}; pages ${P2:-?} then ${P4:-?})" "[ \"$P2\" = 2 ] && [ \"$P4\" = 4 ] && [ \"$T2\" = \"Page 2\" ] && [ \"$T3\" = \"Page 3\" ]"
quit
# The custom command goes again: while one exists the reader's toolbar gains Custom tools at its
# left, which moves every later step's toolbar clicks 32 px (AI steps 364–367 failed so, 28/10).
python3 - "$OUT/profile/data/Sheaf/actions.json" <<'PY352B'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
if p.exists(): p.write_text(json.dumps([a for a in json.loads(p.read_text()) if not a.get("custom_tool")]))
PY352B

# 353. Acrobat's Specify Settings: edit labelled watermark fields, preserve the next step and
# its prompt, save, run, and inspect real output. The serialized text includes a literal pipe.
python3 - "$OUT/353-wizard.json" <<'PY353'
import json, sys
json.dump({"version":1,"name":"Wizard settings","steps":[
    {"tool":"watermark_options","argument":"36|0.5|45|0|0|Helvetica|all|Before","prompt":True,"instruction":"Review each output"},
    {"tool":"set_title","argument":"Wizard result"}],"output":{"same_folder":True,"suffix":"-353"}}, open(sys.argv[1], "w"))
PY353
cp "$OUT/text.pdf" "$OUT/353-source.pdf"
launch
batch_actions
answer "$PWD/$OUT/353-wizard.json"
IM=$(at Import 140x60+220+180); [ -n "$IM" ] && click $IM
ED=$(at Edit 200x250+1040+450); [ -n "$ED" ] && click $ED
# Labelled fields can be below the editor's fold; scroll only that editor until the label is visible.
batch_label() {
    local point
    for _ in $(seq 1 30); do
        point=$(at "$1" 790x360+460+290)
        if [ -n "$point" ]; then echo "$point"; return; fi
        xdotool mousemove 1245 650 click 5; sleep 0.15
    done
}
batch_field() {
    local bx by
    read bx by <<< "$(batch_label "$1")"
    [ -n "$by" ] || return 1
    click "$bx" $(( by + 32 )) 0.2
    key ctrl+a; xdotool type --clearmodifiers --delay 5 -- "$2"; sleep 0.3
}
batch_field Mark "After | review"
batch_field Font "24"
batch_field Pages "2-3"
batch_field Opacity "0.35"
batch_field Rotation "-30"
shot 353-labelled-settings
xdotool mousemove 1245 450; for _ in $(seq 1 50); do xdotool click 4; done; sleep 0.6
SA=$(at Save 320x100+450+470); [ -n "$SA" ] && click $SA 1
check "353 labelled mark controls preserve text, range, appearance, order and per-step instructions" "python3 -c 'import json; a=next(x for x in json.load(open(\"$OUT/profile/data/Sheaf/actions.json\")) if x[\"name\"] == \"Wizard settings\"); assert a[\"steps\"][0][\"argument\"] == \"24|0.35|-30|0|0|Helvetica|2-3|After | review\"; assert a[\"steps\"][0][\"prompt\"] and a[\"steps\"][0][\"instruction\"] == \"Review each output\"; assert a[\"steps\"][1][\"tool\"] == \"set_title\"'"
xdotool mousemove 1245 650; for _ in $(seq 1 65); do xdotool click 5; done; sleep 0.8
answer "$PWD/$OUT/353-source.pdf"
RN=$(at Run 350x120+900+60); [ -n "$RN" ] && click $RN 1
CT=$(at Continue 600x450+340+200); [ -n "$CT" ] && click $CT 2
for _ in $(seq 1 20); do [ -f "$OUT/353-source-353.pdf" ] && break; sleep 0.5; done
shot 353-wizard-output
check "353 saved multistep action prompts then marks only selected pages (${CT:-no Continue})" "pdfinfo \"$OUT/353-source-353.pdf\" 2>/dev/null | grep -q '^Title:.*Wizard result' && pdftotext -f 2 -l 3 \"$OUT/353-source-353.pdf\" - | tr -d '[:space:]' | grep -q 'After|review' && ! pdftotext -f 1 -l 1 \"$OUT/353-source-353.pdf\" - | tr -d '[:space:]' | grep -q 'After|review'"
quit
# 354. Acrobat's saved-action run continues after a damaged PDF or missing file, keeps successful
# results, and lists each failure separately rather than growing an unbounded status paragraph.
python3 - "$OUT/354-run.json" <<'PY354'
import json, sys
json.dump({"version":1,"name":"Mixed input run","steps":[{"tool":"set_title","argument":"Batch success"}],
           "output":{"same_folder":True,"suffix":"-354"}}, open(sys.argv[1], "w"))
PY354
cp "$OUT/text.pdf" "$OUT/354-valid.pdf"
printf 'Not a PDF' > "$OUT/354-broken.pdf"
launch
batch_actions
answer "$PWD/$OUT/354-run.json"
IM=$(at Import 140x60+220+180); [ -n "$IM" ] && click $IM
# Run is at the end of the right editor's scroll area.
xdotool mousemove 1245 650; for _ in $(seq 1 35); do xdotool click 5; done; sleep 1
answer "$PWD/$OUT/354-valid.pdf|$PWD/$OUT/354-broken.pdf|$PWD/$OUT/354-missing.pdf"
RN=$(at Run 350x120+900+60); [ -n "$RN" ] && click $RN 2
for _ in $(seq 1 20); do [ -f "$OUT/354-valid-354.pdf" ] && break; sleep 0.5; done
sleep 1; shot 354-run-report
RS=$(ocr 1060x240+210+550)
check "354 saved action reports one output and both per-file failures (${RN:-no Run})" "[ -f \"$OUT/354-valid-354.pdf\" ] && echo \"$RS\" | grep -q '1 of 3' && echo \"$RS\" | grep -q '2 failed' && echo \"$RS\" | grep -q '354-broken' && echo \"$RS\" | grep -q '354-missing' && cmp -s \"$OUT/text.pdf\" \"$OUT/354-valid.pdf\""
quit

# 355. Stirling's Folder Scanning resumes a saved recursive watch on startup, names outputs,
# checkpoints handled revisions and does not duplicate them after restarting the real app.
python3 - "$PWD/$OUT" <<'PY355'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1]).resolve(); inbox = root / "355-in"; out = root / "355-out"
(inbox / "nested").mkdir(parents=True, exist_ok=True); out.mkdir(exist_ok=True)
settings = root / "profile/data/Sheaf"; settings.mkdir(parents=True, exist_ok=True)
action = {"version":1,"name":"Watched titles","steps":[{"tool":"set_title","argument":"Watched result"}]}
watch = {"input":str(inbox),"output":str(out),"seen":{},"title_names":False,"recursive":True,
         "pattern":"{counter}-{name}","archive_inputs":False,"sequence":0}
(settings / "folder-watch.json").write_text(json.dumps({"version":1,"action":action,"watch":watch}))
PY355
launch
cp "$OUT/text.pdf" "$OUT/355-in/nested/upload.pdf"
for _ in $(seq 1 40); do [ -f "$OUT/355-out/1-upload.pdf" ] && break; sleep 0.5; done
check "355 saved recursive watch processes and renames an incoming PDF" "[ -f \"$OUT/355-out/1-upload.pdf\" ] && pdfinfo \"$OUT/355-out/1-upload.pdf\" 2>/dev/null | grep -q '^Title:.*Watched result'"
quit
launch
sleep 5
N1=$(find "$OUT/355-out" -maxdepth 1 -name '*.pdf' | wc -l)
cp "$OUT/text.pdf" "$OUT/355-in/second.pdf"
for _ in $(seq 1 40); do [ -f "$OUT/355-out/2-second.pdf" ] && break; sleep 0.5; done
AC=$(at Actions 210x400+0+30); [ -n "$AC" ] && click $AC
shot 355-watched-restart
check "355 restarted watch skips its handled revision and continues its naming counter ($N1 existing)" "[ \"$N1\" = 1 ] && [ -f \"$OUT/355-out/2-second.pdf\" ] && [ \"\$(find \"$OUT/355-out\" -maxdepth 1 -name '*.pdf' | wc -l)\" = 2 ]"
quit
# Do not resume this smoke's watch in later smoke steps.
python3 - "$OUT/profile/data/Sheaf" <<'PY355C'
import pathlib, sys
for p in pathlib.Path(sys.argv[1]).glob("folder-watch*.json"): p.unlink()
PY355C

# 356. Acrobat's folder input is gathered off the UI thread, then the selected recursive PDFs
# run through the saved action. Each original stays intact beside its output copy.
python3 - "$OUT/356-folder-action.json" <<'PY356'
import json, sys
json.dump({"version":1,"name":"Folder run","steps":[{"tool":"set_title","argument":"Folder result"}],
           "output":{"same_folder":True,"suffix":"-356"}}, open(sys.argv[1], "w"))
PY356
mkdir -p "$OUT/356-input/nested"
cp "$OUT/text.pdf" "$OUT/356-input/first.pdf"; cp "$OUT/text.pdf" "$OUT/356-input/nested/second.pdf"
launch
batch_actions
answer "$PWD/$OUT/356-folder-action.json"
IM=$(at Import 140x60+220+180); [ -n "$IM" ] && click $IM
xdotool mousemove 1245 650; for _ in $(seq 1 45); do xdotool click 5; done; sleep 0.8
IN=$(at Include 790x440+460+290); [ -n "$IN" ] && click $IN 0.3
answer "$PWD/$OUT/356-input"
RF=$(at Run 740x80+500+640); [ -n "$RF" ] && click $RF 2
for _ in $(seq 1 20); do [ -f "$OUT/356-input/nested/second-356.pdf" ] && break; sleep 0.5; done
shot 356-folder-results
check "356 recursive folder intake processes both PDFs into new copies (${IN:-no Include}; ${RF:-no Run})" "pdfinfo \"$OUT/356-input/first-356.pdf\" 2>/dev/null | grep -q '^Title:.*Folder result' && pdfinfo \"$OUT/356-input/nested/second-356.pdf\" 2>/dev/null | grep -q '^Title:.*Folder result' && cmp -s \"$OUT/text.pdf\" \"$OUT/356-input/first.pdf\" && cmp -s \"$OUT/text.pdf\" \"$OUT/356-input/nested/second.pdf\""
# 330. Acrobat's Create PDF from Scanner through the real process adapter: the fake SANE driver
# returns pixels, not SHEAF_TEST_SCAN's image-copy seam. Also used by the macOS SANE fallback.
actions_reset
scanner_driver() {
    SCANNER_APP=$APP; SCANNER_PATH=$PATH
    mkdir -p "$OUT/scanner-bin"
    convert -size 80x40 xc:'#2a9d4b' "$OUT/scanner-source.png"
    printf '#!/bin/sh\nprintf "%%s\\n" "$@" > "%s/scanner-args"\ncp "%s/scanner-source.png" "$5"\n' "$PWD/$OUT" "$PWD/$OUT" > "$OUT/scanner-bin/scanimage"
    printf '#!/bin/sh\nunset SHEAF_TEST_SCAN\nexec "%s" "$@"\n' "$APP" > "$OUT/scanner-app"
    chmod +x "$OUT/scanner-bin/scanimage" "$OUT/scanner-app"
    export PATH="$PWD/$OUT/scanner-bin:$PATH"; APP="$PWD/$OUT/scanner-app"
}
scanner_restore() { APP=$SCANNER_APP; export PATH=$SCANNER_PATH; }
scanner_driver
launch; answer "$PWD/$OUT/driver-scan.pdf"; FN=$(at scanner 1000x120+250+80); [ -n "$FN" ] && click $FN 1
for _ in $(seq 1 20); do [ -f "$OUT/driver-scan.pdf" ] && break; sleep 0.5; done; answer ""
SN=$(pdftoppm -r 20 -f 1 -l 1 -png "$OUT/driver-scan.pdf" 2>/dev/null | convert - -format "%[pixel:p{5,5}]" info: 2>/dev/null)
shot 330-scanner-driver
check "330 From scanner invokes SANE at 300 dpi and opens the scanned pixels ($SN)" "[ \"\$(npages $OUT/driver-scan.pdf 2>/dev/null)\" = 1 ] && echo \"$SN\" | grep -q 'srgb(42,15[0-9],7[0-9])' && grep -qx -- '--format=png' $OUT/scanner-args && grep -qx '300' $OUT/scanner-args && [ \"\$(ntabs)\" = 1 ]"
quit; scanner_restore

# 331. Preview's Insert from Scanner through the same process adapter: after page 1, one Undo,
# Redo restores it and the remaining original pages retain their order.
scanner_driver
cp "$OUT/text.pdf" "$OUT/driver-insert.pdf"; launch "$OUT/driver-insert.pdf"; sleep 1
xdotool mousemove 120 230 click 3; sleep 0.8; IS=$(at scanner 300x420+20+200); [ -n "$IS" ] && click $IS 2
# The keys go to the pages: after the thumbnail's menu the keyboard stayed with the panel, and in the
# full suite ⌘S saved nothing (4/4/4 with five pages shown, 28/10). The save is waited for.
click 760 500 0.4
key ctrl+s; for _ in $(seq 1 10); do N1=$(npages "$OUT/driver-insert.pdf" 2>/dev/null); [ "$N1" = 5 ] && break; sleep 0.5; done
SN=$(pdftoppm -r 20 -f 2 -l 2 -png "$OUT/driver-insert.pdf" 2>/dev/null | convert - -format "%[pixel:p{5,5}]" info: 2>/dev/null)
key ctrl+z; sleep 0.8; key ctrl+s; sleep 1; N2=$(npages "$OUT/driver-insert.pdf" 2>/dev/null)
key ctrl+shift+z; sleep 0.8; key ctrl+s; sleep 1; N3=$(npages "$OUT/driver-insert.pdf" 2>/dev/null)
shot 331-scanner-insert
check "331 Insert from scanner preserves pixels, page order and Undo/Redo ($N1/$N2/$N3; $SN)" "[ \"$N1/$N2/$N3\" = 5/4/5 ] && echo \"$SN\" | grep -q 'srgb(42,15[0-9],7[0-9])' && [ \"\$(first_text $OUT/driver-insert.pdf 1)\" = 'Page 1' ] && [ \"\$(first_text $OUT/driver-insert.pdf 3)\" = 'Page 2' ] && [ \"\$(first_text $OUT/driver-insert.pdf 5)\" = 'Page 4' ]"
quit; scanner_restore

# 335. iLovePDF's complete PDF/A selector: a Unicode profile produces the selected identifier,
# not a b-level file with the wrong success label. Pictures contain no unmapped text.
pdfa_reader_tool() {
    # The Tools segment of the mode bar (its place never moves: custom tools live in More since W3).
    click $WRENCH_X 61 1.2; xdotool type --delay 20 "pdfa"; key Return; sleep 2
}
convert -size 120x80 xc:'#2a9d4b' "$OUT/unicode-archive.pdf"
launch "$OUT/unicode-archive.pdf"; sleep 1
pdfa_reader_tool
CB=$(at PDF/A- 1000x700+250+90); [ -n "$CB" ] && click $CB 0.5
shot 335-pdfa-levels
xdotool key --clearmodifiers --delay 150 Up Up Up Up Up Up Up Up Down Down Down Return; sleep 0.8
shot 335-pdfa-unicode-options
click 1140 755 2
for _ in $(seq 1 20); do [ -f "$OUT/unicode-archive-pdfa.pdf" ] && break; sleep 0.5; done
UQ=$(qpdf --qdf --object-streams=disable "$OUT/unicode-archive-pdfa.pdf" - 2>/dev/null)
shot 335-pdfa-unicode-result
check "335 PDF/A-2u selection writes the actual U/2 identifier" "echo \"\$UQ\" | grep -aq 'pdfaid:conformance>U<' && echo \"\$UQ\" | grep -aq 'pdfaid:part>2<' && [ \"\$(npages $OUT/unicode-archive-pdfa.pdf 2>/dev/null)\" = 1 ]"
quit

# 336. An a-level needs tags. With Keep a copy unchecked, an untagged input is refused and no
# falsely conforming output appears; the source stays readable.
# Reproduce the full suite's saved custom toolbox, which shifts the wrench past the AI button.
AF="$OUT/profile/data/Sheaf/actions.json"; [ ! -f "$AF" ] || cp "$AF" "$OUT/pdfa-actions-before.json"
python3 - "$AF" <<'PY'
import json, os, sys
p = sys.argv[1]
a = json.load(open(p)) if os.path.exists(p) else []
a.append({"version": 1, "name": "PDF/A toolbar regression", "custom_tool": True,
          "steps": [{"tool": "open_tool", "argument": "pdf_to_pdfa"}]})
json.dump(a, open(p, "w"))
PY
convert -size 120x80 xc:'#2a9d4b' "$OUT/untagged-archive.pdf"
launch "$OUT/untagged-archive.pdf"; sleep 1
pdfa_reader_tool
CB=$(at PDF/A- 1000x700+250+90); [ -n "$CB" ] && click $CB 0.5
xdotool key --clearmodifiers --delay 150 Up Up Up Up Up Up Up Up Down Down Down Down Return; sleep 0.8
KC=$(at Keep 1000x700+250+90); [ -n "$KC" ] && click $KC 0.6
STRICT=$(ocr 1000x700+250+90)
shot 336-pdfa-strict-options
# A refusal returns to the options with a three-second toast: read it before it expires.
click 1140 755 0.5
shot 336-pdfa-strict-result; AR=$(ocr 1280x800+0+0)
check "336 strict PDF/A-2a refuses untagged content without saving a false archive" "[ -n \"$KC\" ] && echo \"\$STRICT\" | grep -qi 'PDF/A-2a' && echo \"\$STRICT\" | grep -qi 'no output' && [ ! -e $OUT/untagged-archive-pdfa.pdf ] && [ \"\$(npages $OUT/untagged-archive.pdf 2>/dev/null)\" = 1 ] && echo \"\$AR\" | grep -qi 'did not pass'"
quit
if [ -f "$OUT/pdfa-actions-before.json" ]; then mv "$OUT/pdfa-actions-before.json" "$AF"; else rm -f "$AF"; fi

# 305. Right to left on tick boxes (C-platform-004; iLovePDF's mirrored interface): in Report an Issue, the ticked
# box of "Include this computer's details" sits right of its label, left of it left to right.
# The x of the accent (ticked) pixels in the 300 px either side of a word's centre, their mean
# (tick boxes take the accent colour since the BAMA pass, 28/10/2026; the accent is blue by default since W1).
tick_x() { import -window root -crop "600x1+$(( $1 - 300 ))+$2" -depth 8 txt:- 2>/dev/null | python3 -c '
import re, sys
xs = [int(m.group(1)) for m in re.finditer(r"^(\d+),\d+: \(\s*(\d+),\s*(\d+),\s*(\d+)", sys.stdin.read(), re.M) if int(m.group(4)) > 180 and int(m.group(2)) < 100 and int(m.group(3)) < 150]
print(sum(xs) // len(xs) + int(sys.argv[1]) - 300 if xs else "")' "$1"; }
export SHEAF_TEST_MENU_BAR=1
for dir in 1 2; do
    prefs_set "p.setdefault('platform', {})['direction'] = $dir"
    launch; sleep 0.5
    HX=$(at Help 1280x34+0+0); [ -n "$HX" ] && click $HX 0.8
    RI=$(at Report 1280x300+0+24); [ -n "$RI" ] && click $RI 1
    # Read on the dialog's lower half (the whole screen's OCR misses its last rows).
    read IX IY <<< "$(at Include 580x350+350+400)"; GX=; [ -n "$IX" ] && GX=$(tick_x $IX $IY)
    [ $dir = 1 ] && { LX=$IX; LG=$GX; shot 305a-check-ltr; } || { RX=$IX; RG=$GX; shot 305b-check-rtl; }
    quit
done
unset SHEAF_TEST_MENU_BAR
check "tick boxes mirror right to left (left to right: box ${LG:-?} < label ${LX:-?}; right to left: box ${RG:-?} > label ${RX:-?})" "[ -n \"$LG\" ] && [ -n \"$RG\" ] && [ \"$LG\" -lt \"$LX\" ] && [ \"$RG\" -gt \"$RX\" ]"
platform_reset
# 364. The AI panel's error and empty states (C-ai-001, C-ai-002): a provider limiting requests is said plainly with
# Try again (which asks again); a cloud provider with no key is told where to add it, with Open AI settings…; the
# panel then still opens and the app answers (no freeze).
actions_reset
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; launch "$OUT/ai.pdf"; sleep 1
ai_open; click 1040 768 0.4; xdotool type --delay 15 "Rate me"; key Return
ai_wait limiting; E1=$(ai_text); N3=$(ai_requests)
read RX RY <<< "$(ai_last Try)"; [ -n "$RY" ] && click $RX $RY 1.5
N4=$(ai_requests); shot ai-364a
quit; ai_stop
prefs_set "p['ai'] = {'provider': 'openai', 'notice_seen': ['openai']}"
launch "$OUT/ai.pdf"; sleep 1
ai_open; click 1040 768 0.4; xdotool type --delay 15 "Hello"; key Return
ai_wait 'API key'; E2=$(ai_text); shot ai-364b
read SX SY <<< "$(ai_last Open)"; [ -n "$SY" ] && click $SX $SY 1.2
PG=$(ocr 1280x120+0+40); shot ai-364c
check "errors are said plainly with what to do (429: $(echo "$E1" | grep -o 'limiting requests[^.]*' | head -1); Try again $N3 → $N4 requests; no key: $(echo "$E2" | grep -o 'API key in Preferences' | head -1); settings: $(echo "$PG" | grep -o Preferences | head -1))" \
    "echo \"$E1\" | grep -q 'limiting requests' && [ \"$N4\" = \$(( N3 + 1 )) ] && echo \"$E2\" | grep -q 'API key in Preferences' && echo \"$PG\" | grep -q Preferences"
quit; ai_stop

# 365. AI spaces renamed (C-ai-006, Acrobat's Rename space): New space, Rename, "Contracts" typed and Enter: the
# list and ai-spaces.json name it so. (Files dropped on the document while a space is shown join it: not automatable
# here — Xvfb has no drag source for files from outside; the routing is unit-tested, ai_ui drop_space.)
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; launch "$OUT/ai.pdf"; sleep 1
ai_open; NS=$(ai_chip New); sleep 0.8; RN=$(ai_chip Rename); sleep 0.5
key ctrl+a; key BackSpace; xdotool type --delay 15 "Contracts"; key Return; sleep 0.8; T5=$(ai_text); shot ai-365
SN=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))[0]["name"])' "$PWD/$OUT/profile/data/Sheaf/ai-spaces.json" 2>/dev/null)
check "a space is renamed (new $NS, rename $RN; saved '$SN'; list: $(echo "$T5" | grep -o 'Space: [A-Za-z]*' | head -1))" "[ \"$SN\" = Contracts ] && echo \"$T5\" | grep -q 'Space: Contracts'"
quit; ai_stop

# 366. The podcast spoken (C-ai-005, Acrobat's Generate podcast): Podcast script, then Save audio… (the dialog answered
# $OUT/ai-pod.wav) records the two hosts with the computer's voices into one WAV of a few seconds.
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; rm -f "$OUT/ai-pod.wav"; echo "$PWD/$OUT/ai-pod.wav" > "$OUT/dialog-answer"
launch "$OUT/ai.pdf"; sleep 1
ai_open; PC=$(ai_chip Podcast); ai_wait Welcome
read AX AY <<< "$(ai_last audio)"; [ -n "$AY" ] && click $AX $AY 0.5
for _ in $(seq 1 40); do [ -s "$OUT/ai-pod.wav" ] && break; sleep 0.5; done; sleep 0.5; ai_wait 'audio saved'; T6=$(ai_text); shot ai-366
WV=$(python3 -c 'import sys, wave; w = wave.open(sys.argv[1]); print(w.getnchannels(), w.getframerate(), round(w.getnframes() / w.getframerate(), 1))' "$OUT/ai-pod.wav" 2>/dev/null)
check "Save audio… speaks the podcast into a WAV (chip $PC, button ${AY:-none}; wav: ${WV:-none}; $(echo "$T6" | grep -o 'audio saved' | head -1))" "[ -n \"$WV\" ] && python3 -c 'import sys; sys.exit(0 if float(sys.argv[1]) > 2 else 1)' \$(echo $WV | cut -d' ' -f3) && echo \"$T6\" | grep -q 'audio saved'"
quit; rm -f "$OUT/dialog-answer"; ai_stop

# 367. A translation into Japanese saved as a PDF (C-ai-004): Translate (Japanese chosen), Save… as $OUT/ai-ja.pdf:
# the PDF holds the Japanese text, searchable (set in a font that draws it, not refused).
ai_mock seen; prefs_set "p['ai']['language'] = 'Japanese'"; cp "$OUT/text.pdf" "$OUT/ai.pdf"; rm -f "$OUT/ai-ja.pdf"; echo "$PWD/$OUT/ai-ja.pdf" > "$OUT/dialog-answer"
launch "$OUT/ai.pdf"; sleep 1
# Translate: the row above the categories field, found by its "Classify PDFs…" button (the placeholder is too
# light for OCR since W1; the language list's chevron and the chips' outlines garble the row itself).
ai_open; read _ CY <<< "$(at Classify 380x300+900+300)"; [ -n "$CY" ] && click 1076 $(( CY - 44 )) 0.5; TR="${CY:-no Classify}"; sleep 2
read SX SY <<< "$(ai_last Save)"; [ -n "$SY" ] && click $SX $SY 0.5
for _ in $(seq 1 20); do [ -s "$OUT/ai-ja.pdf" ] && break; sleep 0.5; done; shot ai-367
JT=$(pdftotext "$OUT/ai-ja.pdf" - 2>/dev/null | tr -d '\n')
check "a Japanese translation is saved as a searchable PDF (translate $TR, save ${SY:-none}; text: $JT)" "echo \"$JT\" | grep -q '東京の素早い'"
quit; rm -f "$OUT/dialog-answer"; ai_stop

# 306. Preferences ▸ Interface ▸ Language (C-preferences-003, iLovePDF's Language): French chosen, Home reads
# Outils and Préférences in its sidebar and the Tools page Fusionner des PDF (a name made in Rust); Language ▸
# English switches back at once, without restarting.
prefs_set "p.setdefault('platform', {})['language'] = 2"
launch; sleep 0.5
SB=$(ocr 250x300+0+40)
OT=$(at Outils 250x300+0+40); [ -n "$OT" ] && click $OT 0.8
TP=$(ocr 1080x700+0+90); shot 306a-french
key ctrl+comma; sleep 0.8
EN=$(pref_find English); [ -n "$EN" ] && click $EN 0.8
SB2=$(ocr 250x300+0+40); shot 306b-english
LG=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["platform"]["language"])' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
check "the interface switches to French and back (sidebar: $(echo $SB | cut -c1-60); Tools page: $(echo "$TP" | grep -o 'Fusionner[^ ]*' | head -1); then $(echo $SB2 | cut -c1-50); language $LG)" "echo \"$SB\" | grep -q 'Outils' && echo \"$SB\" | grep -q 'Préférences' && echo \"$TP\" | grep -q 'Fusionner' && echo \"$SB2\" | grep -q 'Tools' && echo \"$SB2\" | grep -q 'Preferences' && [ \"$LG\" = 1 ]"
quit; platform_reset

# 368. The AI panel has its own pane (C-ai-001; PDF Expert's Copilot never covers the page): with it open, the
# page's last word "fox" shows left of the panel. An answer's cited page (C-ai-002, Acrobat's citations) is a button:
# p. 3 shows page 3 (its text "Page 3" at the top of the pages).
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; launch "$OUT/ai.pdf"; sleep 1
ai_open; sleep 1; read FX _ <<< "$(at fox 650x700+250+90)"
click 1040 768 0.4; xdotool type --delay 15 "Who jumps?"; key Return; ai_wait Mockanswer; sleep 0.5
# The cite button sits above the answer's Copy and Save… row, at its left.
read _ PY <<< "$(ai_last Copy)"; [ -n "$PY" ] && click 946 $(( PY - 36 )) 1.2; shot ai-368
PF=$(ocr 600x80+260+140 | grep -o 'Page [0-9]' | head -1)
check "the AI pane leaves the page whole and a cited page opens (fox at x ${FX:-none}; cite ${PY:-none}; shown: $PF)" "[ -n \"$FX\" ] && [ \"$FX\" -lt 900 ] && [ \"$PF\" = 'Page 3' ]"
quit; ai_stop

# 369. Highlight main points (C-ai-003, PDF Expert's): Main points, then Highlight in PDF: the point's sentence ("Page 2
# the quick brown fox") gets a real highlight annotation on page 2, the point as its note; saved with ⌘S.
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; launch "$OUT/ai.pdf"; sleep 1
ai_open; ai_chip "Main points" >/dev/null; ai_wait Mockpoint; sleep 0.5
read HX HY <<< "$(ai_last Highlight)"; [ -n "$HY" ] && click $HX $HY 1; key Escape; key ctrl+s; sleep 1.5; shot ai-369
HL="$(qpdf --qdf --object-streams=disable "$OUT/ai.pdf" - 2>/dev/null | grep -ac '/Subtype /Highlight') $(qpdf --qdf --object-streams=disable "$OUT/ai.pdf" - 2>/dev/null | python3 -c 'import re, sys; t = sys.stdin.buffer.read().decode("latin-1"); print(",".join(n for n, body in re.findall(r"%% Page (\d+)\n(.*?)(?=%% Contents|%% Page|\Z)", t, re.S) if "/Annots" in body))') $(qpdf --qdf --object-streams=disable "$OUT/ai.pdf" - 2>/dev/null | grep -aq Mockpoint && echo True)"
check "Highlight in PDF adds a highlight on the point's sentence, the point as its note (highlights, pages, note: $HL)" "[ \"$HL\" = '1 2 True' ]"
quit; ai_stop

# 370. A space shared as a file (C-ai-006, Acrobat's shared spaces): Export space… writes $OUT/share/ai.sheafspace with
# the PDF's path relative to it (../ai.pdf); the space deleted, Import space… brings it back with that PDF.
ai_mock seen; cp "$OUT/text.pdf" "$OUT/ai.pdf"; mkdir -p "$OUT/share"; rm -f "$OUT/share/"*; echo "$PWD/$OUT/share/ai.sheafspace" > "$OUT/dialog-answer"
launch "$OUT/ai.pdf"; sleep 1
ai_open; NS=$(ai_chip New); sleep 0.8; EX=$(ai_chip Export); sleep 1
REL=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["files"])' "$OUT/share/ai.sheafspace" 2>/dev/null)
DL=$(ai_chip Delete); sleep 0.8; IM=$(ai_chip Import); sleep 1.2; T7=$(ai_text); shot ai-370
SP=$(python3 -c 'import json, sys; s = json.load(open(sys.argv[1])); print(len(s), s[-1]["files"][-1].endswith("/ai.pdf"))' "$PWD/$OUT/profile/data/Sheaf/ai-spaces.json" 2>/dev/null)
check "a space exported with relative paths is imported back (new $NS, export $EX, delete $DL, import $IM; file lists $REL; spaces $SP; $(echo "$T7" | grep -o 'Space: Space ([0-9] PDF)' | head -1))" "[ \"$REL\" = \"['../ai.pdf']\" ] && [ \"$SP\" = '1 True' ] && echo \"$T7\" | grep -q 'Space: Space (1 PDF)'"
quit; rm -f "$OUT/dialog-answer"; ai_stop

# 371. Preferences ▸ Signatures ▸ Add a smart card or token… (C-sign-013, Acrobat's PKCS#11 Modules and Tokens ▸
# Attach Module): a SoftHSM token holding a P-256 key and its certificate, as a smart card does; its module chosen,
# the card's ID joins the list without the PIN being asked, named after its holder and token, and is kept as a
# pkcs11: reference (it signs with the PIN in 372).
SH=$(for m in /usr/lib/softhsm/libsofthsm2.so /usr/lib/x86_64-linux-gnu/softhsm/libsofthsm2.so /opt/homebrew/lib/softhsm/libsofthsm2.so; do [ -f "$m" ] && echo "$m" && break; done)
TK="$PWD/$OUT/token"; rm -rf "$TK"; mkdir -p "$TK/tokens"
printf 'directories.tokendir = %s\nobjectstore.backend = file\nlog.level = ERROR\n' "$TK/tokens" > "$TK/softhsm2.conf"
# The app inherits it from launch: SoftHSM reads its tokens there, never the user's.
export SOFTHSM2_CONF="$TK/softhsm2.conf"
if [ -n "$SH" ] && command -v pkcs11-tool >/dev/null; then
    softhsm2-util --init-token --free --label "Smoke card" --so-pin 123456 --pin 4321 >/dev/null 2>&1
    openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$TK/card.key" 2>/dev/null
    openssl req -new -x509 -key "$TK/card.key" -days 2 -subj "/CN=Smoke Card Holder" -addext keyUsage=critical,digitalSignature,nonRepudiation -outform DER -out "$TK/card.der" 2>/dev/null
    softhsm2-util --import "$TK/card.key" --token "Smoke card" --label holder --id 01 --pin 4321 >/dev/null 2>&1
    pkcs11-tool --module "$SH" --token-label "Smoke card" --login --pin 4321 --write-object "$TK/card.der" --type cert --id 01 --label holder >/dev/null 2>&1
fi
launch; key ctrl+comma; sleep 0.8
answer "$SH"; read SX SY <<< "$(pref_find smart)"; [ -n "$SY" ] && click $SX $SY 1
for _ in $(seq 1 20); do grep -q pkcs11: "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null && break; sleep 0.5; done; answer ""; sleep 0.8
shot 371-token
# The list starts at x 234, left of pref_find's crop.
LIST=$(ocr 1040x710+226+90)
REF=$(python3 -c 'import json, sys; print(next((d for d in json.load(open(sys.argv[1])).get("digital_ids", []) if d.startswith("pkcs11:")), ""))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
check "Add a smart card or token lists the card's digital ID without its PIN (module ${SH:-none}; button ${SY:-not found}; listed: $(echo "$LIST" | grep -o 'Smoke Card Holder ([^)]*)' | head -1); kept: $(echo "$REF" | cut -c1-60))" "echo \"$LIST\" | grep -q 'Smoke Card Holder (Smoke card)' && echo \"$REF\" | grep -q '^pkcs11:token=Smoke%20card;.*module-path='"
quit

# 372. Sign with the smart card (C-sign-013): Tools ▸ Sign, a typed name, Use a digital ID; the card's ID chosen from
# the IDs Preferences keeps (it has no file), its PIN typed in a field that asks for a PIN: the signed copy carries
# a digital signature by Smoke Card Holder, made on the token.
cp "$OUT/text.pdf" "$OUT/cardsign.pdf"; rm -f "$OUT"/cardsign-*.pdf
launch "$OUT/cardsign.pdf"; sleep 1
click $WRENCH_X 61 1.5; xdotool type --delay 20 "sign"; key Return; sleep 2
# The panel (x 1004–1252): a typed name, placed with a click on the page, then the digital ID below.
click 1127 281 0.4; xdotool type --delay 20 "Ada"; sleep 0.5; click 500 400 1
# "Use a digital ID…" (its 13 px "Use" OCRs as "use"): the row clicked on "digital".
UD=$(at digital 280x700+1000+90); [ -n "$UD" ] && click $UD 0.8
CH=$(at Smoke 280x700+1000+90); [ -n "$CH" ] && click $CH 0.6
# The PIN field is below the panel's fold.
xdotool mousemove 1130 500 click 5 click 5 click 5 click 5; sleep 0.6
PN=$(at PIN 280x700+1000+90); [ -n "$PN" ] && { click $(echo $PN | awk '{ print $1 + 40, $2 + 30 }') 0.4; xdotool type --delay 20 4321; }
shot 372-panel; PT=$(ocr 280x700+1000+90)
click 1133 755 6; shot 372-signed
SG=$(ls "$OUT"/cardsign-*.pdf 2>/dev/null | head -1); SIG=$( [ -n "$SG" ] && pdfsig -nocert "$SG" 2>&1 | tr '\n' ' ')
check "the card's ID signs with its PIN (use ${UD:-?}, card ${CH:-?}, PIN field ${PN:-?}: $(echo "$PT" | grep -o 'PIN of the smart card' | head -1); ${SG:-no file}: $(echo "$SIG" | grep -o 'Common Name: [A-Za-z ]*Holder\|Signature is [A-Za-z]*' | tr '\n' ' '))" "echo \"$PT\" | grep -q 'PIN of the smart card' && echo \"$SIG\" | grep -q 'Common Name: Smoke Card Holder' && echo \"$SIG\" | grep -q 'Signature is Valid'"
quit
# 307. Plugins (C-platform-006; Acrobat's plug-ins, Stirling's custom scripts): the sample plugin in the plugins
# folder adds "Mark as reviewed" to the Tools page; its first run asks first ("Run the plugin …?"), Allow and run
# writes "plug - Mark as reviewed.pdf" titled Reviewed by the plugin's own process; the version is kept as allowed;
# Preferences ▸ Plugins lists it and Off takes its card away.
mkdir -p "$OUT/profile/data/Sheaf/plugins"; rm -rf "$OUT/profile/data/Sheaf/plugins/reviewed-stamp"
cp -r dev/plugins/reviewed-stamp "$OUT/profile/data/Sheaf/plugins/"; cp "$OUT/text.pdf" "$OUT/plug.pdf"; rm -f "$OUT/plug - Mark as reviewed.pdf"
launch; sleep 0.5
TL=$(at Tools 250x300+0+40); [ -n "$TL" ] && click $TL 0.8
# The Plugins section is the page's last: scrolled until its card shows (its name starts left of x 250).
MK=; for _ in $(seq 1 25); do MK=$(at Mark 1080x710+200+90); [ -n "$MK" ] && break; xdotool mousemove 700 500 click 5 click 5 click 5; sleep 0.3; done
[ -n "$MK" ] && click $MK 1
WQ=$(ocr 700x340+290+230); shot 307a-plugin-warning
answer "$PWD/$OUT/plug.pdf"
# Allow and run, found by its word on the dialog's lower part (460 wide, centred; the body's "Allow it only…"
# sits above y 400), else right of Don’t run, their centres 135 px apart.
read AX AY <<< "$(at Allow 460x160+410+400)"; AL=; [ -n "$AX" ] && AL="$AX $AY"
[ -z "$AL" ] && { read DX DY <<< "$(at Don 1280x800+0+0)"; [ -n "$DX" ] && AL="$(( DX + 135 )) $DY"; }
[ -n "$AL" ] && click $AL 1
for _ in $(seq 1 40); do [ -s "$OUT/plug - Mark as reviewed.pdf" ] && break; sleep 0.25; done; sleep 0.5; answer ""
PT=$(pdfinfo "$OUT/plug - Mark as reviewed.pdf" 2>/dev/null | awk '/^Title:/ { print $2 }')
PA=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["allowed"])' "$PWD/$OUT/profile/data/Sheaf/plugins/plugins.json" 2>/dev/null)
key ctrl+comma; sleep 0.8
# The plugin's row is the page's last: once found, the page goes on to its end so its buttons show too.
PR=$(pref_find "Reviewed"); xdotool mousemove 700 500 click 5 click 5 click 5 click 5 click 5; sleep 0.8
PR=$(pref_find "Reviewed"); OF=; [ -n "$PR" ] && { read _ RY <<< "$PR"; OF=$(at Off 1020x200+250+$(( RY - 10 ))); [ -n "$OF" ] || OF=$(at off 1020x200+250+$(( RY - 10 ))); }   # its 13 px label may OCR "off"
[ -n "$OF" ] && click $OF 0.8; shot 307b-plugins-prefs
PD=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["disabled"])' "$PWD/$OUT/profile/data/Sheaf/plugins/plugins.json" 2>/dev/null)
TL=$(at Tools 250x300+0+40); [ -n "$TL" ] && click $TL 0.8
MK2=; for _ in $(seq 1 25); do MK2=$(at Mark 1080x710+200+90); [ -n "$MK2" ] && break; xdotool mousemove 700 500 click 5 click 5 click 5; sleep 0.2; done
check "a plugin's tool runs as its own process after a warning, and Off removes it (card ${MK:-none}; asked: $(echo "$WQ" | grep -o 'Run the plugin' | head -1); title $PT; allowed $PA; prefs row ${PR:-none}, Off ${OF:-none}: disabled $PD; card after: ${MK2:-gone})" "[ -n \"$MK\" ] && echo \"$WQ\" | grep -q 'Run the plugin' && [ \"$PT\" = Reviewed ] && echo \"$PA\" | grep -q 'reviewed-stamp@1.0' && echo \"$PD\" | grep -q 'reviewed-stamp' && [ -z \"$MK2\" ]"
quit; rm -rf "$OUT/profile/data/Sheaf/plugins"

# 308. Preferences ▸ Rendering ▸ Use the system's profile (C-preferences-005, Acrobat's Use display profile): with a
# Display P3 profile set on the display (the root window's _ICC_PROFILE, as colord sets it), the red of image.pdf
# is drawn converted (less red, some green) while white stays white; sRGB before.
# The page's reddest pixel and a white one (a 10× smaller copy of the page area), as "x y r g b" lines.
red_white() { import -window root -crop 1020x700+250+90 png:- 2>/dev/null | convert - -scale 102x70! -depth 8 txt:- | python3 -c '
import re, sys
px = [tuple(map(int, m.groups())) for m in re.finditer(r"^(\d+),(\d+): \(\s*(\d+),\s*(\d+),\s*(\d+)", sys.stdin.read(), re.M)]
red = max(px, key=lambda p: p[2] - p[4] - p[3] // 2)
white = next(p for p in px if min(p[2:]) > 250)
print(*red, *white)'; }
cp "$OUT/image.pdf" "$OUT/p3.pdf"; launch "$OUT/p3.pdf"; sleep 1; read RX RY R0 G0 B0 WX WY W0 _ <<< "$(red_white)"; quit
python3 dev/scripts/smoke/set-icc.py dev/scripts/smoke/display-p3.icc
launch; key ctrl+comma; sleep 0.8
# Use the system's profile, the third of the row whose first is Choose monitor ICC… (segments sized to their
# labels): found by its word "system's" (not "system": Appearance's bold "System" can OCR so).
US=$(pref_find "system's"); [ -n "$US" ] && click $US 0.8
DS=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("display_system"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
quit
launch "$OUT/p3.pdf"; sleep 1.5; shot 308-display-p3
P=$(import -window root -crop 1020x700+250+90 png:- 2>/dev/null | convert - -scale 102x70! -depth 8 txt:- | python3 -c '
import re, sys
px = {(int(m[1]), int(m[2])): (int(m[3]), int(m[4]), int(m[5])) for m in re.finditer(r"^(\d+),(\d+): \(\s*(\d+),\s*(\d+),\s*(\d+)", sys.stdin.read(), re.M)}
print(*px.get((int(sys.argv[1]), int(sys.argv[2])), (0, 0, 0)), *px.get((int(sys.argv[3]), int(sys.argv[4])), (0, 0, 0)))' "${RX:-0}" "${RY:-0}" "${WX:-0}" "${WY:-0}")
read R1 G1 B1 W1 _ <<< "$P"
python3 dev/scripts/smoke/set-icc.py
check "the system's display profile converts the page's colours (display_system $DS; red at ${RX:-?},${RY:-?}: $R0 $G0 $B0 → $R1 $G1 $B1; white $W0 → $W1)" "[ \"$DS\" = True ] && [ \"${R0:-0}\" -gt 200 ] && [ \$(( ${R0:-0} - ${R1:-0} )) -ge 8 ] && [ \$(( ${G1:-0} - ${G0:-0} )) -ge 8 ] && [ \"${W1:-0}\" -ge 250 ]"
quit; prefs_set "p.pop('display_system', None)"
