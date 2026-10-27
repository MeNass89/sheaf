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
MT=$(at Move 1280x400+0+0); [ -n "$MT" ] && click $MT 1.5
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

# 83. A text box typed in place, then another tool picked in the toolbar (the Pen, 234,61): the text is added, not
# thrown away (Sol, 26/10).
cp "$OUT/text.pdf" "$OUT/switch.pdf"; launch "$OUT/switch.pdf"; sleep 1
key t; sleep 0.5; click 700 500 0.8; xdotool type --delay 20 "Kept on switch"; click 234 61 1; key ctrl+s; sleep 1.5
check "text typed in place is kept when another tool is picked" "qpdf --qdf --object-streams=disable $OUT/switch.pdf - 2>/dev/null | grep -aq 'Kept on switch'"
quit

# 84. Shapes (R) ▸ "- -" (Acrobat's Dashed line style): a rectangle drawn is saved dashed (/BS /S /D).
cp "$OUT/text.pdf" "$OUT/dashed.pdf"; launch "$OUT/dashed.pdf"; sleep 1; key r; sleep 1
# "- -" sits just left of the opacity's "100 %" (tesseract reads it merged with the width dots before it);
# the Shapes options take two rows.
OP=$(at 100 1280x90+0+90); DD=$([ -n "$OP" ] && set -- $OP && echo "$(( $1 - 46 )) $2"); [ -n "$DD" ] && click $DD 0.4
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
answer "$PWD/$OUT/stamp-text.pdf"; click 1127 61 0.8; click 1008 659 0.8; OV=$(at Overlay 300x400+880+80); [ -n "$OV" ] && click $OV 1.5; answer ""; key ctrl+s; sleep 1.5
O2=$(pdftotext -f 2 -l 2 "$OUT/overlay.pdf" - 2>/dev/null | tr '\n' ' '); S1=$(pdftotext -f 1 -l 1 "$OUT/stamp-text.pdf" - 2>/dev/null | tr '\n' ' ' | awk '{print $1, $2}')
check "Overlay a PDF draws its page over each page (${OV:-no item}; overlay: ${S1:-none}; page 2: $(echo "$O2" | cut -c1-60))" "[ \"$S1\" = 'Overlay mark' ] && echo \"$O2\" | grep -q 'Page 2 the quick' && echo \"$O2\" | grep -qF \"$S1\""
quit

# 93. A tool's job done (the test seam announces every one; a real run, those past 10 s): a system notification
# names the tool and the file made (Organize ▸ Odd, then even, step 85).
check "a finished job sends a system notification ($(grep -m1 Organize $OUT/notified 2>/dev/null))" "grep -q 'Organize PDF finished: oddeven-organized.pdf is ready' $OUT/notified"

# 94. More ▸ Read aloud (Acrobat's Read Out Loud): from the page shown to the end, each page's text spoken (the test
# seam writes what the voice is given).
cp "$OUT/text.pdf" "$OUT/aloud.pdf"; launch "$OUT/aloud.pdf"; sleep 1
click 1127 61 0.8; click 1008 659 0.8; RD=$(at Read 300x400+880+80); [ -n "$RD" ] && click $RD 3
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
answer "$PWD/$OUT/flat-copy.pdf"; click 1127 61 0.8; click 1008 659 0.8; FC=$(at flattened 300x400+880+80); [ -n "$FC" ] && click $FC 2; answer ""
NF=$(qpdf --json=2 --json-key=acroform "$OUT/flat-copy.pdf" 2>/dev/null | python3 -c "import json,sys;print(len(json.load(sys.stdin)['acroform']['fields']))" 2>/dev/null)
check "Save a flattened copy draws the value in and leaves no field (${FC:-no item}; ${NF:-?} fields; $(pdftotext $OUT/flat-copy.pdf - 2>/dev/null | tr '\n' ' ' | cut -c1-40))" "[ \"\${NF:-1}\" = 0 ] && pdftotext $OUT/flat-copy.pdf - | grep -q 'Drawn in' && [ \"\$(qpdf --json=2 --json-key=acroform $OUT/toflat.pdf | python3 -c 'import json,sys;print(len(json.load(sys.stdin)[\"acroform\"][\"fields\"]))')\" -ge 1 ]"
quit

