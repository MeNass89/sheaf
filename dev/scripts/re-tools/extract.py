import os, sys, nibarchive, plistlib, subprocess, shutil, json
app, out = sys.argv[1], sys.argv[2]
root = os.path.join(app, 'Contents')
EN = {'en.lproj','English.lproj','Base.lproj','en_US.lproj','en_GB.lproj'}
CODE = {'.json','.html','.htm','.js','.css','.qml','.qmltypes','.md','.txt','.xml','.svg_'}
CODE = {'.json','.html','.htm','.js','.css','.qml','.qmltypes','.md'}
log = []
stats = {}
def lang_ok(rel):
    parts = rel.split('/')
    lp = [p for p in parts if p.endswith('.lproj')]
    return all(p in EN for p in lp)
def load_plist(path):
    try:
        with open(path,'rb') as f: return plistlib.load(f)
    except Exception:
        r = subprocess.run(['plutil','-convert','xml1','-o','-',path],capture_output=True)
        if r.returncode: raise RuntimeError(r.stderr.decode(errors='replace').strip())
        return plistlib.loads(r.stdout)
def fmt(obj, ind=0):
    pad='  '*ind; lines=[]
    if isinstance(obj, dict):
        for k,v in obj.items():
            if isinstance(v,(dict,list)):
                lines.append(f'{pad}{k}:'); lines += fmt(v, ind+1)
            else: lines.append(f'{pad}{k} = {v!r}' if not isinstance(v,str) else f'{pad}{k} = {v}')
    elif isinstance(obj, list):
        for v in obj:
            if isinstance(v,(dict,list)): lines.append(f'{pad}-'); lines += fmt(v, ind+1)
            else: lines.append(f'{pad}- {v}')
    else: lines.append(f'{pad}{obj}')
    return lines
def w(sub, rel, text):
    p = os.path.join(out, sub, rel); os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p,'w',encoding='utf-8') as f: f.write(text)
    stats[sub] = stats.get(sub,0)+1
def nib_text(path):
    b=open(path,'rb').read()
    if b[:10]==b'NIBArchive':
        try: return nibarchive.text(b)
        except Exception as ex: log.append(f'NIBArchive decode FAIL {path}: {ex}')
    try:
        d = load_plist(path)
        objs = d.get('$objects') if isinstance(d,dict) else None
        if objs is not None:
            seen=[]; s=set()
            for o in objs:
                if isinstance(o,str) and o not in s and o!='$null': s.add(o); seen.append(o)
            return '\n'.join(seen)
        return '\n'.join(fmt(d))
    except Exception:
        r = subprocess.run(['strings','-n','3',path],capture_output=True)
        return r.stdout.decode('utf-8',errors='replace')
for dp, dns, fns in os.walk(root, followlinks=False):
    rdp = os.path.relpath(dp, root)
    # nib directories
    for dn in list(dns):
        full=os.path.join(dp,dn)
        if os.path.islink(full): dns.remove(dn); continue
        if dn.endswith('.nib') and os.path.isdir(full):
            dns.remove(dn)
            rel=os.path.normpath(os.path.join(rdp,dn))
            if not lang_ok(rel): continue
            parts=[]
            for f in sorted(os.listdir(full)):
                fp=os.path.join(full,f)
                if os.path.isfile(fp): parts.append(f'### {f}\n'+nib_text(fp))
            w('nibs', rel+'.txt', '\n\n'.join(parts))
    for fn in fns:
        full=os.path.join(dp,fn)
        if os.path.islink(full): continue
        rel=os.path.normpath(os.path.join(rdp,fn)); ext=os.path.splitext(fn)[1].lower()
        try:
            if ext in ('.strings','.stringsdict'):
                if not lang_ok(rel): continue
                d=load_plist(full); w('strings', rel+'.txt', '\n'.join(fmt(d))+'\n')
            elif ext=='.loctable':
                d=load_plist(full)
                key=next((k for k in ('en','en_US','Base','en_GB') if k in d), None)
                if key is None: log.append(f'loctable without en: {rel} keys={list(d)[:5]}'); continue
                w('strings', rel+f'.{key}.txt', '\n'.join(fmt(d[key]))+'\n')
            elif ext=='.nib':
                if not lang_ok(rel): continue
                w('nibs', rel+'.txt', nib_text(full))
            elif ext=='.plist':
                if not lang_ok(rel): continue
                p=os.path.join(out,'plists',rel[:-6]+'.xml'); os.makedirs(os.path.dirname(p),exist_ok=True)
                r=subprocess.run(['plutil','-convert','xml1','-o',p,full],capture_output=True)
                if r.returncode: log.append(f'plist FAIL {rel}: {r.stderr.decode().strip()}')
                else: stats['plists']=stats.get('plists',0)+1
            elif ext in CODE:
                if not lang_ok(rel): continue
                p=os.path.join(out,'code',rel); os.makedirs(os.path.dirname(p),exist_ok=True)
                shutil.copyfile(full,p); stats['code']=stats.get('code',0)+1
        except Exception as e:
            log.append(f'FAIL {ext} {rel}: {e}')
print(json.dumps(stats)); print('\n'.join(log))
