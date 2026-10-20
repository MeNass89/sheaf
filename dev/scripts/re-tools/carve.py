import struct,re,zlib
b=open('/Applications/iLovePDF.app/Contents/MacOS/iLovePDF','rb').read()
U32=lambda p: struct.unpack_from('>I',b,p)[0]
blobs=[]
for m in re.finditer(rb'(?:import |pragma |\.pragma |// |/\*)',b):
    p=m.start()-4
    n=U32(p)
    if 10<n<2_000_000 and p+4+n<=len(b):
        t=b[p+4:p+4+n]
        if b'\x00' not in t: blobs.append((p,n))
blobs=sorted(set(blobs))
print('plain blobs',len(blobs), sum(n for _,n in blobs))
z=0
for m in re.finditer(rb'(?s)(.{4})(.{4})\x78[\x01\x5e\x9c\xda]',b):
    p=m.start(); n=U32(p)
    if 8<n<5_000_000 and p+4+n<=len(b):
        try:
            d=zlib.decompress(b[p+8:p+4+n])
            if U32(p+4)==len(d) and (d.lstrip().startswith((b'import',b'pragma',b'//',b'/*',b'.pragma'))): z+=1
        except: pass
print('zlib qml blobs',z)
print(b[blobs[0][0]-80:blobs[0][0]+8])
