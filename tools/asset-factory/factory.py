"""Small, inspectable Delve asset pipeline. Quality decisions precede publication."""
import argparse
from contextlib import contextmanager
import hashlib
import html
import json
import math
import os
from pathlib import Path
import re
import shutil
import tempfile
import sys
import time
import fcntl

import numpy as np
from PIL import Image, ImageDraw, ImageChops

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/sprite-rig'))
from spriterig import quick
from spriterig.quick_motion import CLIPS
from backend import generate

KINDS = ('body', 'weapon', 'worn', 'vfx', 'prop', 'tile')
INKS = quick._palette(str(ROOT))
STYLE = {k: INKS[k] for k in 'KXgjGJho'}
FITS = json.loads((ROOT / 'crates/delve/assets/knight-fit.json').read_text())['profiles']['realm_right']
CATALOG = ROOT / 'crates/delve/assets/factory/catalog.json'
DEFAULT_OUT = Path(tempfile.gettempdir()) / 'asset-factory'


def dump(path, data):
    path = Path(path); path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + '.tmp')
    tmp.write_text(json.dumps(data, indent=2) + '\n'); tmp.replace(path)


def slug(text):
    value = re.sub('[^a-z0-9]+', '-', text.lower()).strip('-')[:24].rstrip('-')
    if not value:
        raise ValueError('asset id needs letters or digits')
    return value


def rgba(rows):
    a = np.zeros((len(rows), len(rows[0]), 4), dtype=np.uint8)
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c != '.':
                a[y, x] = (*INKS[c], 255)
    return Image.fromarray(a)


def rows(image):
    a = np.array(image.convert('RGBA'))
    return quick._rows(quick._inks(a, STYLE))


def components(mask):
    unseen = set(zip(*np.nonzero(mask))); found = []
    while unseen:
        start = unseen.pop(); group = {start}; todo = [start]
        while todo:
            y, x = todo.pop()
            for p in ((y-1,x), (y+1,x), (y,x-1), (y,x+1)):
                if p in unseen:
                    unseen.remove(p); group.add(p); todo.append(p)
        found.append(group)
    return sorted(found, key=len, reverse=True)


def target(kind, part):
    if kind == 'worn':
        return tuple(FITS[part]['size'])
    return {'body': (16,16), 'weapon': tuple(FITS['right']['size']),
            'vfx': (24,16), 'prop': (16,16), 'tile': (16,16)}[kind]


