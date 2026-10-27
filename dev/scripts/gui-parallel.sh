#!/bin/bash
# The GUI smoke test in three shards at once, each on its own Xvfb display, output folder and
# profile (a machine with 4 cores or more): about a third of the time of gui-smoke.sh. Each shard
# runs some parts with the helpers every part defines. Prints the shards' lines in order and exits
# with the total number of failures; a shard's screenshots stay in dev/.tmp/smoke-<n> on failure.
#   bash dev/scripts/gui-parallel.sh
set -u
cd "$(dirname "$0")/../.."
S=dev/scripts/smoke
shards=("$S/1-reader.sh $S/2-documents-and-tools.sh" "$S/3-flows.sh" "$S/4-flows.sh")
pids=()
for i in "${!shards[@]}"; do
    SMOKE_DISPLAY=$((98 + i)) SMOKE_OUT="dev/.tmp/smoke-$i" SMOKE_PARTS="${shards[$i]}" \
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
if [ "$fails" -eq 0 ]; then echo "ALL PASS"; rm -f dev/.tmp/gui-shard-*.log; else echo "$fails FAILED (screenshots in dev/.tmp/smoke-*)"; fi
exit "$fails"
