import struct, sys, subprocess
def slices(b):
    m=struct.unpack_from('>I',b,0)[0]
    if m in (0xcafebabe,):
        n=struct.unpack_from('>I',b,4)[0]; out=[]
        for i in range(n):
            ct,cs,off,sz,al=struct.unpack_from('>5I',b,8+20*i); out.append((ct,off,sz))
        return out
    return [(0,0,len(b))]
def sections(b,base):
    magic=struct.unpack_from('<I',b,base)[0]
    assert magic==0xfeedfacf, hex(magic)
    ncmds=struct.unpack_from('<I',b,base+16)[0]; p=base+32; res={}
    for _ in range(ncmds):
        cmd,size=struct.unpack_from('<II',b,p)
        if cmd==0x19:
            nsect=struct.unpack_from('<I',b,p+64)[0]; q=p+72
            for _ in range(nsect):
                sect=b[q:q+16].rstrip(b'\0').decode(); seg=b[q+16:q+32].rstrip(b'\0').decode()
                addr,size_,off=struct.unpack_from('<QQI',b,q+32)
                res[(seg,sect)]=(base+off,size_); q+=80
        p+=size
    return res
b=open(sys.argv[1],'rb').read()
sl=slices(b)
pick=[s for s in sl if s[0] in (0x0100000c,)] or sl
base=pick[0][1]
secs=sections(b,base)
want=sys.argv[2].split(',')
for (seg,sect),(off,sz) in secs.items():
    if sect in want:
        for s in b[off:off+sz].split(b'\0'):
            if len(s)>=3:
                try: print(f'[{sect}] '+s.decode('utf-8'))
                except: pass
