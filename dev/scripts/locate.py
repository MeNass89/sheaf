#!/usr/bin/env python3
"""For every component, find the lines in dev/re/sources/pretty/<app>/ that carry the evidence strings
of its merged features (command ids, labels). Writes the locations into each component's apps.<app>.code
(paths and line numbers only, never code). Needs ripgrep (rg). Run from the project root.

  locate.py            all components
  locate.py C-editor-014 C-editor-015
"""
import collections, json, pathlib, re, subprocess, sys

ROOT = pathlib.Path("dev/re/sources/pretty")
FEATS = pathlib.Path("dev/board/features.json")
COMPS = pathlib.Path("dev/board/components.json")
SKIP = re.compile(r"(locale|/i18n/|/lang/|\.min\.map$)")


def needles(s):
    s = s.strip()
    if len(s) < 4:
        return []
    out = [s]
    if re.fullmatch(r"[a-z0-9.:-]+", s) and ("." in s or ":" in s):   # command ids: also try camelCase
        out.append(re.sub(r"[-_]([a-z0-9])", lambda m: m.group(1).upper(), s))
    return out


def search(app, needle, limit=4):
    try:
        r = subprocess.run(["rg", "-n", "-F", "--no-heading", "-m", "3", "-M", "400", needle, str(ROOT / app)],
                           capture_output=True, text=True, timeout=60)
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return []
    hits = []
    for line in r.stdout.splitlines():
        p, n, *_ = line.split(":", 2)
        rel = p.replace(str(ROOT) + "/", "")
        if not SKIP.search(rel):
            hits.append(f"{rel}:{n}")
    return hits[:limit]


def main(ids):
    feats = json.loads(FEATS.read_text())
    comps = json.loads(COMPS.read_text())
    cov = collections.Counter()
    for c in comps:
        if ids and c["id"] not in ids:
            continue
        per = collections.defaultdict(list)
        for i in c.get("rows", []):
            f = feats[i]
            for ev in f.get("evidence", []):
                for nd in needles(ev):
                    for h in search(f["app"], nd):
                        if h not in per[f["app"]]:
                            per[f["app"]].append(h)
        for app, hits in per.items():
            x = c.setdefault("apps", {}).setdefault(app, {})
            x["code"] = hits[:8]
            if hits:
                x["source"] = "code"; cov[app] += 1
    COMPS.write_text(json.dumps(comps, ensure_ascii=False, indent=1) + "\n")
    print("components with located code, per app:", dict(cov))


if __name__ == "__main__":
    main(set(sys.argv[1:]))
