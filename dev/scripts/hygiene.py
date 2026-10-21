#!/usr/bin/env python3
"""Storage and code hygiene for a dev project (references/storage.md, references/codebase.md).
Run from the project root, at session start and at the end of every unit.

  hygiene.py                       report against budgets and house rules; exit 1 on any issue
  hygiene.py --fix                 also delete what is safe: stale worktrees, installers and unpacked
                                   copies once archived, dev/.tmp, this project's /tmp dirs, untracked
                                   logs and caches (never a tracked file)
  hygiene.py --target-dir PATH     the shared cargo target dir (default: $CARGO_TARGET_DIR)

"Touched" = files listed by `git status` (the current unit). Code problems in touched files are issues;
the same problems elsewhere are reported as debt and do not fail. Justified exceptions go in
dev/hygiene-allow.txt, one per line: `path  # reason`.
"""
import json, os, pathlib, re, shutil, subprocess, sys, time

FIX = "--fix" in sys.argv
ROOT = pathlib.Path.cwd()
DEV = ROOT / "dev"
MB, GB = 1024 * 1024, 1024 ** 3
SPLIT_LINES = 1000
STALE_DAYS = 2
name = ROOT.name.lower()
target_arg = sys.argv[sys.argv.index("--target-dir") + 1] if "--target-dir" in sys.argv else None
SKIP_DIRS = {".git", "target", "node_modules", "vendor", ".venv", "venv", "dist", "build"}
SOURCE_EXT = {".rs", ".py", ".js", ".ts", ".tsx", ".jsx", ".go", ".swift", ".kt", ".java", ".c", ".cc",
              ".cpp", ".h", ".hpp", ".sh", ".slint", ".rb", ".lua"}


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
        shutil.rmtree(p, ignore_errors=True) if p.is_dir() else p.unlink(missing_ok=True)


def git(*args):
    try:
        return subprocess.run(["git", "--no-optional-locks", *args], capture_output=True, text=True, cwd=ROOT).stdout
    except FileNotFoundError:
        return ""


issues = 0
def issue(msg):
    global issues
    issues += 1
    print(f"  ISSUE {msg}")


allowed = set()
allow_file = DEV / "hygiene-allow.txt"
if allow_file.exists():
    for line in allow_file.read_text().splitlines():
        path = line.split("#", 1)[0].strip()
        if path:
            allowed.add(path)

print("== disk")
free = shutil.disk_usage(ROOT).free
print(f"  free {free/GB:.1f} GB")
if free < 5 * GB:
    issue("less than 5 GB free: storage pass before any build")

# ---------- inspiration apps (phase 3) ----------
inv = json.loads((DEV / "re/inventory.json").read_text()) if (DEV / "re/inventory.json").exists() else []
archived = {r["app"] for r in inv if r.get("archive") and (DEV / r["archive"]).exists()}

print("== archives (≤ 150 MB each)")
for a in sorted((DEV / "re/archive").glob("*.tar.zst")) if (DEV / "re/archive").exists() else []:
    s = size(a)
    print(f"  {a.name}: {s//MB} MB")
    if s > 150 * MB:
        issue(f"{a.name} over 150 MB")

print("== installers and unpacked copies (0 after phase 3)")
for sub in ("downloads", "unpacked", "extracted"):
    d = DEV / "re" / sub
    if d.exists():
        for app in d.iterdir():
            key = app.name.split(".")[0].lower()
            if any(key.startswith(x) or x.startswith(key) for x in archived):
                rm(app, "archived already")
            else:
                issue(f"{app} ({size(app)//MB} MB) not archived yet")

