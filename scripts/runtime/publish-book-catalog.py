#!/usr/bin/env python3
"""Prototype atomic manifest publisher for editable book chapter sources."""
import argparse, hashlib, json, os, stat, tempfile
from pathlib import Path

EXPECTED={
 'gh_navigation.tsv':('⠣',(1,7,9,3,9,1,4,0,0,0)),
 'ch_edits.book':('⠡',(6,7,5,3,5,4,2,3,4,3)),
 'sh_shell.book':('⠩',(10,4,4,8,9,4,5,10,10,2)),
 'th_team.book':('⠹',(4,10,8,3,10,10,4,4,2,10)),
 'wh_world.book':('⠱',(6,6,3,4,2,6,1,1,3,7)),
 'ed_media.book':('⠫',(5,2,4,10,5,5,5,6,8,7)),
 'and_session.book':('⠯',(4,5,1,3,1,1,2,0,0,0)),
 'the_sight.book':('⠮',(6,6,6,1,2,2,2,2,0,0)),
 'of_patches.book':('⠷',(2,5,4,3,6,1,0,0,0,0)),
 'with_build.book':('⠾',(9,4,1,1,4,6,1,0,0,0)),
 't_personality.book':('⠞',(2, 2, 4, 2, 3, 10, 3, 3, 1, 1)),
 'z_brevity.book':('⠵',(1, 1, 2, 3, 1, 5, 1, 3, 1, 3)),
 'ou_checkpoints.book':('⠳',(4, 4, 1, 4, 1, 2, 1, 3, 8, 1)),
 'd4_angles.book':('⠈',(3, 3, 2, 2, 3, 3, 8, 4, 2, 5)),
 'o_orchestration.book':('⠕',(2, 1, 4, 2, 4, 2, 1, 1, 7, 6)),
 'd56_replies.book':('⠰',(4, 7, 10, 10, 9, 9, 10, 5, 5, 4)),
 'ar_seats.book':('⠜',(3, 5, 4, 1, 3, 1, 5, 1, 2, 5)),
 'd4567_briefs.book':('⡸',(9, 9, 6, 3, 5, 6, 4, 0, 0, 0)),
 'ing_drivers.book':('⠬',(6, 8, 2, 5, 4, 7, 9, 4, 2, 2)),
 'd456_knowledge.book':('⠸',(5, 7, 2, 7, 9, 7, 3, 3, 4, 7)),
 'd5_frames.book':('⠐',(4, 4, 5, 7, 4, 2, 9, 3, 8, 4)),
 'n_environment.book':('⠝',(3, 3, 3, 4, 5, 6, 2, 8, 2, 1)),
 'ow_ledgers.book':('⠪',(1, 1, 1, 1, 1, 7, 2, 5, 8, 10)),
 'm_method.book':('⠍',(6, 7, 4, 5, 4, 3, 5, 7, 3, 9)),
 'r_relentless.book':('⠗',(4, 10, 6, 1, 1, 1, 2, 2, 3, 7)),
 'u_skills.book':('⠥',(2, 2, 3, 1, 1, 1, 2, 2, 4, 2)),
 'd467_receipts.book':('⡨',(7, 10, 5, 0, 0, 0, 0, 0, 0, 0)),
 'd45_iteration.book':('⠘',(6, 7, 5, 4, 4, 5, 10, 6, 10, 10)),
 'd6_long_run.book':('⠠',(4, 7, 8, 8, 6, 7, 7, 9, 9, 9)),
 'er_loop.book':('⠻',(7, 3, 2, 4, 3, 1, 1, 2, 3, 6)),
 's_sources.book':('⠎',(0, 3, 1, 1, 1, 4, 2, 3, 1, 1)),
 'd3_roles.book':('⠄',(7, 3, 3, 6, 3, 4, 6, 3, 3, 5)),
 'y_types.book':('⠽',(7, 10, 5, 6, 5, 9, 10, 0, 0, 0)),
 'p_processes.book':('⠏',(4, 2, 4, 3, 5, 1, 2, 2, 3, 5)),
 'w_workflow.book':('⠺',(4, 3, 2, 4, 2, 1, 0, 0, 1, 2)),
 'x_execution.book':('⠭',(4, 2, 3, 2, 2, 0, 2, 4, 9, 1)),
 'd46_recovery.book':('⠨',(3, 10, 8, 6, 9, 3, 2, 7, 6, 4)),
 'v_verification.book':('⠧',(3, 4, 0, 5, 4, 5, 5, 0, 2, 3)),
 'codex_sol_primary.book':('⡞',(2, 10, 4, 10, 3, 4, 10, 10, 9, 8)),
 'codex_sol_shelf_ii.book':('⢞',(10, 10, 5, 9, 1, 10, 3, 9, 10, 3)),
 'codex_sol_shelf_iii.book':('⣞',(10, 2, 5, 5, 6, 1, 6, 0, 0, 0)),
 'codex_sol_context.book':('⡯',(8, 10, 4, 7, 0, 0, 0, 0, 0, 0)),
 'competition_primary.book':('⠅',(10, 4, 3, 0, 0, 0, 5, 0, 0, 0)),
 'competition_shelf.book':('⡅',(4, 3, 4, 0, 0, 0, 0, 0, 0, 0)),
 'connected_primary.book':('⠌',(2, 1, 3, 3, 8, 5, 6, 3, 8, 6)),
 'connected_shelf.book':('⡌',(4, 1, 1, 1, 3, 1, 5, 1, 0, 0)),
 'labyrinth_primary.book':('⡬',(10, 10, 8, 9, 9, 10, 8, 9, 9, 10)),
 'labyrinth_shelf.book':('⣬',(4, 6, 7, 0, 0, 0, 0, 0, 0, 0)),
}
DIGITS='1234567890'
MAX_CHAPTER=64*1024
MAX_PAGE=64*1024
MAX_TOTAL=2*1024*1024
SCHEMA='angel.book-source-manifest/v6'
MARKER=b'@@book-page '

