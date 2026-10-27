#!/usr/bin/env python3
"""Prints every helper function the smoke parts define (dev/scripts/smoke/*.sh), in order, so a
part can run on its own in a shard of gui-parallel.sh. A part may still redefine one."""
import glob, os, re, sys

here = os.path.dirname(os.path.abspath(__file__))
defs, order = {}, []
for f in sorted(glob.glob(os.path.join(here, "smoke", "*.sh"))):
    lines = open(f).read().split("\n")
    i = 0
    while i < len(lines):
        m = re.match(r"^([a-zA-Z_][a-zA-Z0-9_]*)\(\) \{", lines[i])
        if not m:
            i += 1
            continue
        j = i
        first = lines[i].rstrip()
        # A one-line helper closes on its own line; a longer one at a "}" line or a quoted body's "'; }".
        if not (first.endswith("}") and first.count("{") <= first.count("}")):
            j = i + 1
            while j < len(lines) and not (lines[j] == "}" or re.search(r"""['"]\)?; \}$""", lines[j].rstrip())):
                j += 1
        if m.group(1) not in defs:
            order.append(m.group(1))
        defs[m.group(1)] = "\n".join(lines[i:j + 1])
        i = j + 1
sys.stdout.write("\n\n".join(defs[n] for n in order) + "\n")
