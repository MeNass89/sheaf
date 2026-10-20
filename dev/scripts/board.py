#!/usr/bin/env python3
"""The parity board's only writer. Run from the project root (the folder that contains dev/).

  board.py init                     create dev/ with empty apps/features/components and .gitignore
  board.py check                    validate every component against the schema rules
  board.py render                   write dev/BOARD.md from dev/board/components.json
  board.py next                     print the next component to work on (heaviest, then lowest grade)
  board.py status                   counts per status, per domain average, how many at 9+
  board.py grade ID --function N --aesthetics N --performance N --proof "…" [--feature F-NNN]
  board.py start ID F-NNN           mark a component in progress under a feature
"""
import argparse, collections, datetime, json, pathlib, sys

DEV = pathlib.Path("dev")
APPS = DEV / "board/apps.json"
FEATS = DEV / "board/features.json"
COMPS = DEV / "board/components.json"
OUT = DEV / "BOARD.md"
WEIGHT = {"strong": 0, "normal": 1, "light": 2}
AXES = ("function", "aesthetics", "performance")
GATE = 9


def load(p, default):
    return json.loads(p.read_text()) if p.exists() else default


def save(p, data):
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n")


def overall(c):
    g = c.get("grade") or {}
    vals = [g.get(a) for a in AXES]
    if any(v is None for v in vals):
        return None
    return min(vals)


def cmd_init(_):
    for p, d in ((APPS, []), (FEATS, []), (COMPS, [])):
        if not p.exists():
            save(p, d)
    for d in ("features", "re/sources/pretty", "re/evidence", "scripts"):
        (DEV / d).mkdir(parents=True, exist_ok=True)
    gi = DEV / ".gitignore"
    if not gi.exists():
        gi.write_text("re/sources/\nre/unpacked/\nre/downloads/\n")
    print("dev/ ready")


def problems(c, apps):
    out = []
    for k in ("id", "name", "domain", "definition", "weight", "main", "apps", "grade", "status"):
        if not c.get(k):
            out.append(f"missing {k}")
    if c.get("weight") not in WEIGHT:
        out.append("weight must be strong|normal|light")
    havers = [a for a, x in (c.get("apps") or {}).items() if x.get("has") in ("yes", "partial")]
    if c.get("main") and havers and c["main"] not in havers and c["main"] != "ours":
        out.append(f"main '{c['main']}' does not have the feature (havers: {havers})")
    if len(havers) >= 2 and not c.get("main_why"):
        out.append("several apps have it: main_why required")
    for a, x in (c.get("apps") or {}).items():
        if apps and a not in apps:
            out.append(f"unknown app '{a}'")
        if x.get("has") in ("yes", "partial") and not x.get("code") and x.get("source") not in ("docs", "ui", "none"):
            out.append(f"{a}: no code locations and no source (docs|ui|none)")
    g = c.get("grade") or {}
    for a in AXES:
        v = g.get(a)
        if v is not None and not (0 <= v <= 10):
            out.append(f"grade.{a} out of range")
    if g.get("overall") is not None and overall(c) is not None and g["overall"] != overall(c):
        out.append("grade.overall must equal min(function, aesthetics, performance)")
    if c.get("status") == "closed" and (overall(c) or 0) < GATE:
        out.append("closed below the gate")
    if c.get("status") == "out" and not c.get("next"):
        out.append("out needs a reason in next")
    return out


def cmd_check(_):
    apps = {a["id"] for a in load(APPS, [])}
    comps = load(COMPS, [])
    if len(apps) < 2:
        print(f"FAIL: {len(apps)} inspiration app(s); minimum two")
    bad = 0
    ids = collections.Counter(c.get("id") for c in comps)
    for i, n in ids.items():
        if n > 1:
            print(f"FAIL {i}: duplicate id"); bad += 1
    for c in comps:
        for p in problems(c, apps):
            print(f"FAIL {c.get('id')}: {p}"); bad += 1
    print(f"{len(comps)} components, {bad} problem(s)")
    sys.exit(1 if bad or len(apps) < 2 else 0)


def key(c):
    o = overall(c)
    return (WEIGHT.get(c.get("weight"), 9), 99 if o is None else o, c.get("id"))


def open_comps(comps):
    return [c for c in comps if c.get("status") not in ("closed", "out") and (overall(c) is None or overall(c) < GATE)]


def cmd_next(_):
    comps = load(COMPS, [])
    prog = [c for c in comps if c.get("status") == "in-progress"]
    if prog:
        c = prog[0]
        print(f"IN PROGRESS {c['id']} ({c.get('feature')}): {c['name']} — overall {overall(c)} — next: {c.get('next','')}")
        return
    rest = sorted(open_comps(comps), key=key)
    if not rest:
        print("Board at parity: every component ≥ 9 or out.")
        return
    c = rest[0]
    g = c.get("grade") or {}
    print(f"NEXT {c['id']} [{c['weight']}] {c['name']}")
    print(f"  grades: function {g.get('function')} · aesthetics {g.get('aesthetics')} · performance {g.get('performance')} → {overall(c)}")
    print(f"  main: {c.get('main')} — {c.get('main_why','')}")
    for a, x in (c.get("apps") or {}).items():
        if x.get("code"):
            print(f"  code {a}: {', '.join(x['code'][:4])}")
    if c.get("take"):
        print(f"  take: {json.dumps(c['take'], ensure_ascii=False)}")
    print(f"  next: {c.get('next','')}")