# 97. More ▸ Loupe (Acrobat's Loupe tool): the pointer over "Page 1", the panel in the view's bottom-right corner
# shows those words three times larger.
cp "$OUT/text.pdf" "$OUT/loupe.pdf"; launch "$OUT/loupe.pdf"; sleep 1
LW=$(at Page 1020x300+255+80); click 1127 61 0.8; click 1008 659 0.8; LP=$(at Loupe 300x400+880+80); [ -n "$LP" ] && click $LP 0.8
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
click 1127 61 0.8; click 1008 267 1.5
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
click 1127 61 0.8; click 1008 659 0.8; RI=$(at Remove 300x400+880+80); [ -n "$RI" ] && click $RI 1.5; key ctrl+s; sleep 1.5
NI=$(pdfimages -list "$OUT/noimages.pdf" 2>/dev/null | tail -n +3 | wc -l)
check "Remove all images takes every picture out (${RI:-no item}; $NI left)" "[ \"$NI\" = 0 ]"
quit

# 106. More ▸ More tools ▸ Reflow (Acrobat's): the page's text across the window, larger with A+.
cp "$OUT/text.pdf" "$OUT/reflow.pdf"; launch "$OUT/reflow.pdf"; sleep 1
click 1127 61 0.8; click 1008 659 0.8; RF=$(at Reflow 300x400+880+80); [ -n "$RF" ] && click $RF 1; RT=$(ocr 1020x300+255+90)
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
click 1127 61 0.8; click 1008 659 0.8; AK=$(at Accessibility 300x500+880+80); [ -n "$AK" ] && click $AK 3
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
click 1127 61 0.8; click 1008 659 0.8; CF=$(at Copy 300x700+880+80); [ -n "$CF" ] && click $CF 1.5
SF=$(xclip -o -selection clipboard -t x-special/gnome-copied-files 2>/dev/null | sed -n '1{/^copy$/!q};2{s#^file://##;s/%20/ /g;p}')
check "Copy the file puts the document as it is now on the clipboard (${CF:-no item}; $(basename "$SF"))" "[ -f \"$SF\" ] && qpdf --qdf --object-streams=disable \"$SF\" - 2>/dev/null | grep -aq '/Subtype /Highlight' && ! qpdf --qdf --object-streams=disable \"$OUT/share.pdf\" - 2>/dev/null | grep -aq '/Subtype /Highlight'"
quit

# 111. More ▸ More tools ▸ Rulers (Acrobat's): rulers along the page's top and left edge (#F2F2F2 at
# 90 % over the white page, #F3F3F3); a click on the top ruler makes an upright guide (#00A6D6) across
# the page, a second click on its mark takes it away.
cp "$OUT/text.pdf" "$OUT/rulers.pdf"; launch "$OUT/rulers.pdf"; sleep 1
click 1127 61 0.8; click 1008 659 0.8; RU=$(at Rulers 300x700+880+80); [ -n "$RU" ] && click $RU 1.5
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
layout_item() { click 1127 61 0.8; click 1008 659 0.8; click 1008 660 0.8; local i; i=$(at "$1" 340x300+840+70); [ -n "$i" ] && click $i 1.2; echo "${i:-none}"; }
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
# (the whole strip with its red page and buttons does not OCR).
chip() { import -window root -crop "100x34+$1+88" png:- 2>/dev/null | convert - +repage -resize 300% png:- | tesseract stdin stdout --psm 7 2>/dev/null | tr '\n' ' '; }
D0=$(c8 1020); N0=$(chip 767)
click 1020 420 1.5
D1=$(c8 1020); L1=$(c8 510); N1=$(chip 250)
check "split with another document, then its tab forward with this one beside it ($WI; $D0 '$(echo "$N0" | cut -c1-20)' → $L1 $D1 '$(echo "$N1" | cut -c1-20)')" "reddish '${D0:-#000000}' && echo \"$N0\" | grep -q 'split3' && reddish '${D1:-#000000}' && ! reddish '${L1:-#FF0000}' && echo \"$N1\" | grep -q 'split4'"
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
click $(( 264 + 120 * 1000 / 612 )) $(( 100 + (792 - 580) * 1000 / 612 )) 1; shot 210-picked
# The card in the view's top-right corner: "To front" is its first button (OCR may join its two words).
TF=$(at To 680x120+600+90); [ -n "$TF" ] || TF=$(at front 680x120+600+90); [ -n "$TF" ] && click $TF 1.5; shot 210-front; key ctrl+s; sleep 1.5
O1=$(pixel "$OUT/order.pdf" 175 525)
check "Order ▸ To front brings the picked square over the other (${TF:-no To front button}; overlap $O0 → $O1)" "[ \"$O0\" = '0 0 255' ] && [ \"$O1\" = '255 0 0' ]"
quit

