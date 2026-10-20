# Count distinct UI labels per file under sources/pretty/acrobat
import os,re,sys,json
root=sys.argv[1]
eng=re.compile(r"^[A-Z&][A-Za-z&'’,.:()/\-]*( [A-Za-z0-9&'’,.:()/%\-\"…?!]+)*[.…?!:]?$")
def labels(path):
    t=open(path,encoding='utf-8',errors='replace').read()
    rel=os.path.relpath(path,root); s=set()
    if rel.startswith('strings/') and rel.endswith('.txt') and ' = ' in t[:2000]:
        for l in t.splitlines():
            if ' = ' in l:
                v=l.split(' = ',1)[1].strip()
                if re.search('[A-Za-z]{2}',v): s.add(v)
    elif rel.startswith('aapp/'):
        s|=set(v for v in re.findall(r'(?:title|label|tooltip|description|richTooltip|av2Title|av2Description|name)="([^"]+)"',t,re.I) if re.search('[A-Za-z]',v))
    elif rel.endswith('.json'):
        def walk(o):
            if isinstance(o,dict): [walk(v) for v in o.values()]
            elif isinstance(o,list): [walk(v) for v in o]
            elif isinstance(o,str) and re.search('[A-Za-z]{2}',o): s.add(o)
        walk(json.loads(t))
    elif rel.endswith(('.strings','.strings.txt')) and '" = "' in t:
        s|=set(re.findall(r'"\s*=\s*"((?:[^"\\]|\\.)*)"',t))
    elif rel.endswith('.asfx'):
        s|=set(re.findall(r'<val>([^<]+)</val>',t))
    elif rel.endswith(('.eve','.exv')):
        s|=set(re.findall(r"\$\$\$[^'\"]+",t))
    elif rel.endswith('.sdef'):
        s|=set(re.findall(r'(?:name|description)="([^"]+)"',t))
    else:
        for l in t.splitlines():
            l=l.strip()
            if 3<=len(l)<=120 and eng.match(l) and ' ' in l: s.add(l)
    return len(s)
res=[]
for dp,_,fs in os.walk(root):
    for f in fs:
        if f=='README.txt': continue
        p=os.path.join(dp,f); res.append((labels(p),os.path.relpath(p,root)))
res.sort(reverse=True)
for c,p in res: print(c,p,sep='\t')
