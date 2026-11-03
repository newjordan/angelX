#!/usr/bin/env python3
"""Prototype of an exact literal-only Rust book page converter."""
from pathlib import Path
import re
DIGITS='⠁⠃⠉⠙⠑⠋⠛⠓⠊⠚'
MARKER='@@book-page '
class Unsupported(ValueError): pass

def quoted(s,i):
    assert s[i]=='"'
    out=[]; i+=1
    while i<len(s):
        c=s[i]; i+=1
        if c=='"': return ''.join(out),i
        if c!='\\': out.append(c); continue
        if i>=len(s): raise Unsupported('unfinished escape')
        c=s[i]; i+=1
        simple={'n':'\n','r':'\r','t':'\t','0':'\0','\\':'\\','"':'"',"'":"'"}
        if c in simple: out.append(simple[c]); continue
        if c=='x':
            x=s[i:i+2]
            if not re.fullmatch('[0-9a-fA-F]{2}',x) or int(x,16)>127: raise Unsupported('bad ASCII escape')
            out.append(chr(int(x,16))); i+=2; continue
        if c=='u' and i<len(s) and s[i]=='{':
            end=s.find('}',i+1); raw=s[i+1:end].replace('_','')
            if end<0 or not re.fullmatch('[0-9a-fA-F]{1,6}',raw): raise Unsupported('bad Unicode escape')
            cp=int(raw,16)
            if cp>0x10ffff or 0xd800<=cp<=0xdfff: raise Unsupported('bad Unicode scalar')
            out.append(chr(cp)); i=end+1; continue
        if c=='\r' and i<len(s) and s[i]=='\n': i+=1; c='\n'
        if c=='\n':
            while i<len(s) and s[i] in ' \t\r\n': i+=1
            continue
        raise Unsupported('unknown escape '+repr(c))
    raise Unsupported('unterminated string')

def space(s,i):
    while i<len(s):
        if s[i].isspace() or s[i]==',': i+=1; continue
        if s.startswith('//',i):
            n=s.find('\n',i); i=len(s) if n<0 else n+1; continue
        if s.startswith('/*',i):
            depth=1; i+=2
            while i<len(s) and depth:
                if s.startswith('/*',i): depth+=1; i+=2
                elif s.startswith('*/',i): depth-=1; i+=2
                else: i+=1
            if depth: raise Unsupported('unterminated comment')
            continue
        break
    return i

def closing(s,start):
    stack=[s[start]]; pairs={'}':'{',']':'[',')':'('}; i=start+1
    while i<len(s):
        if s.startswith('//',i) or s.startswith('/*',i): i=space(s,i); continue
        c=s[i]
        if c=='"': _,i=quoted(s,i); continue
        if c=="'":
            n=s.find("'",i+1)
            if n<0: raise Unsupported('unterminated character')
            i=n+1; continue
        if c in '{[(': stack.append(c)
        elif c in '}])':
            if not stack or stack.pop()!=pairs[c]: raise Unsupported('mismatched delimiter')
            if not stack: return i
        i+=1
    raise Unsupported('unterminated delimiter')

def pages(body):
    m=re.search(r'\bpages\s*:\s*&\s*\[',body)
    if not m: raise Unsupported('nonliteral or absent pages array')
    opening=m.end()-1; end=closing(body,opening); values=[]; i=opening+1
    while True:
        i=space(body,i)
        if i==end: break
        if i>end or body[i]!='"': raise Unsupported('dynamic page expression')
        value,i=quoted(body,i)
        if not value: raise Unsupported('empty page')
        if any(line.startswith(MARKER) for line in value.split('\n')): raise Unsupported('marker collision')
        values.append(value)
    if len(values)>10: raise Unsupported('too many pages')
    return m.start(),end+1,values

def decode(catalog):
    result=[]; digit=None; chunks=[]
    # Split only LF, not Unicode separators: all other bytes belong to pages.
    lines=catalog.split('\n')
    if lines[-1]!='': raise Unsupported('missing record separator')
    for line in lines[:-1]:
        if line.startswith(MARKER):
            m=re.fullmatch(r'@@book-page ([0-9])@@',line)
            if not m: raise Unsupported('malformed marker')
            if digit is not None: result.append((digit,'\n'.join(chunks)))
            digit=m.group(1); chunks=[]
        else:
            if digit is None: raise Unsupported('page before marker')
            chunks.append(line)
    if digit is not None: result.append((digit,'\n'.join(chunks)))
    return result

