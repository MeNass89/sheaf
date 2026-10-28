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
    # The target dir is shared by every worktree: the GUI layer runs its own copy of this build.
    export SHEAF_APP="$PWD/dev/.tmp/sheaf-app-under-test" SHEAF_COPY_APP="$PWD/dev/.tmp/sheaf-app-under-test"
elif [ -z "${CI:-}" ] && [ -f dev/scripts/cargo.sh ]; then
    # A larger development machine: the same shared target dir and lean debug settings, one rustc
    # per 2.5 GB free (at most one per core), uncapped.
    jobs=$(( $(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo) / 2500 )); [ "$jobs" -lt 1 ] && jobs=1; [ "$jobs" -gt "$(nproc)" ] && jobs=$(nproc)
    # A constrained runner may have less memory than the host: keep its explicit job budget.
    export JOBS="${JOBS:-$jobs}" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/sheaf}"
    # Respect a worker's tighter job budget: the host's free RAM is not its cgroup cap.
    if [[ "${JOBS:-}" =~ ^[1-9][0-9]*$ ]] && [ "$JOBS" -lt "$jobs" ]; then jobs=$JOBS; fi
    export JOBS="$jobs" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/sheaf}"
    # The GUI layer runs its own copy of the app built, so another worktree's build can share the dir.
    cargo_() { sh dev/scripts/cargo.sh "$@" || return; [ "$1" != build ] || { mkdir -p dev/.tmp && cp "$CARGO_TARGET_DIR/debug/sheaf-app" dev/.tmp/sheaf-app-under-test; }; }
    export SHEAF_APP="$PWD/dev/.tmp/sheaf-app-under-test"
else
    cargo_() { cargo "$@"; }
    export SHEAF_APP="${CARGO_TARGET_DIR:-target}/debug/sheaf-app"
fi
export -f cargo_
unit_cmd='cargo_ test --workspace --no-fail-fast && cargo_ test -p pdfcraft-edit --lib && cargo_ test -p pdfcraft-export --lib && cargo_ test -p pdfcraft-ocr --lib'
# A large machine (16 GB and 8 cores or more, not CI) checks lint in its own target dir while the tests build:
# the two compilations no longer wait on each other's lock (28/10: about 9 minutes saved). Its result is
# printed in its place, after the tests.
if [ -z "${CI:-}" ] && [ "$mem_gb" -ge 16 ] && [ "$(nproc)" -ge 8 ] && [ -n "${CARGO_TARGET_DIR:-}" ] && wanted lint && wanted unit; then
    ( CARGO_TARGET_DIR="${CARGO_TARGET_DIR}-lint" JOBS=$(( JOBS > 4 ? JOBS / 2 : JOBS )) cargo_ clippy --workspace --all-targets -- -D warnings >"$OUT/lint.log" 2>&1; echo $? >"$OUT/lint.status" ) &
    lint_pid=$!
    layer unit bash -c "$unit_cmd"
    wait "$lint_pid"
    if [ "$(cat "$OUT/lint.status")" = 0 ]; then echo "PASS lint"; else echo "FAIL lint ($OUT/lint.log):"; tail -n 15 "$OUT/lint.log" | sed 's/^/    /'; failed+=(lint); fi
    rm -f "$OUT/lint.status"
else
    layer lint cargo_ clippy --workspace --all-targets -- -D warnings
    layer unit bash -c "$unit_cmd"
fi
# The reader driven for real (Xvfb + xdotool), checked on the files it writes.
if command -v Xvfb >/dev/null && command -v xdotool >/dev/null && command -v qpdf >/dev/null; then
    # Four cores or more: three shards at once (gui-parallel.sh), each on its own display.
    if [ "$(nproc)" -ge 4 ]; then smoke=gui-parallel.sh; else smoke=gui-smoke.sh; fi
    layer gui bash -c "cargo_ build -p sheaf-app && bash dev/scripts/$smoke"
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
