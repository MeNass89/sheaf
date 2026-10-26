#!/bin/sh
# Builds through dev/scripts/cargo.sh without endangering the rest of a small machine (7 GB, no swap):
# - waits until at least 3 GB are free (rustc on sheaf-app peaks at ~2.4 GB);
# - caps the build at (free memory − 700 MB), so the machine never runs out (1.2 GB above it when a
#   swap file is on: see DECISIONS 2026-10-26);
# - OOMScoreAdjust=1000: if memory still runs out, the kernel kills the build, not the session.
# The app under test must be stopped first (systemctl --user stop sheaf-gui).
#   sh dev/scripts/build-safe.sh build -p sheaf-app
set -eu
cd "$(dirname "$0")/../.."
mkdir -p dev/.tmp
# One build at a time on the machine, whichever worktree or agent starts it (two rustc on sheaf-app
# need ~5 GB): wait for the lock.
exec 9>/tmp/sheaf-build.lock
flock 9
systemctl --user stop sheaf-gui 2>/dev/null || true
# On the VPS the ChatGPT desktop app takes ~1.9 GB and stays closed (Nassim, 21/10/2026).
systemctl --user stop chatgpt-desktop 2>/dev/null || true
for _ in $(seq 1 60); do
    avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
    [ "$avail" -ge 3000 ] && break
    echo "waiting: ${avail} MB free, need 3000" >&2
    sleep 10
done
avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
[ "$avail" -ge 3000 ] || { echo "not enough memory (${avail} MB free); build not started" >&2; exit 3; }
# With a swap file on (512 MB free at least), idle processes can make room: the cap goes 1.2 GB above
# what is free (the build itself never swaps and dies first). Without, 700 MB stay free.
swap=$(awk '/SwapFree/{print int($2/1024)}' /proc/meminfo)
if [ "${swap:-0}" -ge 512 ]; then cap=$((avail + 1200)); else cap=$((avail - 700)); fi
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
systemd-run --user --unit=heavy-build-$$ --collect -q --wait --working-directory="$PWD" \
    -p MemoryMax="${cap}M" -p MemorySwapMax=0 -p OOMScoreAdjust=1000 \
    --setenv=JOBS=1 --setenv=PATH="$PATH" --setenv=HOME="$HOME" \
    -- sh -c "sh dev/scripts/cargo.sh $* > dev/.tmp/build.log 2>&1; echo EXIT \$? >> dev/.tmp/build.log"
# The build's own exit status is this script's: a layer of scripts/test.sh trusts it.
code=$(tail -1 dev/.tmp/build.log | sed -n 's/^EXIT //p')
# SHEAF_COPY_APP=path: the app just built is copied there before the lock is released, so a test
# runs this build and not one another worktree builds right after (they share the target dir).
if [ "${code:-1}" -eq 0 ] && [ -n "${SHEAF_COPY_APP:-}" ]; then
    # Copied then renamed: a copy still running keeps its file ("text file busy" otherwise).
    cp "${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/sheaf}/debug/sheaf-app" "$SHEAF_COPY_APP.new" && mv -f "$SHEAF_COPY_APP.new" "$SHEAF_COPY_APP"
fi
if [ "${code:-1}" -eq 0 ]; then
    grep -E '^(warning|error)' dev/.tmp/build.log | grep -v 'generated [0-9]* warning' || true
    rm -f dev/.tmp/build.log
    echo "EXIT 0"
    exit 0
fi
tail -n 30 dev/.tmp/build.log    # kept until the failure is fixed
exit "${code:-1}"