def convert(source,chapter):
    if chapter=='d3456_advisories': raise Unsupported('protected chapter')
    if not re.fullmatch('[a-z0-9_]+',chapter): raise Unsupported('invalid chapter')
    # A chapter may already use CATALOG as a Route constant (u_skills does).
    # Pick a distinct source identifier without changing that public symbol.
    source_ident = 'CATALOG'
    if re.search(r'\b(?:const|static)\s+CATALOG\b', source):
        source_ident = 'BOOK_PAGES_CATALOG'
        if re.search(r'\b(?:const|static)\s+BOOK_PAGES_CATALOG\b', source):
            raise Unsupported('catalog source identifier already in use')
    constants={}
    pattern=r"\bconst\s+(\w+)\s*:\s*Route\s*=\s*Route::new\(\s*CELL\s*,\s*'([^']+)'\s*\)"
    for m in re.finditer(pattern,source): constants[m.group(1)]=m.group(2)
    records=[]; replacements=[]; used=set()
    for match in re.finditer(r'\bSub\s*\{',source):
        opening=match.end()-1; end=closing(source,opening); body=source[opening+1:end]
        route=re.search(r"\broute\s*:\s*(?:Route::new\(\s*CELL\s*,\s*'([^']+)'\s*\)|(\w+))",body)
        if not route: raise Unsupported('unsupported route expression')
        cell=route.group(1) or constants.get(route.group(2))
        if cell is None or cell not in DIGITS or cell in used: raise Unsupported('invalid or duplicate section')
        used.add(cell); digit=str((DIGITS.index(cell)+1)%10)
        begin,finish,entries=pages(body)
        if entries:
            records.extend((digit,text) for text in entries)
            replacements.append((opening+1+begin,opening+1+finish,f"pages: &super::catalog_pages::<{len(entries)}>({source_ident}, b'{digit}')"))
    if not replacements: raise Unsupported('no nonempty literal pages')
    output=source
    for start,end,value in reversed(replacements): output=output[:start]+value+output[end:]
    m=re.search(r'^use\s+super::\{([^}]+)\};[ \t]*$',output,re.M)
    if not m or not {'Primary','Route','Sub'}.issubset({part.strip() for part in m.group(1).split(',')}):
        raise Unsupported('chapter import layout differs')
    insertion=f'\n\nconst {source_ident}: &str = include_str!("../../../../book/{chapter}.book");'
    output=output[:m.end()]+insertion+output[m.end():]
    catalog=''.join(f'{MARKER}{digit}@@\n{text}\n' for digit,text in records)
    if decode(catalog)!=records: raise AssertionError('catalog failed roundtrip')
    return output,catalog,records

# The vendor profile is one Rust source with four independent book cells. Each
# shelf needs its own asset and manifest identity because page digits restart
# at each cell. The mapping is explicit so an unrelated multi-cell chapter is
# never guessed into this publication format.
CODEX_SHELVES = {
    'CELL': ('PRIMARY', 'codex_sol_primary.book'),
    'SHELF_II_CELL': ('SHELF_II', 'codex_sol_shelf_ii.book'),
    'SHELF_III_CELL': ('SHELF_III', 'codex_sol_shelf_iii.book'),
    'CONTEXT_CELL': ('CONTEXT', 'codex_sol_context.book'),
}

