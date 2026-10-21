#!/bin/sh
# Builds through dev/scripts/cargo.sh without endangering the rest of a small machine (7 GB, no swap):
# - waits until at least 3 GB are free (rustc on sheaf-app peaks at ~2.4 GB);
# - caps the build at (free memory − 700 MB), so the machine never runs out;
# - OOMScoreAdjust=1000: if memory still runs out, the kernel kills the build, not the session.
# The app under test must be stopped first (systemctl --user stop sheaf-gui).
#   sh dev/scripts/build-safe.sh build -p sheaf-app
set -eu
cd "$(dirname "$0")/../.."
systemctl --user stop sheaf-gui 2>/dev/null || true
# On the VPS the ChatGPT desktop app takes ~1.9 GB: stopped for the build, restarted after
# (Nassim's agreement, 21/10/2026).
restart_chatgpt=0
if systemctl --user is-active -q chatgpt-desktop 2>/dev/null; then
    systemctl --user stop chatgpt-desktop && restart_chatgpt=1
    sleep 3
fi
trap '[ "$restart_chatgpt" = 1 ] && systemctl --user start chatgpt-desktop' EXIT
for _ in $(seq 1 60); do
    avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
    [ "$avail" -ge 3000 ] && break
    echo "waiting: ${avail} MB free, need 3000" >&2
    sleep 10
done
avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
[ "$avail" -ge 3000 ] || { echo "not enough memory (${avail} MB free); build not started" >&2; exit 3; }
cap=$((avail - 700))
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
systemd-run --user --unit=heavy-build --collect -q --wait --working-directory="$PWD" \
    -p MemoryMax="${cap}M" -p MemorySwapMax=0 -p OOMScoreAdjust=1000 \
    --setenv=JOBS=1 --setenv=PATH="$PATH" --setenv=HOME="$HOME" \
    -- sh -c "sh dev/scripts/cargo.sh $* > dev/.tmp/build.log 2>&1; echo EXIT \$? >> dev/.tmp/build.log"
tail -1 dev/.tmp/build.log
