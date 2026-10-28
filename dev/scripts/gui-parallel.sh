#!/bin/bash
# The GUI smoke test in three shards at once, each on its own Xvfb display, output folder and
# profile (a machine with 4 cores or more): about a third of the time of gui-smoke.sh. Each shard
# runs some parts with the helpers every part defines. Prints the shards' lines in order and exits
# with the total number of failures; a shard's screenshots stay in dev/.tmp/smoke-<n> on failure.
#   bash dev/scripts/gui-parallel.sh
set -u
cd "$(dirname "$0")/../.."
S=dev/scripts/smoke
# More shards on 8 cores or more (4-flows cut between its step families by smoke-split.py,
# helpers carried along), three below: each shard waits on the screen, not the processor, so more of them
# finish sooner (28/10: the GUI layer took 23 min in three shards).
if [ "$(nproc)" -ge 8 ]; then
    split=dev/.tmp/smoke-split; rm -rf "$split"
    # 3-flows stays whole: its steps reuse files made many steps earlier (withatt.pdf from 25 in 52).
    mapfile -t four < <(python3 dev/scripts/smoke-split.py "$S/4-flows.sh" 3 "$split")
    shards=("$S/1-reader.sh $S/2-documents-and-tools.sh" "$S/3-flows.sh" "${four[@]}")
else
    shards=("$S/1-reader.sh $S/2-documents-and-tools.sh" "$S/3-flows.sh" "$S/4-flows.sh")
fi
# Displays 140–145: low numbers belong to others on this machine (the class recorder's Xvfb is :99;
# a shard on it killed a class's recording, 27/10). During a class being recorded, a machine under
# 8 cores waits; a larger one runs at the lowest priority, leaving the recording its share.
if pgrep -f '[j]oin_teams\.py' >/dev/null; then
    if [ "$(nproc)" -lt 8 ]; then
        while pgrep -f '[j]oin_teams\.py' >/dev/null; do echo "waiting: a class is being recorded" >&2; sleep 60; done
    else
        renice -n 19 $$ >/dev/null
    fi
fi
pids=()
for i in "${!shards[@]}"; do
    # Every shard but the first starts from the preferences the earlier parts leave.
    SMOKE_SEED=$([ "$i" -gt 0 ] && echo 1) SMOKE_DISPLAY=$((140 + i)) SMOKE_OUT="dev/.tmp/smoke-$i" SMOKE_PARTS="${shards[$i]}" \
        bash dev/scripts/gui-smoke.sh > "dev/.tmp/gui-shard-$i.log" 2>&1 &
    pids+=($!)
    sleep 2   # the displays start one after the other
done
fails=0
for i in "${!pids[@]}"; do
    wait "${pids[$i]}" || true
    grep -a '^PASS\|^FAIL' "dev/.tmp/gui-shard-$i.log"
    n=$(grep -ac '^FAIL' "dev/.tmp/gui-shard-$i.log")
    fails=$((fails + n))
    # A shard that stopped early (no ALL PASS nor its count) says so.
    grep -aq 'ALL PASS\|FAILED' "dev/.tmp/gui-shard-$i.log" || { echo "FAIL shard $i stopped early:"; tail -5 "dev/.tmp/gui-shard-$i.log"; fails=$((fails + 1)); }
done
if [ "$fails" -eq 0 ]; then echo "ALL PASS"; rm -rf dev/.tmp/gui-shard-*.log dev/.tmp/smoke-split; else echo "$fails FAILED (screenshots in dev/.tmp/smoke-*)"; fi
exit "$fails"
