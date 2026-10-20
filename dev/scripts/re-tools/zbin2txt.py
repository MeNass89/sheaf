# ZString Binary Format -> "key = value" UTF-8 text
import struct,sys
def decode(path):
    d=open(path,'rb').read()
    assert d.startswith(b'ZString Binary Format'), 'bad magic'
    n,n2,_,kb,vb=struct.unpack_from('<5I',d,0x27)
    T=0x3b
    ko=struct.unpack_from('<%dI'%n,d,T); kl=struct.unpack_from('<%dH'%n,d,T+4*n)
    vo=struct.unpack_from('<%dI'%n,d,T+6*n); vl=struct.unpack_from('<%dH'%n,d,T+10*n)
    ks=T+12*n; vs=ks+kb
    out=[]
    for i in range(n):
        k=d[ks+ko[i]:ks+ko[i]+kl[i]].decode('latin-1')
        v=d[vs+vo[i]:vs+vo[i]+vl[i]].decode('utf-16le')
        v=v.replace('\\','\\\\').replace('\r\n','\\n').replace('\n','\\n').replace('\r','\\n').replace('\t','\\t')
        out.append(f'{k} = {v}')
    return out
src,dst=sys.argv[1],sys.argv[2]
lines=decode(src)
open(dst,'w',encoding='utf-8').write('\n'.join(lines)+'\n')
print(len(lines),dst)
