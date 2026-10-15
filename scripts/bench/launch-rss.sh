#!/bin/sh
# Build first: swiftc -O scripts/bench/coldstart.swift -o scripts/bench/coldstart
# usage: launch-rss.sh <app path> <owner> <process match> <quit name>
set -u
app="$1"; owner="$2"; match="$3"; qname="$4"
for run in 1 2 3; do
  osascript -e "quit app \"$qname\"" >/dev/null 2>&1; sleep 1; pkill -f "$match" 2>/dev/null; sleep 3
  t=$("$(dirname "$0")/coldstart" "$app" "$owner")
  sleep 10
  pid=$(pgrep -f "$match" | head -1)
  rss=$(ps -o rss= -p "$pid" | tr -d ' ')
  kids=$(pgrep -P "$pid" | tr '\n' ' ')
  krss=0; for k in $kids; do krss=$((krss + $(ps -o rss= -p $k | tr -d ' '))); done
  echo "run $run: first-window ${t}s  rss ${rss} KB  children [${kids}] ${krss} KB"
done
osascript -e "quit app \"$qname\"" >/dev/null 2>&1