def clean(source, kind, part, key=None):
    with Image.open(source) as opened:
        if opened.width > 4096 or opened.height > 4096:
            raise ValueError('source exceeds 4096 pixels per side')
        image = opened.convert('RGBA')
    a = np.array(image)
    if key:
        # Explicit background key, not corner guessing. Preserve dark opaque ink.
        color = np.array(tuple(bytes.fromhex(key.lstrip('#'))), dtype=np.int16)
        if color.shape != (3,):
            raise ValueError('key must be six hex digits')
        delta = np.linalg.norm(a[:,:,:3].astype(np.int16)-color, axis=2)
        a[delta < 60, 3] = 0
    a[:,:,3] = np.where(a[:,:,3] >= 128, 255, 0)
    if not a[:,:,3].any():
        raise ValueError('empty source after explicit alpha/key cleanup')
    source_image = Image.fromarray(a)
    box = source_image.getbbox(); cropped = source_image.crop(box)
    tw, th = target(kind, part)
    cw, ch = cropped.size
    ratio = min(tw/cw, th/ch)
    size = (max(1, round(cw*ratio)), max(1, round(ch*ratio)))
    # Area sampling of a master is intentional; only native ink pixels reach game.
    small = cropped.resize(size, Image.Resampling.BOX if ratio < 1 else Image.Resampling.NEAREST)
    b = np.array(small); b[:,:,3] = np.where(b[:,:,3] >= 128,255,0)
    native = Image.new('RGBA',(tw,th))
    native.alpha_composite(rgba(quick._rows(quick._inks(b, STYLE))), ((tw-size[0])//2, th-size[1]))
    return native, {'source_crop': list(box), 'uniform_scale': ratio, 'native_size': [tw,th],
                    'key': key, 'note': 'No anisotropic squash; empty armour only for torso.'}


def animation(entry):
    base = entry['frames'][0]
    if entry['kind'] == 'vfx':
        # Hold the source centre, erase outer rays over time. Never translate frames.
        a = np.array([list(row) for row in base]); h,w = a.shape
        yy,xx = np.mgrid[:h,:w]; distance = np.sqrt((xx-w//2)**2+(yy-h//2)**2)
        maximum = max(1,float(distance[a!='.'].max()))
        frames=[]
        for factor in (1.0, .8, .55, .3):
            f = a.copy(); f[distance>maximum*factor] = '.'
            if not (f!='.').any():
                y,x = min(zip(*np.nonzero(a!='.')), key=lambda p: distance[p]); f[y,x] = a[y,x]
            frames.append(quick._rows(f))
        entry['frames'] = frames
    if entry['kind'] == 'weapon':
        clip = entry['attack']['clip']; frames=[]
        for phase in range(4):
            image = Image.new('RGBA',(24,16)); d = ImageDraw.Draw(image)
            if clip == 'thrust':
                d.line((3+phase*3,8,10+phase*3,8), fill=(*STYLE['h'],255), width=1)
                d.line((7+phase*3,9,11+phase*3,9), fill=(*STYLE['o'],255), width=1)
            else:
                d.arc((2,1,22,15), 195+phase*35, 240+phase*35, fill=(*STYLE['h'],255),width=1)
                d.arc((4,3,20,13), 200+phase*35, 230+phase*35, fill=(*STYLE['o'],255),width=1)
            frames.append(rows(image))
        entry['attack']['vfx'] = frames
    return entry


def checks(entry, image):
    a=np.array(image); mask=a[:,:,3]>0; h,w=mask.shape
    if not mask.any():
        return [dict(check='nonempty', passed=False, reason='No ink survives cleanup; fix key or source.', fix=None)]
    ink = [c for frame in entry['frames'] for row in frame for c in row if c!='.']
    groups=components(mask)
    result=[]
    def check(name, ok, reason, fix=None):
        result.append(dict(check=name, passed=bool(ok), reason=reason, fix=fix))
    check('palette', bool(ink) and set(ink)<=set(STYLE), 'Only muted Delve structure inks and one ochre accent.', 'quantize-style')
    check('alpha', bool(np.isin(a[:,:,3], [0,255]).all()), 'Binary alpha; no matte or semi-transparent edge pixels.', 'binary-alpha')
    check('native-size', (w,h)==target(entry['kind'],entry['part']), f'{w}×{h}; fit derived from knight-fit.json, uniform scale only.')
    px,py=entry['pivot']
    check('pivot', 0<=px<w and 0<=py<h, f'Fixed source pixel pivot {px},{py}.')
    check('frame-registration', all(len(f)==h and all(len(r)==w for r in f) for f in entry['frames']), 'Identical canvas, no per-frame trim or independently fitted pivot.')
    occupancy=float(mask.mean())
    check('silhouette-mass', .025<occupancy<.88, f'1x occupied area {occupancy:.1%}; reference contact sheet requires visual review.')
    warm=sum(c=='o' for c in ink)/max(1,len(ink))
    check('warm-accent', warm<=.28, f'Ochre is {warm:.1%} of ink; at most 28%, no signal-bank white/glow.', 'reduce-accent')
    if entry['kind'] not in ('vfx','weapon'):
        connected=len(groups[0])/max(1,int(mask.sum())) if groups else 0
        check('silhouette-connected', connected>=.80, f'Largest 4-connected cluster {connected:.1%}; no isolated matte flecks.', 'remove-flecks')
    if entry['kind']=='worn' and entry['part']=='body':
        yy,xx=np.nonzero(mask); aspect=(xx.max()-xx.min()+1)/(yy.max()-yy.min()+1) if len(xx) else 0
        check('armour-not-figure', aspect>=1.5, f'Torso silhouette aspect {aspect:.2f}; empty short/wide breastplate, not a compressed person.', 'needs-armour-source')
    if entry['kind']=='body':
        game=entry.get('rig',{}); clips=game.get('clips',{})
        check('full-clips', set(clips)==set(CLIPS), 'All 13 shared quick-rig clips, not repeated static labels.')
        try:
            quick.validate_game(game,root=str(ROOT)); valid=True
        except ValueError:
            valid=False
        check('rig-contract', valid, 'Quick-rig schema validates palette, integer sockets, frame canvas and pivot.')
        floors=[]
        for name in ('idle','walk','run'):
            for frame in clips.get(name,[]):
                ys=[y for y,row in enumerate(frame['rows']) if any(c!='.' for c in row)]
                if ys: floors.append(max(ys))
        check('ground-jitter', bool(floors) and max(floors)-min(floors)<=1, f'Idle/walk/run painted floor spread {max(floors)-min(floors) if floors else "missing"} px.')
    return result


def repair(image, failures):
    a=np.array(image); notes=[]
    for failure in failures:
        fix=failure['fix']
        if fix=='remove-flecks':
            groups=components(a[:,:,3]>0)
            # Only remove true single-pixel noise, never missing anatomy or a limb.
            for group in groups[1:]:
                if len(group)<=1:
                    for y,x in group: a[y,x]=0
            notes.append('Removed only isolated one-pixel alpha flecks; anatomy unchanged.')
        elif fix=='reduce-accent':
            color=np.array(STYLE['o']); warm=(a[:,:,:3]==color).all(axis=2)&(a[:,:,3]>0)
            yy,xx=np.nonzero(warm)
            for i,(y,x) in enumerate(zip(yy,xx)):
                if i%5: a[y,x,:3]=STYLE['G']
            notes.append('Reduced excessive ochre to small retained accents, mapped remainder to mid grey.')
    return Image.fromarray(a), notes


def fit_entry(spec, image):
    kind=spec['kind']; w,h=image.size; part=spec.get('part','body')
    entry={k:spec[k] for k in ('id','name','brief','kind')}
    entry.update(frames=[rows(image)], part=part, pivot=[w//2,h-1], anchors={})
    if kind=='body':
        entry['part']='whole'
        rig=quick.fit(image, root=str(ROOT))
        game=quick.build(image,rig,root=str(ROOT))
        # Fit the shared gait to this silhouette's actual raster floor. Save
        # correction in authoring poses, then rebuild; never shift exported rows.
        for _ in range(3):
            changed=False
            for name in ('idle','walk','run'):
                for pose,frame in zip(rig['clips'][name],game['clips'][name]):
                    floor=max(y for y,row in enumerate(frame['rows']) if any(c!='.' for c in row))
                    dy=rig['pivot'][1]-floor
                    if dy:
                        pose.setdefault('offset',[0,0])[1]+=dy; changed=True
            if not changed: break
            game=quick.build(image,rig,root=str(ROOT))
        entry['rig']=game
        entry['anchors']={k:[int(round(v)) for v in p['point']] for k,p in rig['anchors'].items()}
        # Source-space sockets must remain inside source; game canvas sockets are in rig.
        entry['anchors']={k:[min(w-1,max(0,p[0])),min(h-1,max(0,p[1]))] for k,p in entry['anchors'].items()}
        return entry,rig
    if kind=='weapon':
        mask=np.array(image)[:,:,3]>0; yy,xx=np.nonzero(mask)
        entry['part']='right'; entry['anchors']={'grip':[int(np.median(xx[yy==yy.max()])),int(yy.max())],'tip':[int(xx[yy.argmin()]),int(yy.min())]}
        entry['pivot']=entry['anchors']['grip'][:]
        entry['attack']={'clip':spec.get('clip','slash'),'sound':spec.get('sound',''),'vfx':[]}
    elif kind=='worn':
        entry['anchors']={part:[w//2,h-1 if FITS[part]['bottom'] else h//2]}
    else:
        entry['anchors']={'origin':entry['pivot'][:]}
    return entry,None


def contact(entry, image, path):
    reference=Image.open(ROOT/'tools/sprite-rig/samples/quick/knight/source.png').convert('RGBA')
    samples=[reference,image]
    if entry['kind']=='body':
        for clip in ('idle','walk','run','slash','thrust','flail','hurt','roll'):
            samples.append(quick.frame_image(entry['rig']['clips'][clip][0],root=str(ROOT)))
    else:
        samples += [rgba(f) for f in entry['frames']]
    canvas=Image.new('RGBA',(max(160,len(samples)*48),64),(29,29,27,255))
    d=ImageDraw.Draw(canvas); d.line((0,49,canvas.width,49),fill=(70,68,61))
    for i,sample in enumerate(samples):
        canvas.alpha_composite(sample, (i*48+8,49-sample.height))
    canvas.save(path)
    canvas.resize((canvas.width*4,canvas.height*4),Image.Resampling.NEAREST).save(path.with_name('contact-4x.png'))


def publish(entry, path=CATALOG):
    path=Path(path); path.parent.mkdir(parents=True,exist_ok=True)
    with path.with_suffix('.lock').open('w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        catalog=json.loads(path.read_text()) if path.exists() else {'version':1,'assets':[]}
        catalog['assets']=[a for a in catalog['assets'] if a['id']!=entry['id']]+[entry]
        catalog['assets'].sort(key=lambda a:a['id'])
        dump(path,catalog)


def review(out):
    out=Path(out); destination=out/'review'; destination.mkdir(parents=True,exist_ok=True)
    cards=[]
    for record in sorted((out/'jobs').glob('*/record.json')):
        r=json.loads(record.read_text()); ident=r['id']; here=destination/ident
        shutil.copytree(record.parent,here,dirs_exist_ok=True)
        pictures=[]
        for file,label in [('source.png','Source'),('before-fix.png','Before targeted fix'),('clean.png','Clean / corrected'),('fit.png','Fit / anchors'),('animate.png','Animate'),('contact-1x.png','Game scale / knight'),('contact-4x.png','Pixel review')]:
            if (here/file).exists(): pictures.append(f'<figure><figcaption>{label}</figcaption><img src="{ident}/{file}" alt="{label}"></figure>')
        for scale in (1,4):
            shot=out/'shots'/f'{ident}-{scale}x.png'
            if shot.exists():
                shutil.copy2(shot,here/shot.name)
                pictures.append(f'<figure><figcaption>Arena {scale}x</figcaption><img src="{ident}/{shot.name}" alt="Arena {scale}x"></figure>')
        poses=[out/'shots'/f'{ident}-{pose}-1x.png' for pose in ('rest','attack')]
        if all(path.exists() for path in poses):
            pair=Image.new('RGB',(400,240),(38,37,33)); draw=ImageDraw.Draw(pair)
            for n,path in enumerate(poses):
                with Image.open(path) as opened:
                    im=opened.convert('RGB')
                # Native Img PNGs intentionally export an opaque background.
                bounds=ImageChops.difference(im,Image.new('RGB',im.size,im.getpixel((0,0)))).getbbox()
                if bounds:
                    im=im.crop(bounds); x=n*200+5
                    draw.text((x,3),('Rest','Contact')[n],fill=(173,146,101))
                    pair.paste(im,(x,20))
                    pair.paste(im.resize((im.width*6,im.height*6),Image.Resampling.NEAREST),(x,65))
            pair.save(here/'renderer-pair.png')
            pictures.append(f'<figure><figcaption>Real renderer · 1x / 6x</figcaption><img src="{ident}/renderer-pair.png" alt="Native and enlarged rest and contact figures"></figure>')
        if r['kind']=='body':
            clips=' '.join(f'<a href="{ident}/clips/{name}.gif">{name}</a>' for name in CLIPS)
            pictures.append(f'<figure><figcaption>Shared clips · 30Hz</figcaption><p>{clips}</p></figure>')
        reasons=''.join(f'<li>{"PASS" if c["passed"] else "FAIL"} · {html.escape(c["check"])} — {html.escape(c["reason"])}</li>' for c in r.get('checks',[]))
        times=' · '.join(f'{k}: {v:.3f}s' for k,v in r.get('seconds',{}).items())
        attempts=''.join(f'<li>{html.escape(json.dumps(a))}</li>' for a in r.get('attempts',[]))
        if r.get('error'): reasons+=f'<li>FAIL — {html.escape(r["error"])}</li>'
        cards.append(f'<article id="{ident}"><h2>{html.escape(r["name"])} — {r["status"]}</h2><p>{html.escape(r["brief"])}</p><p>{html.escape(times)}</p><div class="stages">{"".join(pictures)}</div><ul>{reasons}</ul><details><summary>Targeted fixes</summary><ul>{attempts}</ul></details><p>{html.escape(r.get("visual_review","Visual review pending; metrics do not certify style."))}</p></article>')
    page='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Delve · asset bench</title><style>
body{background:#242421;color:#b0b2a8;font:15px Georgia,serif;margin:24px}h1,h2{color:#a97639;font-weight:normal}article{border:4px double #63655c;padding:16px;margin:24px 0;background:#2d2c28}figure{margin:8px;max-width:100%;overflow:auto}figcaption{padding:5px}img{image-rendering:pixelated;max-width:none}.stages{display:flex;flex-wrap:wrap;align-items:flex-start}li{margin:5px}summary{color:#a97639}header{border-bottom:3px double #7a7c72}p{line-height:1.5}</style><header><h1>Delve · asset bench</h1><p>Inspect pixels · Read gate · Enter arena</p><p>Quality before speed. Timings are recorded, not optimised. Native contact sheets include the actual quick-rig knight source; arena shots use the production renderer. Local Qwen-Image only; no Grok images.</p></header>'''+''.join(cards)+'</html>'
    (destination/'index.html').write_text(page)
    return destination/'index.html'


def make(spec, out=DEFAULT_OUT, catalog=CATALOG):
    if spec['kind'] not in KINDS: raise ValueError('unknown kind')
    spec=dict(spec); spec.setdefault('name',spec['brief'][:60]); spec.setdefault('id',slug(spec['name']))
    if spec['id']!=slug(spec['id']): raise ValueError('id must be a lowercase slug, at most 24 characters')
    spec.setdefault('part','body')
    if spec['part'] not in FITS: raise ValueError('part must name an existing knight-fit part')
    out=Path(out); job=out/'jobs'/spec['id']; job.mkdir(parents=True,exist_ok=True)
    if (job/'record.json').exists():
        history=job/'history'; history.mkdir(exist_ok=True)
        shutil.copy2(job/'record.json',history/f'{time.time_ns()}.json')
    record={**spec,'seconds':{},'attempts':[],'status':'failed'}
    @contextmanager
    def stage(name):
        start=time.perf_counter()
        try: yield
        finally: record['seconds'][name]=record['seconds'].get(name,0)+time.perf_counter()-start
    try:
        if not spec['name'].strip() or len(spec['name'].encode())>120 or len(spec['brief'].encode())>2048 or any(c in spec['name']+spec['brief'] for c in '\r\n'):
            raise ValueError('name/brief must be bounded single lines')
        if spec['kind']=='weapon':
            spec.setdefault('clip','slash'); spec.setdefault('sound','swing')
            if spec['clip'] not in ('slash','overhead','thrust','flail'):
                raise ValueError('weapon requires a shared melee clip')
            sound_root=ROOT/'cockpit/assets/dungeon/sfx'
            sounds={pick for path in sound_root.rglob('*.mp3')
                    for pick in (path.relative_to(sound_root).with_suffix('').as_posix(),
                                 path.relative_to(sound_root).with_suffix('').as_posix().replace('/','-'))}
            if spec['sound'] not in sounds:
                raise ValueError('sound must be an existing game sound pick')
        with stage('generate'):
            source=spec.get('source')
            if source:
                source=Path(source).resolve()
                if source != (job/'source.png').resolve(): shutil.copy2(source,job/'source.png')
                record['backend']='source image'; record['source_sha256']=hashlib.sha256((job/'source.png').read_bytes()).hexdigest()
            else:
                record['generation']=generate(spec['brief'],spec['kind'],job/'source.png',out/'generation-calls.jsonl',spec.get('seed',39))
                spec.setdefault('key','ff00ff')
        with stage('clean'):
            image,registration=clean(job/'source.png',spec['kind'],spec['part'],spec.get('key'))
            image.save(job/'clean.png'); record['registration']=registration
            if spec.get('fixup'):
                fixup=json.loads(Path(spec['fixup']).read_text())
                if not isinstance(fixup.get('reason'),str) or not fixup['reason'].strip():
                    raise ValueError('fixup needs a specific review reason')
                fixed=rgba(fixup['rows'])
                if fixed.size != image.size: raise ValueError('fixup must keep the canonical native canvas')
                image.save(job/'before-fix.png'); image=fixed
                image.save(job/'clean.png')
                record['attempts'].append({'fix':'native-pixel-edit','reason':fixup['reason'], 'source_sha256':hashlib.sha256((job/'source.png').read_bytes()).hexdigest()})
        for attempt in range(2):
            with stage('fit'):
                entry,rig=fit_entry(spec,image)
                overlay=image.copy(); d=ImageDraw.Draw(overlay)
                for p in entry['anchors'].values(): d.point(tuple(p),fill=(*STYLE['o'],255))
                overlay.save(job/'fit.png')
                if rig: dump(job/'rig.json',rig)
            with stage('animate'):
                entry=animation(entry)
                if rig:
                    quick.export(entry['rig'],job/'clips',root=str(ROOT))
                    shutil.copy2(job/'clips/sheet.png',job/'animate.png')
                else:
                    frames=entry.get('attack',{}).get('vfx',entry['frames'])
                    strip=Image.new('RGBA',(len(frames)*len(frames[0][0]),len(frames[0])))
                    for i,frame in enumerate(frames): strip.alpha_composite(rgba(frame),(i*len(frame[0]),0))
                    strip.save(job/'animate.png')
            with stage('gate'):
                result=checks(entry,image); failures=[c for c in result if not c['passed']]
                record['checks']=result
                if not failures: break
                record['attempts'].append({'attempt':attempt+1,'failed':failures})
                fixed,notes=repair(image,failures)
                if not notes: break
                record['attempts'][-1]['fixes']=notes
                image=fixed; image.save(job/'clean.png')
        dump(job/'asset.json',entry); contact(entry,image,job/'contact-1x.png')
        if not failures:
            with stage('publish'): publish(entry,catalog)
            record['status']='passed'
        else:
            record['error']='Gate rejected; fix source/rig using the listed reasons, then rerun this id. No blind generation retry.'
    except Exception as error:
        record['error']=str(error)
    log=out/'generation-calls.jsonl'
    if log.exists() and record.get('source_sha256'):
        calls=[json.loads(line) for line in log.read_text().splitlines() if line.strip()]
        original=next((call for call in calls if call.get('sha256')==record['source_sha256']),None)
        if original:
            record['generation']=original
            record['seconds']['source_import']=record['seconds'].get('generate',0)
            record['seconds']['generate']=original['seconds']
            record['backend']='local Qwen-Image 2.1, imported logged master'
    dump(job/'record.json',record); review(out)
    return record


def main(argv=None):
    p=argparse.ArgumentParser(description=__doc__); p.add_argument('--out',type=Path,default=DEFAULT_OUT)
    sub=p.add_subparsers(dest='command',required=True)
    m=sub.add_parser('make'); m.add_argument('kind',choices=KINDS); m.add_argument('brief'); m.add_argument('--name'); m.add_argument('--id'); m.add_argument('--source'); m.add_argument('--key'); m.add_argument('--fixup'); m.add_argument('--part',default='body',choices=list(FITS)); m.add_argument('--clip',default='slash',choices=['slash','overhead','thrust','flail']); m.add_argument('--sound',default='swing'); m.add_argument('--seed',type=int,default=39)
    b=sub.add_parser('batch'); b.add_argument('file',type=Path)
    sub.add_parser('review')
    a=sub.add_parser('approve'); a.add_argument('id'); a.add_argument('reason')
    args=p.parse_args(argv)
    if args.command=='review': print(review(args.out)); return 0
    if args.command=='approve':
        path=args.out/'jobs'/slug(args.id)/'record.json'; r=json.loads(path.read_text())
        if r['status']!='passed': raise ValueError('cannot approve a failed gate')
        r['visual_review']=args.reason; dump(path,r); print(review(args.out)); return 0
    specs=json.loads(args.file.read_text()) if args.command=='batch' else [{k:v for k,v in vars(args).items() if k not in ('command','out') and v is not None}]
    if not isinstance(specs,list): p.error('batch must be a JSON array')
    failed=False
    for spec in specs:
        if args.command=='batch':
            for field in ('source','fixup'):
                if field in spec: spec[field]=str((args.file.parent/spec[field]).resolve())
        r=make(spec,args.out); print(f'{r["id"]}: {r["status"]} {r.get("error","")}'); failed|=r['status']!='passed'
    return int(failed)

if __name__=='__main__':
    raise SystemExit(main())
