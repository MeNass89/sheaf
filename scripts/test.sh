#!/usr/bin/env bash
# The project's one test entry point (template from the dev skill, references/testing.md).
# Copy to <project>/scripts/test.sh and edit only the LAYERS block. It runs every layer, prints one
# line per layer and ends on one verdict line; exit 0 only when every layer is green.
#   bash scripts/test.sh              every layer: the only run that closes a unit
#   bash scripts/test.sh unit gui     only these layers, while iterating
# Every layer writes under dev/.tmp/test/: deleted when the run is green, kept when it is red.
set -u
cd "$(dirname "$0")/.."
OUT=dev/.tmp/test
rm -rf "$OUT"; mkdir -p "$OUT"
start=$(date +%s)
failed=(); skipped=()
only=("$@")

# layer NAME COMMAND... : run one layer; its output goes to $OUT/NAME.log.
# The command's exit status is the verdict, so a wrapper (e.g. a memory-capped build) must pass it through.
wanted() { [ ${#only[@]} -eq 0 ] || [[ " ${only[*]} " =~ " $1 " ]]; }
layer() {
    local name=$1; shift
    wanted "$name" || return 0
    if "$@" >"$OUT/$name.log" 2>&1; then
        echo "PASS $name"
    else
        echo "FAIL $name ($OUT/$name.log):"; tail -n 15 "$OUT/$name.log" | sed 's/^/    /'
        failed+=("$name")
    fi
}
# skip NAME REASON : a layer this machine cannot run (no display, other OS). Shown in the verdict.
skip() { wanted "$1" || return 0; echo "SKIP $1: $2"; skipped+=("$1"); }

# ---- LAYERS: one line per layer. A new kind of test is a new line here, never a second script. ----
# On a small Linux machine (under 10 GB, systemd), Cargo runs memory-capped through build-safe.sh,
# which stops the GUI under test first; elsewhere (CI, a Mac) it runs as is.
mem_gb=$(awk '/MemTotal/{print int($2/1048576)}' /proc/meminfo 2>/dev/null || echo 99)
if [ -z "${CI:-}" ] && command -v systemd-run >/dev/null && [ "$mem_gb" -lt 10 ]; then
    cargo_() { sh dev/scripts/build-safe.sh "$@"; }
    export SHEAF_APP="$HOME/.cache/cargo-target/sheaf/debug/sheaf-app"   # dev/scripts/cargo.sh's target dir
else
    cargo_() { cargo "$@"; }
    export SHEAF_APP="${CARGO_TARGET_DIR:-target}/debug/sheaf-app"
fi
export -f cargo_
layer lint cargo_ clippy --workspace --all-targets -- -D warnings
layer unit cargo_ test --workspace --no-fail-fast
# The reader driven for real (Xvfb + xdotool), checked on the files it writes.
if command -v Xvfb >/dev/null && command -v xdotool >/dev/null && command -v qpdf >/dev/null; then
    layer gui bash -c 'cargo_ build -p sheaf-app && bash dev/scripts/gui-smoke.sh'
else
    skip gui "needs Xvfb, xdotool and qpdf"
fi

secs=$(( $(date +%s) - start ))
note=""; [ ${#skipped[@]} -gt 0 ] && note=", skipped: ${skipped[*]}"
if [ ${#failed[@]} -eq 0 ]; then
    rm -rf "$OUT"
    [ ${#only[@]} -gt 0 ] && echo "GREEN (layers: ${only[*]}; not a full run) (${secs}s${note})" && exit 0
    echo "ALL GREEN (${secs}s${note})"; exit 0
fi
echo "RED: ${failed[*]} (${secs}s${note}) — logs in $OUT"
exit 1
