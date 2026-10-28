#!/usr/bin/env python3
"""Cuts a GUI smoke part into contiguous pieces of its steps, for more shards at once (gui-parallel.sh).

Usage: smoke-split.py PART PIECES OUTDIR  ->  prints the piece files written, one per line.

A step starts at a "# N. " header line. Each piece holds the part's lines before its first step, then
its own steps in order. A piece after the first also gets, first, every helper the earlier steps
defined (a "name() {" line through its closing "}" line, or a one-line "name() { ... }"), so a step
that calls a helper defined further up still finds it. Pieces are cut on step boundaries at about
equal line counts; a step never goes to two pieces.
"""
import re
import subprocess
import sys
from pathlib import Path

part, pieces, outdir = Path(sys.argv[1]), int(sys.argv[2]), Path(sys.argv[3])
lines = part.read_text().split("\n")
header = re.compile(r"^# \d+[a-z]?\. ")
starts = [i for i, l in enumerate(lines) if header.match(l)]
if not starts or pieces < 2:
    print(part)
    sys.exit(0)
preamble = lines[: starts[0]]
steps = [lines[a:b] for a, b in zip(starts, starts[1:] + [len(lines)])]

# Contiguous groups of steps of about equal size, cut only where a family of steps ends: a new hundred
# (200s, 300s…) or, under 100, a new ten. A family shares files an earlier step of it made (step 205
# needs 200's across.pdf; cut between them, it failed, 28/10).
def family(s):
    n = int(re.match(r"^# (\d+)", s[0]).group(1))
    return n // 100 if n >= 100 else n // 10 - 100
total = sum(len(s) for s in steps)
groups, current, size = [], [], 0
for k, s in enumerate(steps):
    current.append(s)
    size += len(s)
    ends_family = k + 1 < len(steps) and family(steps[k + 1]) != family(s)
    if ends_family and size >= total / pieces * (len(groups) + 1) and len(groups) < pieces - 1:
        groups.append(current)
        current = []
if current:
    groups.append(current)

func = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\(\) *\{")


def helpers(block):
    """The helper definitions in `block`, in order, whole: from its "name() {" line to the first line that
    makes the definition parse on its own (`bash -n`), however its body is laid out."""
    out, i = [], 0
    while i < len(block):
        if func.match(block[i]):
            for j in range(i, min(i + 120, len(block))):
                text = "\n".join(block[i : j + 1]) + "\n"
                if subprocess.run(["bash", "-n"], input=text, text=True, capture_output=True).returncode == 0:
                    out.extend(block[i : j + 1])
                    i = j
                    break
        i += 1
    return out


outdir.mkdir(parents=True, exist_ok=True)
earlier = []
for n, group in enumerate(groups):
    body = preamble + helpers(earlier) + [l for s in group for l in s]
    path = outdir / f"{part.stem}-{n}.sh"
    path.write_text("\n".join(body) + "\n")
    print(path)
    earlier += [l for s in group for l in s]
