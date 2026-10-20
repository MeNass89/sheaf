import os,sys,re
root,app,notes=sys.argv[1],sys.argv[2],sys.argv[3]
L=[f'{app}: pretty extraction (English only). Generated 2026-10-21 by shell tools (plutil, strings, nm, swift-demangle, python3).','']
tot_n=tot_s=0
L.append('Subfolders:')
allf=[]
for sub in sorted(os.listdir(root)):
    p=os.path.join(root,sub)
    if not os.path.isdir(p): continue
    n=s=0
    for dp,_,fs in os.walk(p):
        for f in fs:
            fp=os.path.join(dp,f); sz=os.path.getsize(fp); n+=1; s+=sz; allf.append((fp,sz))
    tot_n+=n; tot_s+=s
    L.append(f'  {sub}/  {n} files, {s/1024:.0f} KB')
L.append(f'  TOTAL  {tot_n} files, {tot_s/1048576:.1f} MB'); L.append('')
L.append('Folder meaning:')
L.append('  strings/  .strings/.stringsdict/.loctable (en/Base only) and translation catalogs, as key = value text')
L.append('  nibs/     compiled nibs decoded: objects with titles, key equivalents (mods=cmd/opt/ctrl/shift), actions, identifiers; then all strings')
L.append('  plists/   plists as XML (Info.plist: document types, UTIs, URL schemes, NSServices)')
L.append('  code/     json/html/js/css/md/qml/qmltypes copied with folder structure')
L.append('  symbols/  <bin>.txt = strings -n 6 filtered to UI-label/command-like lines; <bin>.types.txt = ObjC/Swift class names, selectors, Swift property names; *.cpp-methods.txt = demangled C++ class::method names')
L.append('')
def score(fp):
    t=open(fp,encoding='utf-8',errors='replace').read()
    return len(re.findall(r'(?m)(Title=|msgid "|= [A-Z][A-Za-z ]{3,}|qsTr\(|text: |^[A-Z][A-Za-z ]{4,}$)',t))
rich=sorted(((score(fp),fp,sz) for fp,sz in allf if not fp.endswith('README.txt')),reverse=True)[:10]
L.append('10 richest feature-list files (by count of label-like lines):')
for sc,fp,sz in rich: L.append(f'  {sc:6d} labels  {sz/1024:7.0f} KB  {os.path.relpath(fp,root)}')
L.append(''); L.append('Notes:'); L+= ['  '+x for x in notes.split('|')]
open(os.path.join(root,'README.txt'),'w').write('\n'.join(L)+'\n')
print('\n'.join(L))