print("== evidence (PNG ≤ 400 KB, raw dumps ≤ 20 MB, total ≤ 200 MB)")
ev = DEV / "re/evidence"
if ev.exists():
    for f in ev.rglob("*"):
        if f.is_file() and f.suffix in (".json", ".txt", ".log") and size(f) > 20 * MB:
            issue(f"{f} {size(f)//MB} MB: summarise into features.json, then delete")
    big_png = [f for f in ev.rglob("*.png") if size(f) > 400 * 1024]
    if big_png:
        issue(f"{len(big_png)} screenshots over 400 KB: downscale to 1600 px")
    if size(ev) > 200 * MB:
        issue(f"evidence total {size(ev)//MB} MB > 200 MB")

# ---------- build and worktrees ----------
print("== worktrees (≤ 3)")
paths = [l.split(" ", 1)[1] for l in git("worktree", "list", "--porcelain").splitlines() if l.startswith("worktree ")][1:]
for p in paths:
    age = (time.time() - pathlib.Path(p).stat().st_mtime) / 86400 if pathlib.Path(p).exists() else 999
    if age > STALE_DAYS:
        if FIX:
            print(f"  remove stale worktree {p} ({age:.0f} d)")
            subprocess.run(["git", "worktree", "remove", "--force", p], cwd=ROOT)
        else:
            issue(f"stale worktree {p} ({age:.0f} d)")
if FIX:
    git("worktree", "prune")
if len(paths) > 3:
    issue(f"{len(paths)} worktrees alive > 3")

print("== Rust target dirs (one shared; stale profiles pruned)")
targets = [p for p in ROOT.rglob("target") if (p / "CACHEDIR.TAG").exists()]
shared = target_arg or os.environ.get("CARGO_TARGET_DIR")
for t in targets:
    print(f"  in project: {t} {size(t)//MB} MB")
if len(targets) > 1:
    issue(f"{len(targets)} target dirs in the project: set CARGO_TARGET_DIR to one shared folder")
if shared and pathlib.Path(shared).is_dir():
    for prof in sorted(pathlib.Path(shared).iterdir()):
        if prof.is_dir():
            age = (time.time() - prof.stat().st_mtime) / 86400
            print(f"  {prof} {size(prof)//MB} MB, last written {age:.0f} d ago")
            if age > STALE_DAYS:
                issue(f"stale profile {prof}: rm -rf it (no build may be running)")
else:
    print(f"  shared target dir: {shared or '(unset; pass --target-dir)'}")

# ---------- temp, test outputs, logs, caches ----------
print("== temp and test outputs (0 after a green run)")
tmp = DEV / ".tmp"
# A red run's logs (scripts/test.sh keeps dev/.tmp/test only when red) live until it is fixed:
# --fix never deletes them (Sheaf, 21/10/2026: --fix erased the logs of a red run).
red = tmp / "test"
if tmp.exists() and any(tmp.iterdir()):
    if red.exists():
        issue(f"dev/.tmp/test holds a red run's logs: fix it, then run the suite again (kept by --fix)")
        if FIX:
            for p in tmp.iterdir():
                if p != red and p.name not in ("smoke",):
                    rm(p, "dev/.tmp outside a red run's logs")
    elif FIX:
        rm(tmp, "dev/.tmp is emptied at the end of every unit")
    else:
        issue(f"dev/.tmp holds {size(tmp)//1024} KB: delete once its failure is fixed")
for d in list(pathlib.Path("/tmp").glob(f"{name}*")) + list(pathlib.Path("/private/tmp").glob(f"{name}-*")):
    if d.resolve() == ROOT.resolve() or d.resolve() in ROOT.resolve().parents:
        continue                             # a project living in /tmp is never its own leftover
    if FIX:
        rm(d, "temp dir left by this project")
    else:
        issue(f"{d} left in temp")

