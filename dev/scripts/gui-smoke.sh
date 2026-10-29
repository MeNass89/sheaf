#!/bin/bash
# GUI smoke test of the reader on Linux (Xvfb + xdotool), checked on the files it writes, not on
# screenshots: open, highlight, note, save, page operations, form filling, close prompt.
# Prints one PASS/FAIL line per check and exits non-zero on any failure. Screenshots of each step
# go to dev/.tmp/smoke/, kept only when something fails. It is the `gui` layer of the one suite:
#   bash scripts/test.sh gui
set -u
cd "$(dirname "$0")/../.."
APP="${SHEAF_APP:-$HOME/.cache/cargo-target/sheaf/debug/sheaf-app}"   # set by scripts/test.sh
OUT=${SMOKE_OUT:-dev/.tmp/smoke}
rm -rf "$OUT"; mkdir -p "$OUT"
# One Xvfb display per run: gui-parallel.sh runs several shards at once, each on its own.
D=${SMOKE_DISPLAY:-98}
export DISPLAY=:$D
fails=0
pass() { echo "PASS $1"; }
fail() { echo "FAIL $1"; fails=$((fails + 1)); }
check() { if eval "$2"; then pass "$1"; else fail "$1"; fi; }
shot() { import -window root "$OUT/$1.png" 2>/dev/null; }