# 211. Edit ▸ a paragraph open ▸ U and • (Acrobat's Underline and Bulleted list) apply to it, not only to new text:
# applied (⌘↩) and saved, the page draws a line under it and the paragraph starts with a bullet. U and • follow
# Mono in the row (U 113, • 147 px right of Mono, as step 2c's places).
# Lines (… l, then S) the file draws: counted before and after.
strokes() { qpdf --qdf --object-streams=disable "$1" - 2>/dev/null | grep -acE '[0-9.-]+ [0-9.-]+ l( S)?$'; }
cp "$OUT/text.pdf" "$OUT/underline.pdf"; UL0=$(strokes "$OUT/underline.pdf"); launch "$OUT/underline.pdf"; key ctrl+9; sleep 1
key e; sleep 1; QK=$(at quick 1020x700+255+90); [ -n "$QK" ] && click $QK 1.2
MX=$(at Mono 1280x40+0+90 | cut -d' ' -f1); MX=${MX:-421}
click $(( MX + 113 )) 110 0.4; click $(( MX + 147 )) 110 0.4; key ctrl+Return; sleep 1.5; key Escape; key v; key ctrl+s; sleep 1.5
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
LP=$(at Loupe 1280x40+0+90); [ -n "$LP" ] && click $LP 0.5; drag 500 300 120 120
MK=$(at Mask 1280x40+0+90); [ -n "$MK" ] && click $MK 0.5; drag 450 500 200 100
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
# Preferences ▸ Keyboard shortcuts' search field (its grey placeholder does not OCR): 93 px above Reset all,
# the section's last row; then what is typed is searched.
# SR keeps Reset all's place before the search: the rows found then start 45 px below the field,
# 42 px apart (row 1 at SR - 48, row 2 at SR - 6).
shortcut_search() { local r; read _ r <<< "$(pref_find Reset)"; SR=${r:-0}; [ -n "$r" ] && click 500 $(( r - 93 )) 0.5; xdotool type --delay 20 "$1"; sleep 0.8; }
# A row of the shortcuts list found by a word of its name: its keys box (right-aligned, x 1157) clicked.
keybox() { local y; read _ y <<< "$(at "$1" 1020x700+250+90)"; [ -n "$y" ] && click 1157 "$y" 0.5; echo "${y:-no $1 row}"; }
# The preferences file, written even when no earlier step made it.
prefs_set() { python3 -c 'import json, os, sys; f = sys.argv[1]; p = json.load(open(f)) if os.path.exists(f) else {}; exec(sys.argv[2]); os.makedirs(os.path.dirname(f), exist_ok=True); json.dump(p, open(f, "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" "$1"; }
# The steps below change the platform preferences: put back as they were.
platform_reset() { python3 -c 'import json, sys; p = json.load(open(sys.argv[1])); p.pop("platform", None); json.dump(p, open(sys.argv[1], "w"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null; }

# 221. Preferences ▸ Keyboard shortcuts (C-preferences-013, Stirling's Hotkeys): "find" searched, Find's Ctrl+F
# clicked and Ctrl+Alt+F pressed: kept in the preferences; Find Next given Ctrl+S is refused (Save has it).
launch; key ctrl+comma; sleep 0.8
shortcut_search "find"
# The rows' places come from the search field's (OCR found "Find…" and "Next" a row off while the
# list was still laying out): Find… first, Find Next second.
KF=$(( SR - 48 )); KG=$(( SR - 6 ))
[ "$SR" -gt 48 ] && click 1157 $KF 0.5; sleep 0.5; key ctrl+alt+f; sleep 1
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
SP=$(ocr 1280x710+0+90)
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
KH=$(keybox Highlight); key q; sleep 0.8
TK=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("platform", {}).get("tool_keys"))' "$PWD/$OUT/profile/data/Sheaf/prefs.json" 2>/dev/null)
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
# Change All, the button before Change (the word All is only on it).
CH=$(at All 1020x700+250+90); [ -n "$CH" ] && click $CH 1.2; shot 227-after-change
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
JR=$(at Run 700x500+290+100); [ -n "$JR" ] && click $JR 2
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
LA=$(at Add 1000x160+220+35); [ -n "$LA" ] && click $LA 2
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
PW=$(at Save 700x44+295+504); [ -n "$PW" ] && click $PW 3
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
