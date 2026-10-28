#!/usr/bin/env python3
"""Brings crates/sheaf-app/translations/fr/LC_MESSAGES/sheaf-ui.po up to date with the interface: every @tr
text of ui/, every i18n::tr("…") of src/, the Tools page's names and sections. French already written is kept;
new texts get an empty msgstr to fill in (the unit test i18n::every_interface_text_has_its_french fails until
they are). Usage: python3 dev/scripts/po-update.py crates/sheaf-app"""
import glob, re, sys, importlib.util, os
crate = sys.argv[1]
PO = crate + '/translations/fr/LC_MESSAGES/sheaf-ui.po'

def read_po(path):
    out, mid, cur = {}, None, None
    for line in open(path).read().split('\n') if os.path.exists(path) else []:
        if line.startswith('msgid '): mid, cur = dec(line[7:-1]), 'id'
        elif line.startswith('msgstr '): out[mid] = dec(line[8:-1]); cur = 'str'
        elif line.startswith('"') and cur == 'str': out[mid] += dec(line[1:-1])
        elif line.startswith('"') and cur == 'id': mid += dec(line[1:-1])
    return out

def dec(s):
    return re.sub(r'\\(.)', lambda m: {'n': '\n', 't': '\t'}.get(m.group(1), m.group(1)), s)

def msgs(src):
    out = []; rest = src
    while '@tr("' in rest:
        i = rest.index('@tr("'); rest = rest[i + 5:]; e = 0
        while not (rest[e] == '"' and rest[e - 1] != '\\'): e += 1
        out.append(dec(rest[:e])); rest = rest[e:]
    return out

FR = read_po(PO)
seen = []
def add(m, where):
    for s, w in seen:
        if s == m:
            w.add(where); return
    seen.append((m, {where}))
for f in sorted(glob.glob(crate + '/ui/**/*.slint', recursive=True)):
    for m in msgs(open(f).read()):
        add(m, os.path.relpath(f, crate))
for f in sorted(glob.glob(crate + '/src/**/*.rs', recursive=True)):
    if f.endswith('i18n.rs'): continue
    for m in msgs(open(f).read().replace('i18n::tr("', '@tr("')):
        add(m, os.path.relpath(f, crate))
core = open(crate + '/../sheaf-core/src/tools.rs').read()
i = core.index('pub fn label')
for m in re.findall(r'=> "([^"]+)"', core[i:i + 600]): add(m, '../sheaf-core/src/tools.rs')
for m in re.findall(r'tool!\(\w+, "([^"]+)"', core): add(m, '../sheaf-core/src/tools.rs')

def q(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n') + '"'
out = ['# French for Sheaf\'s interface. Brought up to date by dev/scripts/po-update.py; translate the empty msgstr.',
       'msgid ""', 'msgstr ""', '"Project-Id-Version: Sheaf\\n"', '"Language: fr\\n"', '"MIME-Version: 1.0\\n"',
       '"Content-Type: text/plain; charset=UTF-8\\n"', '"Content-Transfer-Encoding: 8bit\\n"',
       '"Plural-Forms: nplurals=2; plural=(n > 1);\\n"', '']
missing = []
for m, where in seen:
    t = FR.get(m)
    if t is None: missing.append(m); t = ''
    out += ['#: ' + ' '.join(sorted(where)), 'msgid ' + q(m), 'msgstr ' + q(t), '']
open(PO, 'w').write('\n'.join(out))
print(len(seen), 'messages,', len(missing), 'without French')
for m in missing: print('MISSING', repr(m))
