import struct, sys, re, zlib, os, collections
def qt_hash(s):
    h=0
    for ch in s:
        h=((h<<4)+ord(ch))&0xffffffff
        h^=(h&0xf0000000)>>23
        h&=0x0fffffff
    return h
b=open(sys.argv[1],'rb').read(); out=sys.argv[2]
U16=lambda p: struct.unpack_from('>H',b,p)[0]
U32=lambda p: struct.unpack_from('>I',b,p)[0]
def name_at(p):
    n=U16(p)
    if n==0 or n>300 or p+6+2*n>len(b): return None
    try: s=b[p+6:p+6+2*n].decode('utf-16-be')
    except: return None
    if not s.isprintable(): return None
    if qt_hash(s)!=U32(p+2): return None
    return s
# 1. find name entries
valid={}
for m in re.finditer(rb'(?s)\x00[\x01-\xff].{4}\x00[\x20-\x7e]', b):
    p=m.start(); s=name_at(p)
    if s: valid[p]=s
print('name entries',len(valid))
# group contiguous runs -> names tables
runs=[]; cur=None
for p in sorted(valid):
    if cur and p==cur[1]: cur[1]=p+6+2*len(valid[p]); cur[2].append(p)
    else:
        cur=[p,p+6+2*len(valid[p]),[p]]; runs.append(cur)
runs=[r for r in runs if len(r[2])>=2]
print('name tables',len(runs))
# 2. find trees: root nodes  name=0? Actually root name offset is 0 and flags=2, child offset=1
root_pat=re.compile(rb'\x00\x00\x00\x00\x00\x02(.{4})\x00\x00\x00\x01',re.S)
roots=[m.start() for m in root_pat.finditer(b)]
print('root candidates',len(roots))
# text-entry positions for data base voting
entries=set()
for m in re.finditer(rb'(?s)(.{4})(?=import |pragma |\.pragma|// |/\*|module |module |qmldir|// )',b):
    p=m.start(); n=U32(p)
    if 0<n<5_000_000: entries.add(p)
for m in re.finditer(rb'(?s)(.{4})(.{4})(?=\x78[\x01\x5e\x9c\xda])',b):
    p=m.start(); n=U32(p)
    if 8<n<5_000_000: entries.add(p)
for m in re.finditer(rb'(?s)(.{4})(?=\x28\xb5\x2f\xfd)',b):
    p=m.start(); n=U32(p)
    if 4<n<5_000_000: entries.add(p)
# extend by chaining contiguous [len][data] entries
seeds=sorted(entries); entries=set()
for p in seeds:
    q=p; k=0
    while q not in entries and k<5000 and q+4<=len(b):
        n=U32(q)
        if not (0<n<20_000_000) or q+4+n>len(b): break
        entries.add(q); q=q+4+n; k+=1
print('entry candidates',len(entries))
zstd=None
saved=0; fails=[]; manifest=[]
used_names=[r for r in runs]
for ri,root in enumerate(roots):
    for NS in (22,14):
        # pick names table: try each run, validate children names
        cnt=U32(root+6)
        if cnt==0 or cnt>5000: break
        def node(i): return root+i*NS
        o1=U32(node(1)) if cnt>=1 else 0
        cn=sorted({p-o1 for p in valid if p-o1>=0}, key=lambda x: abs(x-root))[:400]
        chosen=None
        for N in cn:
            files=[]; st={'bad':False}
            def walk(i,path,depth=0):
                if depth>40 or st['bad']: return
                p=node(i)
                if p+NS>len(b): st['bad']=True; return
                nm=name_at(N+U32(p)) if i else ''
                if nm is None: st['bad']=True; return
                flags=U16(p+4); full=(path+'/'+nm) if nm else path
                if flags&2:
                    c=U32(p+6); f=U32(p+10)
                    if c>5000: st['bad']=True; return
                    for k in range(c): walk(f+k,full,depth+1)
                else: files.append((full,flags,U32(p+10)))
            walk(0,'')
            if not st['bad'] and files: chosen=N; break
        if chosen is None:
            fails.append(f'root@{hex(root)} ns={NS}: no names table fits'); continue
        N=chosen
        es=sorted(entries)
        import bisect
        dset=[d for _,_,d in files]
        # vote using a sample of files
        for d in dset[:200]:
            pass
        ds=sorted(set(dset))
        nodes_end=root+NS*(1+max(i for i in range(cnt+1)))
        def validB(B):
            if B<0: return False
            prev_end=None
            for d in ds:
                p=B+d
                if p+4>len(b): return False
                n=U32(p)
                if n>50_000_000 or p+4+n>len(b): return False
                if prev_end is not None and p<prev_end: return False
                prev_end=p+4+n
            for full,flags,d in files[:5]:
                if flags&1:
                    p=B+d; n=U32(p)
                    try: zlib.decompress(b[p+8:p+4+n])
                    except Exception: return False
            return True
        cand=[]
        # names-run end after this tree
        for r in runs:
            for pad in range(0,17): cand.append(r[1]+pad)
        cand += [e for e in entries if root-60_000_000<e<root+60_000_000]
        good=[B for B in cand if validB(B)]
        if not good:
            fails.append(f'root@{hex(root)} ns={NS}: no data base ({len(files)} files)'); break
        # prefer closest to tree
        B=min(good,key=lambda x: abs(x-root))
        for full,flags,d in files:
            p=B+d; n=U32(p); data=b[p+4:p+4+n]
            try:
                if flags&1: data=zlib.decompress(data[4:])
                elif flags&4:
                    if zstd is None: raise RuntimeError('zstd unavailable')
                    data=zstd.decompress(data)
            except Exception as ex:
                fails.append(f'{full}: decompress {ex}'); continue
            manifest.append(f'{full}\t{len(data)}\tflags={flags}')
            ext=os.path.splitext(full)[1].lower()
            if (ext=='.po' and '/i18n/en/' in full) or ext in ('.py','.html','.svg_','.qml','.js','.mjs','.json','.qmltypes','.txt','.md','.html','.css','.xml','.qmldir','') or full.endswith('qmldir'):
                try: t=data.decode('utf-8')
                except: continue
                if '\x00' in t: continue
                dst=os.path.join(out,full.lstrip('/'))
                os.makedirs(os.path.dirname(dst),exist_ok=True)
                if not os.path.exists(dst): open(dst,'w',encoding='utf-8').write(t); saved+=1
        break
open(os.path.join(out,'_rcc_manifest.tsv'),'w').write('\n'.join(sorted(set(manifest)))+'\n')
print('saved',saved,'manifest',len(set(manifest)))
print('\n'.join(fails[:30]))