def convert_shelves(source, shelves, prefix='PROFILE_'):
    constants = {}
    cell_names = '|'.join(re.escape(name) for name in shelves)
    pattern = rf"\bconst\s+(\w+)\s*:\s*Route\s*=\s*Route::new\(\s*({cell_names})\s*,\s*'([^']+)'\s*\)"
    for match in re.finditer(pattern, source):
        constants[match.group(1)] = (match.group(2), match.group(3))
    records = {cell: [] for cell in shelves}
    replacements = []
    used = set()
    for match in re.finditer(r'\bSub\s*\{', source):
        opening = match.end() - 1
        end = closing(source, opening)
        body = source[opening + 1:end]
        route = re.search(r'\broute\s*:\s*(\w+)', body)
        if not route or route.group(1) not in constants:
            raise Unsupported('unsupported multi-shelf route expression')
        cell, section = constants[route.group(1)]
        if section not in DIGITS or (cell, section) in used:
            raise Unsupported('invalid or duplicate multi-shelf section')
        used.add((cell, section))
        digit = str((DIGITS.index(section) + 1) % 10)
        begin, finish, values = pages(body)
        records[cell].extend((digit, value) for value in values)
        if values:
            shelf, _ = shelves[cell]
            replacements.append((opening + 1 + begin, opening + 1 + finish,
                f"pages: &super::catalog_pages::<{len(values)}>({prefix}{shelf}, b'{digit}')"))
    if not replacements or any(not values for values in records.values()):
        raise Unsupported('missing multi-shelf page records')
    output = source
    for start, end, replacement in reversed(replacements):
        output = output[:start] + replacement + output[end:]
    imported = re.search(r'^use\s+super::\{([^}]+)\};[ \t]*$',output,re.M)
    if (not imported
        or not {'Primary','Route','Sub'}.issubset({part.strip() for part in imported.group(1).split(',')})
        or re.search(r'\b(?:const|static)\s+' + re.escape(prefix), output)):
        raise Unsupported('multi-shelf source layout differs')
    imports = '\n'.join(
        f'const {prefix}{shelf}: &str = include_str!("../../../../book/{name}");'
        for shelf, name in shelves.values()
    )
    output = output[:imported.end()] + '\n' + imports + output[imported.end():]
    assets = {}
    for cell, values in records.items():
        name = shelves[cell][1]
        catalog = ''.join(f'{MARKER}{digit}@@\n{text}\n' for digit, text in values)
        if decode(catalog) != values:
            raise AssertionError('multi-shelf catalog failed roundtrip')
        assets[name] = catalog
    return output, assets, records

def convert_codex_shelves(source):
    return convert_shelves(source, CODEX_SHELVES)

CHAPTER_SHELVES = {
    'labyrinth_campaign': {
        'CELL': ('PRIMARY', 'labyrinth_primary.book'),
        'SHELF_CELL': ('SHELF', 'labyrinth_shelf.book'),
    },
    'k_competition': {
        'CELL': ('PRIMARY', 'competition_primary.book'),
        'SHELF_CELL': ('SHELF', 'competition_shelf.book'),
    },
    'st_connected': {
        'CELL': ('PRIMARY', 'connected_primary.book'),
        'SHELF_CELL': ('SHELF', 'connected_shelf.book'),
    },
}

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description='Stage literal-only book chapter conversions; review and verify compiled parity before use.')
    parser.add_argument('chapters', nargs='+', help='unlocked chapter file stems')
    parser.add_argument('--output-dir', type=Path, help='stage converted Rust and book files in an empty directory')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2] / 'cockpit/src/agent/harness/book'
    staged = []
    for name in args.chapters:
        if name == 'd3456_advisories' or not re.fullmatch('[a-z0-9_]+', name):
            parser.exit(1, f'{name}: protected or invalid chapter name\n')
        try:
            source = (root / (name + '.rs')).read_text(encoding='utf-8')
            if name == 'd23457_codex_sol':
                converted, assets, records = convert_codex_shelves(source)
                count = sum(len(rows) for rows in records.values())
            elif name in CHAPTER_SHELVES:
                converted, assets, records = convert_shelves(source, CHAPTER_SHELVES[name], 'BOOK_')
                count = sum(len(rows) for rows in records.values())
            else:
                converted, catalog, records = convert(source, name)
                assets = {name + '.book': catalog}
                count = len(records)
        except (OSError, Unsupported) as exc:
            parser.exit(1, f'{name}: conversion refused: {exc}\n')
        staged.append((name, converted, assets))
        print(f'{name}: {count} pages; Rust lines {len(source.splitlines())} -> {len(converted.splitlines())}; {sum(len(data.encode()) for data in assets.values())} data bytes')
    if args.output_dir:
        args.output_dir.mkdir(parents=True, exist_ok=True)
        targets = []
        for name, rust, assets in staged:
            targets.append((args.output_dir / (name + '.rs'), rust))
            targets.extend((args.output_dir / asset, content) for asset, content in assets.items())
        if any(path.exists() for path, _ in targets):
            parser.exit(1, 'output already exists; no files were written\n')
        created = []
        try:
            for path, content in targets:
                with path.open('x', encoding='utf-8', newline='') as handle:
                    created.append(path)
                    handle.write(content)
        except OSError:
            for path in created:
                path.unlink(missing_ok=True)
            raise