tracked = set(git("ls-files").splitlines())
JUNK_DIRS = {"__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache", "htmlcov", ".nyc_output", "coverage"}
JUNK_FILE = re.compile(r"(\.log|\.profraw|\.orig|\.rej|\.bak|~|\.swp)$|^core(\.\d+)?$|^\.coverage$")
print("== untracked logs and caches")
for dp, dirs, fs in os.walk(ROOT):
    rel = pathlib.Path(dp).relative_to(ROOT)
    dirs[:] = [d for d in dirs if d not in SKIP_DIRS and str(rel / d) not in ("dev/re", "dev/.tmp")]
    for d in [d for d in dirs if d in JUNK_DIRS]:
        dirs.remove(d)
        if not any(t.startswith(str(rel / d) + "/") for t in tracked):
            rm(pathlib.Path(dp) / d, "cache") if FIX else issue(f"cache {rel / d}")
    for f in fs:
        if JUNK_FILE.search(f) and str(rel / f) not in tracked:
            rm(pathlib.Path(dp) / f, "untracked leftover") if FIX else issue(f"leftover {rel / f}")

# ---------- code: orphans, dead code, size ----------
print("== code (issues in touched files; debt elsewhere)")
status = git("status", "--porcelain").splitlines()
touched = {l[3:].split(" -> ")[-1].strip('"') for l in status if not l.startswith(" D") and not l.startswith("D ")}
deleted = {l[3:].strip('"') for l in status if l.startswith(" D") or l.startswith("D ")}
deleted |= {l[3:].split(" -> ")[0] for l in status if l.startswith("R")}
files = sorted((tracked - deleted) | {t for t in touched if (ROOT / t).is_file()})
ORPHAN = re.compile(r"(^|[_.-])(old|bak|backup|copy|v\d+|orig|unused|deprecated)([_.-]|$)", re.I)
CODEY = re.compile(r"(;|\{|\}|\)|,)\s*$|^(let|fn|pub|if|for|while|return|def|import|from|const|var|class|use|match|async|await)\b")
COMMENT = re.compile(r"^\s*(//(?![/!])|#(?![!\[])|--)\s?(.*)$")
debt = {}


def flag(path, kind, msg):
    if path in allowed:
        return
    if path in touched or kind == "debug tag":
        issue(f"{kind}: {msg}")
    else:
        debt.setdefault(kind, []).append(msg)


for path in files:
    p = ROOT / path
    if any(part in SKIP_DIRS for part in pathlib.Path(path).parts) or not p.is_file():
        continue
    if ORPHAN.search(p.stem) and not ORPHAN.fullmatch(p.stem):     # copy.svg is an icon; foo_copy.rs is an orphan
        flag(path, "orphan name", f"{path}: one current file under its real name")
    if p.suffix not in SOURCE_EXT:
        continue
    try:
        lines = p.read_text(errors="ignore").splitlines()
    except OSError:
        continue
    if len(lines) > SPLIT_LINES:
        flag(path, "split candidate", f"{path} {len(lines)} lines: split by responsibility")
    run = 0
    for i, line in enumerate(lines, 1):
        if "[DEBUG" "-" in line:  # split so this line does not flag itself
            flag(path, "debug tag", f"{path}:{i} remove the tagged debug line")
        m = COMMENT.match(line)
        run = run + 1 if m and m.group(2) and CODEY.search(m.group(2).strip()) else 0
        if run == 3:
            flag(path, "commented-out code", f"{path}:{i-2} delete the block (git keeps it)")

for gone in sorted(deleted):
    base = pathlib.Path(gone).name
    if len(base) < 5:
        continue
    refs = [l for l in git("grep", "-n", "-F", base).splitlines() if not l.startswith(gone + ":")]
    for r in refs[:3]:
        issue(f"dangling reference to removed {gone}: {r[:120]}")

for kind, items in debt.items():
    print(f"  debt ({kind}): {len(items)}" + "".join(f"\n    {x}" for x in items[:5]) + ("\n    …" if len(items) > 5 else ""))

print(f"== project total: {size(ROOT)//MB} MB · disk free {shutil.disk_usage(ROOT).free/GB:.1f} GB")
print("OK" if issues == 0 else f"{issues} issue(s){' left after fix' if FIX else ''}")
sys.exit(0 if issues == 0 else 1)
