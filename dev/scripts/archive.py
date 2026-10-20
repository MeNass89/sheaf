#!/usr/bin/env python3
"""Pack an inspiration app's readable code and feature-bearing resources into dev/re/archive/<app>.tar.zst,
verify it, and record it in dev/re/inventory.json. Run from the project root. Needs zstd.

  archive.py pack <app> <src_dir> [--version V] [--source URL] [--installer-sha256 H]
  archive.py extract <app>        unpack the archive into dev/re/sources/pretty/<app>/
  archive.py list

Kept: any text file (no NUL byte in its first 8 KB) plus source code (js, mjs, ts, jsx, tsx, css, html, qml, py, java, kt, swift, m, h, c, cc, cpp, rs, go, cs,
lua, sh), text resources (json, xml, plist, strings, yaml, yml, toml, ini, md, txt, po, ts-translations,
properties, svg), decompiler output. Dropped: binaries, images, fonts, media, archives, maps over 2 MB.
"""
import hashlib, json, os, pathlib, subprocess, sys, datetime

KEEP = {".js", ".mjs", ".cjs", ".ts", ".jsx", ".tsx", ".css", ".scss", ".html", ".htm", ".qml", ".py", ".java",
        ".kt", ".swift", ".m", ".mm", ".h", ".hpp", ".c", ".cc", ".cpp", ".rs", ".go", ".cs", ".lua", ".sh",
        ".json", ".xml", ".plist", ".strings", ".stringsdict", ".yaml", ".yml", ".toml", ".ini", ".md", ".txt",
        ".po", ".properties", ".svg", ".smali", ".il", ".decompiled", ".asm", ".gradle", ".sql", ".graphql"}
MAX_FILE = 8 * 1024 * 1024
ARCH = pathlib.Path("dev/re/archive")
INV = pathlib.Path("dev/re/inventory.json")
PRETTY = pathlib.Path("dev/re/sources/pretty")


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def is_text(p):
    """Unknown extensions are kept when the file is text (no NUL byte in its first 8 KB)."""
    try:
        with open(p, "rb") as f:
            head = f.read(8192)
        return bool(head) and b"\0" not in head
    except OSError:
        return False


def pack(app, src, version="", source="", inst_sha=""):
    src = pathlib.Path(src).resolve()
    files = []
    for dp, _, fs in os.walk(src):
        for f in fs:
            p = pathlib.Path(dp) / f
            if not p.is_symlink() and p.stat().st_size <= MAX_FILE and (p.suffix.lower() in KEEP or is_text(p)):
                files.append(p.relative_to(src))
    ARCH.mkdir(parents=True, exist_ok=True)
    out = (ARCH / f"{app}.tar.zst").resolve()
    lst = pathlib.Path(f"/tmp/{app}-filelist.txt")
    lst.write_text("\n".join(str(f) for f in files))
    subprocess.run(["tar", "-C", str(src), "-cf", "-", "-T", str(lst)], check=True, stdout=open("/tmp/" + app + ".tar", "wb"))
    subprocess.run(["zstd", "-19", "-T0", "-q", "-f", "/tmp/" + app + ".tar", "-o", str(out)], check=True)
    os.remove("/tmp/" + app + ".tar"); lst.unlink()
    n = int(subprocess.run(f"zstd -dc '{out}' | tar -tf - | wc -l", shell=True, capture_output=True, text=True).stdout.strip())
    if n != len(files):
        sys.exit(f"verify failed: {n} in archive vs {len(files)} selected")
    inv = json.loads(INV.read_text()) if INV.exists() else []
    inv = [r for r in inv if r.get("app") != app]
    inv.append({"app": app, "version": version, "source": source, "installer_sha256": inst_sha,
                "archive": str(out.relative_to(pathlib.Path.cwd())), "archive_sha256": sha256(out),
                "files": n, "bytes": out.stat().st_size, "date": datetime.date.today().isoformat()})
    INV.parent.mkdir(parents=True, exist_ok=True)
    INV.write_text(json.dumps(inv, ensure_ascii=False, indent=1) + "\n")
    print(f"{app}: {n} files, {out.stat().st_size // (1024*1024)} MB → {out}")


def extract(app):
    a = ARCH / f"{app}.tar.zst"
    d = PRETTY / app
    d.mkdir(parents=True, exist_ok=True)
    subprocess.run(f"zstd -dc '{a}' | tar -xf - -C '{d}'", shell=True, check=True)
    print(f"extracted {app} → {d}")


if __name__ == "__main__":
    a = sys.argv[1:]
    if not a or a[0] == "list":
        for r in (json.loads(INV.read_text()) if INV.exists() else []):
            print(f"{r['app']:12s} {r.get('version',''):12s} {r['files']:7d} files {r['bytes']//(1024*1024):5d} MB")
    elif a[0] == "pack":
        kw = {"version": "", "source": "", "inst_sha": ""}
        for k, n in (("--version", "version"), ("--source", "source"), ("--installer-sha256", "inst_sha")):
            if k in a:
                kw[n] = a[a.index(k) + 1]
        pack(a[1], a[2], **kw)
    elif a[0] == "extract":
        extract(a[1])