def cmd_start(a):
    comps = load(COMPS, [])
    for c in comps:
        if c["id"] == a.id:
            c["status"] = "in-progress"; c["feature"] = a.feature
    save(COMPS, comps); render(comps); print(f"{a.id} in progress under {a.feature}")


def cmd_grade(a):
    comps = load(COMPS, [])
    hit = None
    for c in comps:
        if c["id"] == a.id:
            hit = c
    if not hit:
        sys.exit(f"no component {a.id}")
    g = hit.setdefault("grade", {})
    for ax in AXES:
        v = getattr(a, ax)
        if v is not None:
            g[ax] = v
    g["overall"] = overall(hit)
    hit.setdefault("history", []).append({"date": datetime.date.today().isoformat(), "feature": a.feature, "grade": dict(g), "proof": a.proof})
    if g["overall"] is not None and g["overall"] >= GATE:
        hit["status"] = "closed"; hit["next"] = "—"
    elif hit.get("status") == "closed":
        hit["status"] = "open"
    save(COMPS, comps); render(comps)
    print(f"{a.id}: overall {g['overall']} → {hit['status']}")


def cmd_status(_):
    comps = load(COMPS, [])
    st = collections.Counter(c.get("status") for c in comps)
    at = sum(1 for c in comps if (overall(c) or 0) >= GATE)
    print(f"{len(comps)} components · " + ", ".join(f"{k} {v}" for k, v in st.items()) + f" · at ≥{GATE}: {at}")
    dom = collections.defaultdict(list)
    for c in comps:
        if overall(c) is not None:
            dom[c["domain"]].append(overall(c))
    for d, v in sorted(dom.items()):
        print(f"  {d:14s} avg {sum(v)/len(v):.1f}  ≥9 {sum(1 for x in v if x >= GATE)}/{len(v)}")


def cell(s):
    return str(s if s is not None else "").replace("|", "\\|").replace("\n", " ")


def render(comps=None):
    comps = comps if comps is not None else load(COMPS, [])
    apps = load(APPS, [])
    st = collections.Counter(c.get("status") for c in comps)
    at = sum(1 for c in comps if (overall(c) or 0) >= GATE)
    L = ["# Parity board", "",
         f"Inspirations: " + ", ".join(f"{a['name']} ({a.get('version','?')})" for a in apps),
         f"{len(comps)} components · at parity (≥{GATE}): {at} · " + ", ".join(f"{k} {v}" for k, v in st.items()),
         "Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.", ""]
    prog = [c for c in comps if c.get("status") == "in-progress"]
    if prog:
        L += ["## In progress", "| id | feature | component | F · A · P → overall | next |", "|---|---|---|---|---|"]
        for c in prog:
            g = c.get("grade") or {}
            L.append(f"| {c['id']} | {cell(c.get('feature'))} | {cell(c['name'])} | {g.get('function')} · {g.get('aesthetics')} · {g.get('performance')} → **{overall(c)}** | {cell(c.get('next'))} |")
        L.append("")
    L += ["## Queue (next first)", "| # | id | weight | component | main | F · A · P → overall | next |", "|---|---|---|---|---|---|---|"]
    for i, c in enumerate(sorted(open_comps([c for c in comps if c.get("status") != "in-progress"]), key=key)[:60], 1):
        g = c.get("grade") or {}
        L.append(f"| {i} | {c['id']} | {c['weight']} | {cell(c['name'])} | {c.get('main')} | {g.get('function')} · {g.get('aesthetics')} · {g.get('performance')} → **{overall(c)}** | {cell(c.get('next'))} |")
    L += ["", "## By domain", "| domain | components | avg overall | at ≥9 |", "|---|---|---|---|"]
    dom = collections.defaultdict(list)
    for c in comps:
        dom[c.get("domain")].append(overall(c))
    for d, v in sorted(dom.items(), key=lambda x: str(x[0])):
        vv = [x for x in v if x is not None]
        L.append(f"| {d} | {len(v)} | {sum(vv)/len(vv):.1f} | {sum(1 for x in vv if x >= GATE)} |" if vv else f"| {d} | {len(v)} | — | 0 |")
    closed = [c for c in comps if c.get("status") == "closed"]
    if closed:
        L += ["", "## Closed", "| id | component | overall | last proof |", "|---|---|---|---|"]
        for c in closed:
            h = (c.get("history") or [{}])[-1]
            L.append(f"| {c['id']} | {cell(c['name'])} | {overall(c)} | {cell(h.get('proof'))} |")
    OUT.write_text("\n".join(L) + "\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for n in ("init", "check", "render", "next", "status"):
        sub.add_parser(n)
    g = sub.add_parser("grade"); g.add_argument("id")
    for ax in AXES:
        g.add_argument(f"--{ax}", type=int)
    g.add_argument("--proof", required=True); g.add_argument("--feature")
    s = sub.add_parser("start"); s.add_argument("id"); s.add_argument("feature")
    a = ap.parse_args()
    {"init": cmd_init, "check": cmd_check, "render": lambda _: (render(), print(f"wrote {OUT}")), "next": cmd_next,
     "status": cmd_status, "grade": cmd_grade, "start": cmd_start}[a.cmd](a)


if __name__ == "__main__":
    main()
