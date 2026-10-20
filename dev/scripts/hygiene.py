#!/usr/bin/env python3
"""Storage guardrails for a dev project (references/storage.md). Run from the project root.

  hygiene.py          report sizes against budgets
  hygiene.py --fix    also prune stale worktrees, delete installers/unpacked copies once archived,
                      empty dev/.tmp, remove project temp dirs left in /tmp by this project
"""
import json, os, pathlib, shutil, subprocess, sys, time

FIX = "--fix" in sys.argv
ROOT = pathlib.Path.cwd()
DEV = ROOT / "dev"
MB = 1024 * 1024
name = ROOT.name.lower()


def size(p):
    p = pathlib.Path(p)
    if not p.exists():
        return 0
    if p.is_file():
        return p.stat().st_size
    total = 0
    for dp, _, fs in os.walk(p, followlinks=False):
        for f in fs:
            try:
                total += os.lstat(os.path.join(dp, f)).st_size
            except OSError:
                pass
    return total


def rm(p, why):
    p = pathlib.Path(p)
    if p.is_symlink():                       # never follow a link out of the project (e.g. /tmp/x -> ~/.cargo)
        print(f"  unlink {p} (symlink): {why}")
        if FIX:
            p.unlink(missing_ok=True)
        return
    print(f"  remove {p} ({size(p)//MB} MB): {why}")
    if FIX:
        shutil.rmtree(p, ignore_errors=True) if pathlib.Path(p).is_dir() else pathlib.Path(p).unlink(missing_ok=True)


issues = 0
inv = json.loads((DEV / "re/inventory.json").read_text()) if (DEV / "re/inventory.json").exists() else []
archived = {r["app"] for r in inv if r.get("archive") and (DEV / r["archive"]).exists()}

print("== archives")
for a in sorted((DEV / "re/archive").glob("*.tar.zst")) if (DEV / "re/archive").exists() else []:
    s = size(a)
    flag = "  OVER 150 MB" if s > 150 * MB else ""
    issues += bool(flag)
    print(f"  {a.name}: {s//MB} MB{flag}")

print("== installers and unpacked copies (0 after phase 3)")
for sub in ("downloads", "unpacked", "extracted"):
    d = DEV / "re" / sub
    if d.exists():
        for app in d.iterdir():
            key = app.name.split(".")[0].lower()
            if any(key.startswith(x) or x.startswith(key) for x in archived):
                rm(app, "archived already")
            else:
                print(f"  keep {app} ({size(app)//MB} MB): not archived yet"); issues += 1

print("== raw REA dumps (≤ 20 MB each)")
ev = DEV / "re/evidence"
if ev.exists():
    for f in ev.rglob("*"):
        if f.is_file() and f.suffix in (".json", ".txt", ".log") and size(f) > 20 * MB:
            print(f"  {f} {size(f)//MB} MB: summarise into features.json, then delete"); issues += 1
    big_png = [f for f in ev.rglob("*.png") if size(f) > 400 * 1024]
    if big_png:
        print(f"  {len(big_png)} screenshots over 400 KB: downscale to 1600 px"); issues += 1
    if size(ev) > 200 * MB:
        print(f"  evidence total {size(ev)//MB} MB > 200 MB"); issues += 1

print("== worktrees (≤ 3)")
try:
    wt = subprocess.run(["git", "worktree", "list", "--porcelain"], capture_output=True, text=True).stdout
    paths = [l.split(" ", 1)[1] for l in wt.splitlines() if l.startswith("worktree ")][1:]
    for p in paths:
        age = (time.time() - pathlib.Path(p).stat().st_mtime) / 86400 if pathlib.Path(p).exists() else 999
        if age > 2:
            print(f"  stale worktree {p} ({age:.0f} d)")
            if FIX:
                subprocess.run(["git", "worktree", "remove", "--force", p]); issues += 0
            else:
                issues += 1
    if FIX:
        subprocess.run(["git", "worktree", "prune"])
    if len(paths) > 3:
        print(f"  {len(paths)} worktrees alive > 3"); issues += 1
except FileNotFoundError:
    pass

print("== Rust target dirs")
targets = [p for p in ROOT.rglob("target") if (p / "CACHEDIR.TAG").exists()]
shared = os.environ.get("CARGO_TARGET_DIR")
for t in targets:
    print(f"  {t} {size(t)//MB} MB")
if len(targets) > 1:
    print(f"  {len(targets)} target dirs: set CARGO_TARGET_DIR to one shared folder"); issues += 1
print(f"  CARGO_TARGET_DIR={shared or '(unset)'}")

print("== temp")
tmp = DEV / ".tmp"
if tmp.exists() and FIX:
    shutil.rmtree(tmp, ignore_errors=True)
for d in pathlib.Path("/tmp").glob(f"{name}*"):
    rm(d, "temp dir left by this project")
for d in pathlib.Path("/private/tmp").glob(f"{name}-*"):
    rm(d, "temp dir left by this project")

print(f"== project total: {size(ROOT)//MB} MB")
print("OK" if issues == 0 else f"{issues} issue(s){' (after fix)' if FIX else ''}")
sys.exit(0 if issues == 0 else 1)