# A display left by an interrupted run is stopped first.
# Only an Xvfb is stopped (another program's display is left alone: the run fails instead).
[ -f /tmp/.X$D-lock ] && p=$(tr -d ' ' </tmp/.X$D-lock) && [ "$(ps -o comm= -p "$p" 2>/dev/null)" = Xvfb ] && ! tr '\0' ' ' < /proc/"$p"/cmdline 2>/dev/null | grep -q 1920x1080 && kill "$p" 2>/dev/null && sleep 0.5
# The class recorder's display (:99) is never touched, and a lock still held by a live process is
# never removed (27/10: a run took :99 and a class recording was lost).
[ "$D" = 99 ] && { echo "FAIL display :99 belongs to the class recorder"; exit 1; }
if [ -f /tmp/.X$D-lock ] && kill -0 "$(tr -d ' ' </tmp/.X$D-lock)" 2>/dev/null; then echo "FAIL display :$D is in use by another program"; exit 1; fi
rm -f /tmp/.X$D-lock /tmp/.X11-unix/X$D
# -noreset: an X server resets when its last client leaves; an app launched during the reset
# cannot connect (intermittent "opened no window" after a quit, 21/10).
Xvfb :$D -screen 0 1280x800x24 -nolisten tcp -noreset >"$OUT/xvfb.log" 2>&1 &
XVFB=$!
APP_PID=
cleanup() { [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null; kill "$XVFB" 2>/dev/null; }
trap cleanup EXIT
# The checks mean nothing without a display: wait for it, or stop here.
for _ in $(seq 1 40); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.25; done
xdpyinfo >/dev/null 2>&1 || { echo "FAIL no X display :$D (Xvfb did not start)"; exit 1; }

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
# text.pdf as made, for the steps that need a page no other step has drawn on (Prepare form).
cp "$OUT/text.pdf" "$OUT/plain.pdf"

# The user's own Sheaf data, as it is before the run (the check at the end compares).
real_profile() { ls -la --time-style=full-iso "${XDG_DATA_HOME:-$HOME/.local/share}/Sheaf" 2>/dev/null | md5sum; }
REAL0=$(real_profile)

# Toolbar controls reached by position (1280×800 window), kept here: update them when the toolbar
# changes (a capture of the toolbar shows where they went).
# Redesign W3 (29/10): the zoom field at 132–204 (figure 160, arrow 190), the Tools segment of the mode bar at 784.
ZOOM_X=160; ZOOM_MENU_X=190; WRENCH_X=784

# The next file dialog's answer (debug builds read it instead of showing the dialog); empty: none.
answer() { printf '%s' "$1" > "$OUT/dialog-answer"; }
# Images into JPG to PDF by its Open file (Tools ▸ JPG to PDF ▸ Open file, the dialog answered with them): images
# opened from outside go to the reader (C-reader-064). Cards at 101/237/373…, 130.
jpg_to_pdf() { answer "$(printf "$PWD/%s|" "$@" | sed 's/|$//')"; launch; click 50 21 1; click 67 113 1.5; click 271 517 2; click 494 405 1.5; answer ""; }

launch() {
    # A fresh profile: no kept signature, history or preferences from earlier runs.
    # Absolute paths: the dirs crate ignores relative XDG paths and would fall back to the real
    # profile (it did until 21/10: smoke runs wrote into ~/.local/share/Sheaf).
    # Its own temp folder too: the app is killed, so its temp copies are not removed on exit.
    local p="$PWD/$OUT/profile"; mkdir -p "$p/tmp"
    # No app left over on this display: a step that launched twice left one running, the next launch handed
    # its file to it (the window registry) and every later step drove the wrong window (28/10).
    for q in $(pgrep -x sheaf-app-under); do tr '\0' '\n' < /proc/$q/environ 2>/dev/null | grep -qx "DISPLAY=:$D" && kill "$q" 2>/dev/null; done
    rm -rf "$p/data/Sheaf/windows"
    TMPDIR="$p/tmp" XDG_CONFIG_HOME="$p/config" XDG_DATA_HOME="$p/data" XDG_CACHE_HOME="$p/cache" SHEAF_READER_DEBUG=1 SHEAF_TEST_DIALOG="$PWD/$OUT/dialog-answer" SHEAF_TEST_MAIL="$PWD/$OUT/mail-sent" SHEAF_TEST_SCREENSHOT="$PWD/$OUT/stamp.png" SHEAF_TEST_SCAN="$PWD/$OUT/stamp.png" SHEAF_TEST_NOTIFY="$PWD/$OUT/notified" SHEAF_TEST_SPEECH="$PWD/$OUT/spoken" SHEAF_TEST_DEFAULT="$PWD/$OUT/made-default" "$APP" "$@" >>"$OUT/app.log" 2>&1 &
    APP_PID=$!
    for _ in $(seq 1 40); do W=$(xdotool search --name '^Sheaf$' 2>/dev/null | tail -1); [ -n "$W" ] && break; sleep 0.25; done
    [ -n "$W" ] || { echo "FAIL the app opened no window (Xvfb $(kill -0 $XVFB 2>/dev/null && echo alive || echo gone), socket $(ls /tmp/.X11-unix/X$D 2>/dev/null || echo missing), $(ls /proc/$XVFB/fd 2>/dev/null | wc -l) fds, $(pgrep -c xclip) xclip):"; tail -3 "$OUT/app.log"; tail -5 "$OUT/xvfb.log"; exit 1; }
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
# The pointer comes from beside the target: Slint takes no press from a pointer that has not moved onto a row
# rebuilt under it (a second click on the same spot did nothing, 28/10: steps 221, 225, 342b).
click() { xdotool mousemove $(( $1 + 4 )) $(( $2 + 4 )); sleep 0.05; xdotool mousemove "$1" "$2" mousedown 1; sleep 0.08; xdotool mouseup 1; sleep "${3:-0.5}"; }
# Keys go to the app window: focus it first (Xvfb has no window manager to keep it).
key() { xdotool windowfocus --sync "$W" 2>/dev/null; xdotool key "$@"; sleep 0.4; }
# The screen centre "x y" of the first word starting with $1 (OCR at 200 %, its box halved; OCR may glue
# punctuation to it, before it too: a 13 px bold tab title read " mirrored.pdf") in the area $2 (WxH+X+Y, default the whole screen), empty when not seen.
at() { local g="${2:-1280x800+0+0}"; local ox=${g#*+}; ox=${ox%%+*}; local oy=${g##*+}; import -window root -crop "$g" png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' -v w="$1" -v ox="$ox" -v oy="$oy" '{ t = $12; sub(/^[^A-Za-z0-9]+/, "", t) } index($12, w) == 1 || index(t, w) == 1 { printf "%d %d\n", ox + ($7 + $9 / 2) / 2, oy + ($8 + $10 / 2) / 2; exit }'; }
# The screen "left right y" of the first word starting with $1 in the area $2 (as `at`), empty when not seen.
span() { local g="${2:-1280x800+0+0}"; local ox=${g#*+}; ox=${ox%%+*}; local oy=${g##*+}; import -window root -crop "$g" png:- 2>/dev/null | convert - +repage -resize 200% png:- | tesseract stdin stdout tsv 2>/dev/null | awk -F'\t' -v w="$1" -v ox="$ox" -v oy="$oy" '{ t = $12; sub(/^[^A-Za-z0-9]+/, "", t) } index($12, w) == 1 || index(t, w) == 1 { printf "%d %d %d\n", ox + $7 / 2, ox + ($7 + $9) / 2, oy + ($8 + $10 / 2) / 2; exit }'; }
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
# The first line of text of page $2 of $1, its first two words (blank lines skipped).
first_text() { pdftotext -f "$2" -l "$2" "$1" - 2>/dev/null | grep -m1 '[^[:space:]]' | cut -d' ' -f1-2; }

# The steps, in order (dev/scripts/smoke/*.sh share this shell: helpers, profile, variables).
# A shard (gui-parallel.sh) runs some parts only: the helpers the others define come first.
if [ -n "${SMOKE_PARTS:-}" ]; then
    # shellcheck source=/dev/null
    . <(python3 dev/scripts/smoke-helpers.py)
    # The preferences the earlier parts leave (step 9's author, step 10's outputs next to the
    # originals, step 11's light theme), so a later part starts where it would have.
    [ -n "${SMOKE_SEED:-}" ] && mkdir -p "$PWD/$OUT/profile/data/Sheaf" &&
    printf '{"output": "next_to_original", "theme": "light", "author": "Smoke Author"}' > "$PWD/$OUT/profile/data/Sheaf/prefs.json"
fi
for part in ${SMOKE_PARTS:-dev/scripts/smoke/*.sh}; do   # the cwd is the repository root (above)
    . "$part"
done

check "the real user profile was not touched" "[ \"\$(real_profile)\" = \"$REAL0\" ]"
echo "---"
# Outputs are deleted on success (SMOKE_KEEP=1 keeps the screenshots, e.g. to file a proof).
if [ $fails -eq 0 ]; then echo "ALL PASS"; [ -n "${SMOKE_KEEP:-}" ] || rm -rf "$OUT"; else echo "$fails FAILED (screenshots in $OUT)"; fi
exit $fails