def read_bounded(path):
    fd=os.open(path,os.O_RDONLY|os.O_CLOEXEC|os.O_NOFOLLOW|os.O_NONBLOCK)
    with os.fdopen(fd,'rb') as f:
        if not stat.S_ISREG(os.fstat(f.fileno()).st_mode): raise ValueError(f'{path.name}: not a regular file')
        data=f.read(MAX_CHAPTER+1)
    if len(data)>MAX_CHAPTER: raise ValueError(f'{path.name}: exceeds limit')
    return data

def parse_marked(data):
    data.decode('utf-8')
    rows=data.split(b'\n')
    if rows[-1]!=b'': raise ValueError('missing final separator')
    entries=[]; digit=None; content=[]
    for line in rows[:-1]:
        if line.startswith(MARKER):
            if len(line)!=len(MARKER)+3 or line[-2:]!=b'@@' or line[len(MARKER)] not in b'0123456789':
                raise ValueError('invalid marker')
            if digit is not None: entries.append((digit,b'\n'.join(content)))
            digit=chr(line[len(MARKER)]); content=[]
        else:
            if digit is None: raise ValueError('page before marker')
            content.append(line)
    if digit is not None: entries.append((digit,b'\n'.join(content)))
    return entries

def parse_navigation(data):
    text=data.decode('utf-8')
    entries=[]
    lines=text.split('\n')
    trailing_newline = lines[-1] == ''
    if trailing_newline: lines.pop()
    for index,line in enumerate(lines):
        if index < len(lines)-1 or trailing_newline: line=line.removesuffix('\r')
        digit,sep,page=line.partition('\t')
        if not sep or len(digit)!=1 or digit not in DIGITS or not page or '\t' in page or '\r' in page:
            raise ValueError('invalid navigation row')
        entries.append((digit,page.encode()))
    return entries

def validate(name,data):
    _,expected=EXPECTED[name]
    entries=parse_navigation(data) if name.endswith('.tsv') else parse_marked(data)
    limit=4096 if name.endswith('.tsv') else MAX_PAGE
    if any(not page or len(page)>limit for _,page in entries): raise ValueError(f'{name}: invalid page size')
    counts=tuple(sum(digit==d for digit,_ in entries) for d in DIGITS)
    if counts!=expected: raise ValueError(f'{name}: changed route/page addresses')
    # Grouping order is part of the immutable section layout.
    if [d for d,_ in entries] != [d for d,n in zip(DIGITS,expected) for _ in range(n)]:
        raise ValueError(f'{name}: routes out of order')

def publish(root):
    directory=Path(root)/'cockpit/book'
    first={name:read_bounded(directory/name) for name in EXPECTED}
    if sum(map(len,first.values()))>MAX_TOTAL: raise ValueError('catalog sources exceed total limit')
    for name,data in first.items(): validate(name,data)
    for name,data in first.items():
        if read_bounded(directory/name)!=data: raise ValueError(f'{name}: changed during publication')
    manifest={'schema':SCHEMA,'chapters':{name:{'cell':EXPECTED[name][0],'sha256':hashlib.sha256(data).hexdigest()} for name,data in sorted(first.items())}}
    body=(json.dumps(manifest,ensure_ascii=False,sort_keys=True,separators=(',',':'))+'\n').encode()
    fd,tmp=tempfile.mkstemp(prefix='.catalog-manifest-',suffix='.tmp',dir=directory)
    try:
        with os.fdopen(fd,'wb') as f:
            f.write(body); f.flush(); os.fsync(f.fileno())
        os.replace(tmp,directory/'catalog-manifest.json')
        dfd=os.open(directory,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC)
        try: os.fsync(dfd)
        finally: os.close(dfd)
    except BaseException:
        try: os.unlink(tmp)
        except FileNotFoundError: pass
        raise
    return manifest

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[2])
    args=parser.parse_args()
    try: print(json.dumps(publish(args.root),ensure_ascii=False,sort_keys=True))
    except (OSError,ValueError) as error: parser.exit(1,f'book catalog publication failed: {error}\n')
