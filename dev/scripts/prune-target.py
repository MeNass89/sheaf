#!/usr/bin/env python3
"""Deletes from the shared Cargo target dir every artifact the suite no longer uses.

Cargo keeps one build per (crate, features, profile settings); each settings change leaves the
previous builds behind, and on 21/10/2026 they filled 7 GB of a 48 GB disk. This asks Cargo, with
everything already built (so it costs seconds), which artifacts the suite's builds use — the GUI
build, the workspace tests, clippy — and deletes every other file in deps/, build/ and
.fingerprint/ of the debug profile. Run it after a green suite:
    python3 dev/scripts/prune-target.py          # report what would go
    python3 dev/scripts/prune-target.py --fix    # delete it
"""
import json, os, re, shutil, subprocess, sys

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
# Relative to the project, as Cargo reads it (its runs below start in the project).
target = os.path.join(root, os.path.expanduser(os.environ.get("CARGO_TARGET_DIR") or "~/.cache/cargo-target/sheaf"))
debug = os.path.join(target, "debug")
HASH = re.compile(r"-([0-9a-f]{16})(?:\.|$|/)")

# The builds the suite makes (scripts/test.sh). Brought up to date first through the memory-capped
# wrapper (seconds when they are), so the queries below never compile outside the cap.
builds = [["build", "-p", "sheaf-app"], ["test", "--workspace", "--no-run"], ["clippy", "--workspace", "--all-targets"]]
for b in builds:
    if subprocess.run(["sh", "dev/scripts/build-safe.sh", *b], cwd=root).returncode != 0:
        sys.exit(f"cargo {' '.join(b)} failed: nothing pruned")
# Same wrapper, same profile flags: any other flags would change every hash and rebuild everything.
keep = set()
env = dict(os.environ, CARGO_TARGET_DIR=target)
for b in builds:
    out = subprocess.run(["sh", "dev/scripts/cargo.sh", *b, "--offline", "--message-format=json"], cwd=root, env=env, capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"cargo {' '.join(b)} failed: run the suite first\n{out.stderr[-2000:]}")
    for line in out.stdout.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        paths = m.get("filenames", []) + [m.get("executable") or "", m.get("out_dir") or ""]
        for p in paths:
            keep.update(HASH.findall(p + "/"))
    if re.search(r"^\s*(Compiling|Checking) ", out.stderr, re.M):
        sys.exit(f"cargo {' '.join(b)} compiled something: run the suite first, then prune")

gone, size = [], 0
for sub in ("deps", "build", ".fingerprint"):
    d = os.path.join(debug, sub)
    for name in os.listdir(d) if os.path.isdir(d) else []:
        h = HASH.findall(name + "/")
        if h and not set(h) & keep:
            p = os.path.join(d, name)
            if os.path.isdir(p):
                size += sum(os.path.getsize(os.path.join(a, f)) for a, _, fs in os.walk(p) for f in fs)
            else:
                size += os.path.getsize(p)
            gone.append(p)
print(f"{len(keep)} artifact hashes in use; {len(gone)} stale entries, {size / 1e9:.1f} GB")
if "--fix" in sys.argv:
    for p in gone:
        shutil.rmtree(p) if os.path.isdir(p) else os.remove(p)
    print("deleted")
