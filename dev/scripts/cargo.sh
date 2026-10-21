#!/bin/sh
# cargo for small machines (2 cores, 7 GB RAM, little disk): one shared target dir, two jobs, optimised
# dependencies (PdfCraft and pdfium bindings unoptimised took 30 s to open a 2,000-page book), line tables
# only for our crates and no debug info for dependencies. Full debug info made the test binaries 14 GB.
# Use it for every build so the flags never change (a change rebuilds everything).
#   sh dev/scripts/cargo.sh build --workspace
#   sh dev/scripts/cargo.sh test --workspace
set -eu
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/sheaf}"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cmd=$1; shift
exec cargo "$cmd" -j "${JOBS:-2}" \
    --config 'profile.dev.debug="line-tables-only"' \
    --config 'profile.dev.package."*".debug=false' \
    --config 'profile.dev.package."*".opt-level=2' \
    "$@"
