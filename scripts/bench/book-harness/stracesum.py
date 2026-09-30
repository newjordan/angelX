import re, sys, collections
opens=collections.Counter(); wbytes=collections.Counter(); wcalls=collections.Counter()
execs=collections.Counter(); syncs=collections.Counter(); renames=collections.Counter()
fdmap={}
def norm(p):
    p=re.sub(r'/tmp/angel-bench-(ws|home)-\w+', r'<\1>', p)
    p=re.sub(r'\d{6,}', 'N', p)
    return p
for line in open(sys.argv[1]):
    m=re.match(r'(\d+)\s+\S+\s+(.*)', line)
    if not m: continue
    pid, rest = m.groups()
    if rest.startswith('openat('):
        mm=re.search(r'openat\([^,]+, "([^"]*)".*= (\d+)$', rest)
        if mm:
            path, fd = mm.groups(); fdmap[(pid,fd)]=norm(path); opens[norm(path)]+=1
    elif rest.startswith(('write(','pwrite64(')):
        mm=re.match(r'p?write(?:64)?\((\d+),.*= (\d+)$', rest)
        if mm:
            fd,n=mm.groups(); p=fdmap.get((pid,fd), f'fd{fd}'); wbytes[p]+=int(n); wcalls[p]+=1
    elif rest.startswith('execve('):
        mm=re.search(r'execve\("([^"]*)"', rest); execs[mm.group(1) if mm else '?']+=1
    elif rest.startswith(('fsync','fdatasync')):
        mm=re.match(r'\w+\((\d+)', rest); syncs[fdmap.get((pid,mm.group(1)),'?')]+=1
    elif rest.startswith('rename('):
        mm=re.search(r'rename\("[^"]*", "([^"]*)"', rest); renames[norm(mm.group(1)) if mm else '?']+=1
print("== bytes written by path (top 25)")
for p,n in wbytes.most_common(25): print(f"{n:>12,} {wcalls[p]:>6} {p}")
print("== opens (top 25)")
for p,n in opens.most_common(25): print(f"{n:>6} {p}")
print("== execs"); [print(f"{n:>6} {p}") for p,n in execs.most_common(15)]
print("== fsyncs"); [print(f"{n:>6} {p}") for p,n in syncs.most_common(10)]
print("== renames"); [print(f"{n:>6} {p}") for p,n in renames.most_common(10)]
