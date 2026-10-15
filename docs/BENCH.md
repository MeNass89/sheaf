# Sheaf vs a commercial PDF desktop app: size and speed

_Sheaf is an independent project, not affiliated with or endorsed by iLovePDF. Figures are factual measurements of publicly available software._

Measured on 15/10/2026 on the same Mac (Apple M4 Pro, 24 GB, macOS 27.2), with
Sheaf 0.1.0 (`target/Sheaf.app`, built by `scripts/bundle-macos.sh`) and
iLovePDF Desktop 2.3.0 (`/Applications/iLovePDF.app`). Input file: `WelcomeGuide.pdf`
(3,322,740 bytes, 7 pages; the guide shipped inside iLovePDF.app). Sizes are in
binary units (1 MB = 1,048,576 bytes), the units both apps display.

| # | Measure | Sheaf 0.1.0 | iLovePDF 2.3.0 | Method |
|---|---|---|---|---|
| 1 | App size on disk | **29 MB** (29,804 KB) | 334 MB (342,272 KB) | `du -sh` / `du -sk` on the `.app` |
| 1b | Download size | **13 MB** zip (13,776,733 bytes) | not measured | `ditto -c -k --keepParent target/Sheaf.app` |
| 2 | Architecture | arm64 (native) | x86_64 (runs under Rosetta 2) | `file Contents/MacOS/<main binary>` |
| 3 | Launch to first window, median of 3 | **0.35 s** (0.46 / 0.30 / 0.35) | 1.12 s (2.56 / 1.12 / 1.11) | `open -a`, then poll the on-screen window list (CGWindowList, layer 0, owner = process name) every 50 ms until a window exists. Before each run: quit the app, wait 3 s. No `purge`, so these are warm-disk launches. |
| 4 | Idle memory 10 s after the window appears, median of 3 | **115 MB** (117,232 to 117,840 KB) | 617 MB (615,432 to 638,580 KB) | `ps -o rss= -p <pid>`, plus child processes found with `pgrep -P`. Neither app had children. iLovePDF's RSS includes Rosetta translation overhead. |
| 5 | Merge WelcomeGuide ×10 (70 pages, 33 MB out) | **0.14 s** (0.11 / 0.15 / 0.14), peak 66 MB | not measured (iLovePDF has no CLI; GUI-only) | `/usr/bin/time -l sheaf merge WelcomeGuide.pdf ×10 -o merged.pdf`, 3 runs, median |
| 6 | Compress WelcomeGuide, Recommended | **0.14 s**; 3.17 MB → **1.85 MB (−41.5%, shown as −42%)** (1,943,407 bytes) | about 0.7 s after the save dialog, timed by hand; 3.17 MB → 2.23 MB (−29%) | Sheaf: `/usr/bin/time -l sheaf compress --level recommended`, 3 runs, all 0.14 s (the first run of a freshly signed binary took 0.70 s, the one-time first-launch check; not counted). iLovePDF: run through its GUI on 15/10, stopwatch, single run. The Sheaf GUI gives the same file (screenshot `t13-bundle-compress.png`: 3.17 MB → 1.85 MB, −42%). |
| 7 | Peak memory during compress | **57 MB** (59,703,296 bytes max RSS, median) | not measured | `/usr/bin/time -l`, "maximum resident set size" |

## Reading the table

- About **11× smaller** on disk (29 MB vs 334 MB). The bundle holds an 18 MB app
  binary, a 3.8 MB CLI, the 7.3 MB pdfium library and 0.5 MB of licence notices.
- About **3× faster** to the first window, and **5× less memory** at idle.
- Compress on Recommended gives a file 17% smaller than iLovePDF's
  (1.85 MB vs 2.23 MB). The rows measure the work differently: the CLI runs the
  whole process, while the iLovePDF figure is stopwatch time after its save dialog.

## Reproduce

```sh
./scripts/bundle-macos.sh
du -sh target/Sheaf.app /Applications/iLovePDF.app
file target/Sheaf.app/Contents/MacOS/Sheaf /Applications/iLovePDF.app/Contents/MacOS/iLovePDF
B=target/Sheaf.app/Contents/Helpers/sheaf
/usr/bin/time -l $B compress WelcomeGuide.pdf --level recommended -o out.pdf
/usr/bin/time -l $B merge WelcomeGuide.pdf WelcomeGuide.pdf … (×10) -o merged.pdf
```

Launch time and idle memory (rows 3 and 4):

```sh
swiftc -O scripts/bench/coldstart.swift -o scripts/bench/coldstart
scripts/bench/launch-rss.sh "$PWD/target/Sheaf.app" Sheaf Sheaf.app/Contents/MacOS/Sheaf Sheaf
scripts/bench/launch-rss.sh /Applications/iLovePDF.app iLovePDF iLovePDF.app/Contents/MacOS/iLovePDF iLovePDF
```

`scripts/bench/coldstart.swift` runs `open -a <app>` and then polls
`CGWindowListCopyWindowInfo` every 50 ms for an on-screen layer-0 window whose
owner matches the app's process name. `launch-rss.sh` runs it 3 times (quitting
the app and waiting 3 s before each run) and reads `ps -o rss=` 10 s after the
window appears.

Sheaf rows were re-measured after the image crate was trimmed to the formats
Sheaf reads (png, jpeg, webp, tiff, bmp, gif); iLovePDF rows are from the
first pass on the same day.
