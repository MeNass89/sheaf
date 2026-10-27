#!/bin/sh
# cargo for small machines (2 cores, 7 GB RAM, little disk): one shared target dir, two jobs, optimised
# dependencies (PdfCraft and pdfium bindings unoptimised took 30 s to open a 2,000-page book), line tables
# only for our crates and no debug info for dependencies. Full debug info made the test binaries 14 GB.
# Use it for every build so the flags never change (a change rebuilds everything).
#   sh dev/scripts/cargo.sh build --workspace
#   sh dev/scripts/cargo.sh test --workspace
set -eu
# No incremental cache, and no debug info for the app (Slint generates one huge module): rustc on
# sheaf-app otherwise needs more than 2.5 GB.
export CARGO_INCREMENTAL=0
# Bound glibc's per-thread allocation arenas on memory-capped builders. This only affects
# build/test processes, not the app a user launches; an explicit caller setting takes precedence.
export MALLOC_ARENA_MAX="${MALLOC_ARENA_MAX:-1}"
# Keep large compiler buffers separately releasable instead of growing glibc's adaptive
# mmap/trim thresholds during a long Slint compile. These are bytes (128 KiB).
export MALLOC_MMAP_THRESHOLD_="${MALLOC_MMAP_THRESHOLD_:-131072}"
export MALLOC_TRIM_THRESHOLD_="${MALLOC_TRIM_THRESHOLD_:-131072}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/sheaf}"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
# The faces for text WinAnsi cannot write (scripts/fetch-fonts.sh), embedded by PdfCraft when present.
fonts="$(cd "$(dirname "$0")/../.." && pwd)/vendor/craft-fonts"
[ -f "$fonts/fonts/manifest.txt" ] && export CRAFT_FONTS_DIR="$fonts"
# Nassim 21/10/2026: nothing else runs on this VPS while a class is being recorded
# (7 GB RAM; a cargo test left 184 MB free mid-class). Wait, never build over it.
while pgrep -f join_teams.py >/dev/null 2>&1; do
    echo "cargo.sh: class-recorder is recording, waiting 60 s" >&2
    sleep 60
done
cmd=$1; shift
exec cargo "$cmd" -j "${JOBS:-2}" \
    --config 'profile.dev.debug="line-tables-only"' \
    --config 'profile.dev.package."*".debug=false' \
    --config 'profile.dev.package."*".opt-level=2' \
    --config 'profile.dev.package.sheaf-app.debug=false' \
    "$@"
