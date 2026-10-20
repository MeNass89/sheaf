import struct, sys
def varint(b, i):
    r=0; s=0
    while True:
        c=b[i]; i+=1; r|=(c&0x7f)<<s; s+=7
        if c&0x80: return r,i
def parse(b):
    assert b[:10]==b'NIBArchive'
    h=struct.unpack('<10I', b[10:50])
    _,_,oc,oo,kc,ko,vc,vo,cc,co=h
    keys=[]; i=ko
    for _ in range(kc):
        n,i=varint(b,i); keys.append(b[i:i+n].decode('utf-8','replace')); i+=n
    classes=[]; i=co
    for _ in range(cc):
        n,i=varint(b,i); extra,i=varint(b,i); i+=4*extra
        classes.append(b[i:i+n].rstrip(b'\0').decode('utf-8','replace')); i+=n
    vals=[]; i=vo
    for _ in range(vc):
        k,i=varint(b,i); t=b[i]; i+=1
        if t==0: v=b[i]; i+=1
        elif t==1: v=struct.unpack('<h',b[i:i+2])[0]; i+=2
        elif t==2: v=struct.unpack('<i',b[i:i+4])[0]; i+=4
        elif t==3: v=struct.unpack('<q',b[i:i+8])[0]; i+=8
        elif t==4: v=True
        elif t==5: v=False
        elif t==6: v=struct.unpack('<f',b[i:i+4])[0]; i+=4
        elif t==7: v=struct.unpack('<d',b[i:i+8])[0]; i+=8
        elif t==8:
            n,i=varint(b,i); v=b[i:i+n]; i+=n
        elif t==9: v=None
        elif t==10: v=('ref',struct.unpack('<I',b[i:i+4])[0]); i+=4
        else: raise ValueError(f'type {t}')
        vals.append((keys[k],v))
    objs=[]; i=oo
    for _ in range(oc):
        ci,i=varint(b,i); vi,i=varint(b,i); n,i=varint(b,i)
        objs.append((classes[ci], vals[vi:vi+n]))
    return objs
def text(b):
    objs=parse(b)
    def sval(idx):
        cls,vs=objs[idx]
        for k,v in vs:
            if k=='NS.bytes' and isinstance(v,bytes):
                try: return v.decode('utf-8')
                except: return None
        return None
    INTEREST={'NSTitle','NSKeyEquiv','NSAction','NSContents','NSPlaceholderString','NSToolTip','NSLabel','NSPaletteLabel','NSToolbarItemIdentifier','NSIdentifier','NSMenuItemIdentifier','NSKeyPath','NSAttributedTitle','NSClassName','NSHelpTag','NSName','NSString','NSTextContainer','NSSelectorName','IBModuleName','NSBinding','NSHeaderToolTip','NSIdent'}
    out=[]; seen=set()
    for idx,(cls,vs) in enumerate(objs):
        if cls in ('NSString','NSMutableString','NSLocalizableString'): continue
        parts=[]
        for k,v in vs:
            s=None
            if isinstance(v,tuple):
                if k in INTEREST or k.startswith('NS') and 'Title' in k or 'Label' in k or 'ToolTip' in k or 'Identifier' in k or 'KeyEquiv' in k or 'Action' in k or 'Placeholder' in k:
                    s=sval(v[1])
            elif isinstance(v,bytes) and (k in INTEREST):
                try: s=v.decode('utf-8')
                except: pass
            if k=='NSKeyEquivModMask' and isinstance(v,int):
                m=[n for bit,n in ((1<<17,'shift'),(1<<18,'ctrl'),(1<<19,'opt'),(1<<20,'cmd')) if v&bit]
                parts.append('mods='+'+'.join(m)); continue
            if s is not None and s.strip():
                if k=='NSKeyEquiv': s=repr(s)
                parts.append(f'{k}={s}')
        if parts:
            line=f'[{cls}] '+' | '.join(parts)
            if line not in seen: seen.add(line); out.append(line)
    # all distinct strings too
    strs=[]; ss=set()
    for idx,(cls,vs) in enumerate(objs):
        if cls in ('NSString','NSMutableString','NSLocalizableString'):
            s=sval(idx)
            if s and s not in ss: ss.add(s); strs.append(s)
    return '## Objects with UI properties\n'+'\n'.join(out)+'\n\n## All distinct strings\n'+'\n'.join(strs)+'\n'
if __name__=='__main__':
    print(text(open(sys.argv[1],'rb').read()))
