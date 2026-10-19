#!/usr/bin/env -S uv run --no-project --with Pillow python
"""Stage read-only slot deliveries. See docs/SLOT-ART-IMPORT.md."""
import argparse
from collections import Counter, deque
from functools import lru_cache
import hashlib
import io
import json
from pathlib import Path
import re
import statistics
from PIL import Image, ImageDraw, ImageFilter, ImageChops, ImageEnhance

ROOT = Path(__file__).resolve().parents[1]
SOURCES = Path('/work/artifacts/art-prod/slots')
OUTPUT = ROOT / 'crates/delve/assets/art-slots'
VERSION = 2


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, data, check=False):
    if check:
        assert path.read_bytes() == data, f'stale: {path}'
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists() or path.read_bytes() != data:
            path.write_bytes(data)


def js(data):
    return (json.dumps(data, indent=2, sort_keys=True) + '\n').encode()


def png(im):
    b = io.BytesIO(); im.save(b, format='PNG'); return b.getvalue()


def requests(extra=None):
    result = {a['id']: a for g in json.loads((ROOT/'crates/delve/assets/art-requests.json').read_text())['games'] for a in g['assets']}
    # Supplemental deliveries carry a complete local request, rather than
    # borrowing a different actor's geometry or editing the shared inventory.
    for path in sorted(OUTPUT.glob('*/request.json')):
        request = json.loads(path.read_text())
        slot = request['id']
        assert slot == path.parent.name and re.fullmatch(r'[a-z]+\.[a-z0-9-]+', slot)
        assert slot not in result, f'supplemental request shadows inventory: {slot}'
        for field in ('role', 'fit', 'timing', 'geometry', 'frame_groups'):
            assert field in request, f'{slot}: missing {field}'
        result[slot] = request
    if extra:
        result.update({a['id']: a for g in json.loads(extra.read_text())['games'] for a in g['assets']})
    return result


def palette():
    text = (ROOT/'crates/delve/src/render/overworld/ink.rs').read_text()
    bank = text.split('pub const PALETTE: [u32; 75] = [')[1].split('];')[0]
    return [tuple(bytes.fromhex(h)) for h in re.findall(r'0x([0-9a-f]{6})', bank) if h not in ('000000', 'e4fff8') and min(bytes.fromhex(h)) < 240]


@lru_cache(maxsize=None)
def ink(rgb):
    return min(palette_cache, key=lambda c: sum((a-b)**2 for a,b in zip(rgb,c)))


palette_cache = palette()


def reading_order(pieces):
    """Rows from centres, not top edges (raised weapons otherwise reorder poses)."""
    if not pieces: return []
    gap = statistics.median(p['source_rect_xywh'][3] for p in pieces) * .6
    rows = []
    for p in sorted(pieces, key=lambda p: p['source_rect_xywh'][1]+p['source_rect_xywh'][3]/2):
        x,y,w,h = p['source_rect_xywh']; cy=y+h/2
        if not rows or cy-statistics.mean(q['source_rect_xywh'][1]+q['source_rect_xywh'][3]/2 for q in rows[-1]) > gap:
            rows.append([])
        rows[-1].append(p)
    return [p for row in rows for p in sorted(row,key=lambda p:p['source_rect_xywh'][0])]


def fringe(c):
    r,g,b,a = c
    return a >= 32 and r > g*1.35+12 and b > g*1.3+8 and min(r,b)>45


def despill(im):
    """Propagate trusted interior RGB into the 3px alpha boundary; preserve alpha."""
    px=im.load(); w,h=im.size
    core=im.getchannel('A').point(lambda a:255 if a>=128 else 0)
    if core.getextrema()==(255,255):
        return im.copy(), {'recoloured':0,'fringe_before':0,'fringe_after_boundary':0}
    padded=Image.new('L',(w+6,h+6));padded.paste(core,(3,3))
    interior=padded.filter(ImageFilter.MinFilter(7)).crop((3,3,w+3,h+3))
    good=interior.load(); owners={}; todo=deque()
    for y in range(h):
        for x in range(w):
            if good[x,y] and not fringe(px[x,y]):
                owners[x,y]=px[x,y][:3]; todo.append((x,y,0))
    while todo:
        x,y,d=todo.popleft()
        for q in ((x-1,y),(x+1,y),(x,y-1),(x,y+1)):
            xx,yy=q
            if 0<=xx<w and 0<=yy<h and q not in owners and px[q][3]>0:
                owners[q]=owners[x,y];todo.append((xx,yy,d+1))
    out=im.copy(); op=out.load(); changed=0
    for y in range(h):
        for x in range(w):
            c=px[x,y]
            if c[3] and not good[x,y] and (c[3]<224 or fringe(c)) and (x,y) in owners:
                # Only the boundary is recoloured; warm interior materials survive.
                op[x,y]=owners[x,y]+(c[3],);changed+=int(op[x,y]!=c)
            if c[3]==0: op[x,y]=(0,0,0,0)
    residue=sum(fringe(op[x,y]) for y in range(h) for x in range(w) if op[x,y][3]>=32 and not good[x,y])
    return out, {'recoloured':changed,'fringe_before':sum(fringe(c) for c in im.get_flattened_data()),'fringe_after_boundary':residue}


def initial_map(slot, req, meta, notes, sheet, source=None):
    source = source or SOURCES/slot/'v01'
    ordered=reading_order(meta['pieces'])
    labels=re.findall(r'^\s*\d+\.\s*(.+)$',notes,re.M)[:len(ordered)]
    frames=[(g['state'],i,n) for g in req['frame_groups'] for i,n in enumerate(g.get('names',[])) if not g['state'].startswith('retained')]
    size=re.search(r'(\d+)\s*[×x]\s*(\d+)',req['fit'])
    dims=list(map(int,size.groups())) if size else [48,48]
    actor=req['role'] in ('boss','rival','enemy','player knight')
    pivot=[dims[0]//2,dims[1]] if actor else [dims[0]//2,dims[1]//2]
    items=[]
    for i,p in enumerate(ordered):
        x,y,w,h=p['source_rect_xywh']
        crop=sheet.crop((x,y,x+w,y+h)); a=crop.getchannel('A')
        feet=[xx for yy in range(max(0,h-max(2,h//12)),h) for xx in range(w) if a.getpixel((xx,yy))>=128]
        sp=[round(statistics.mean(feet)) if feet else w//2,h] if actor else [w/2,h/2]
        exact=len(frames)==len(ordered)
        state,fi,name=frames[i] if exact else (re.sub(r'[^a-z0-9]+','-',(labels[i] if i<len(labels) else 'unmapped').split(':')[0].lower()).strip('-')[:64],i,f'piece-{p["index"]:02}')
        items.append(dict(piece=p['index'],source_rect_xywh=p['source_rect_xywh'],label=labels[i] if i<len(labels) else 'No delivered reading-order label',state=state,frame=fi,name=name,source_pivot_xy=sp,approved=False))
    expected=sum(g.get('source_pose_count') or 0 for g in req['frame_groups'] if not g['state'].startswith('retained'))
    return dict(schema_version=1,slot=slot,review='needs-manual-map',mapping_basis='unreviewed request order when counts match; otherwise delivered labels',source_sha256=digest(source/'source-sheet.png'),notes_sha256=digest(source/'notes.md'),registration='shared-feet' if actor else 'own-bounds',size_basis=req['fit'],sizes={'v1':dims,'v2':dims},anchor_xy=pivot,anchor_canvas_wh=dims.copy(),expected_existing_pieces=expected,delivered_pieces=len(items),timing=req['timing'],geometry=req['geometry'],pieces=items,runtime=False,comments=['Proposed reading order; review labels, pivots and mapping before activation.','Both kits retain the request canvas until a separate size decision.'])


def validate(mapping, req, meta, sheet_path):
    assert mapping['source_sha256']==digest(sheet_path), 'source changed; review or regenerate map'
    actual={p['index']:p['source_rect_xywh'] for p in meta['pieces']}
    assert len(mapping['pieces'])==len(actual)
    assert {p['piece'] for p in mapping['pieces']}==set(actual)
    assert len({p['name'] for p in mapping['pieces']})==len(actual), 'duplicate output name'
    for p in mapping['pieces']:
        assert p['source_rect_xywh']==actual[p['piece']], 'map crop differs from measured delivery'
        assert re.fullmatch(r'[a-zA-Z0-9_-]+',p['name'])
        assert len(p['source_pivot_xy'])==2
    assert mapping['registration'] in ('shared-feet','shared-pivot','own-bounds')
    assert all(0<v for dims in mapping['sizes'].values() for v in dims)
    if 'scale_reference_pieces' in mapping:
        refs = mapping['scale_reference_pieces']
        assert refs and len(set(refs)) == len(refs)
        assert set(refs) <= {p['piece'] for p in mapping['pieces'] if p['approved'] and p.get('runtime')}, 'scale reference must be an approved active subset'


def kit_value(control, field, kit):
    value = control.get(field, [])
    return value.get(kit, []) if isinstance(value, dict) else value


def checked_rect(rect, size):
    assert len(rect) == 4 and all(type(v) is int for v in rect)
    x, y, w, h = rect
    assert x >= 0 and y >= 0 and w > 0 and h > 0 and x+w <= size[0] and y+h <= size[1], 'review rectangle outside canvas'
    return x, y, x+w, y+h


def registered_crop(piece, crop):
    control = piece.get('reviewed_processing', {})
    pivot = piece['source_pivot_xy']
    if 'source_crop_xywh' not in control:
        return crop, pivot
    assert control.get('reviewed') is True, 'source subcrop needs explicit review'
    x, y, r, b = checked_rect(control['source_crop_xywh'], crop.size)
    return crop.crop((x,y,r,b)), [pivot[0]-x, pivot[1]-y]


def reviewed_native(ref, native, piece, kit, dest, scale, anchor):
    """Explicit material/erase data; no hue inference when a mask is supplied."""
    control = piece.get('reviewed_processing')
    if control is None:
        return native, None, None
    assert control.get('reviewed') is True, 'processing controls need explicit review'
    assert set(control) <= {'reviewed','source_crop_xywh','cleanup_rects','erase_runs','grade','material_mask','material_runs','replace_inferred_material'}, 'unknown reviewed processing control'
    native = native.copy()
    erase = Image.new('L', native.size)
    draw = ImageDraw.Draw(erase)
    for rect in kit_value(control, 'cleanup_rects', kit):
        x, y, r, b = checked_rect(rect, native.size)
        draw.rectangle((x, y, r-1, b-1), fill=255)
    for y, x, w in kit_value(control, 'erase_runs', kit):
        x, y, r, b = checked_rect([x, y, w, 1], native.size)
        draw.rectangle((x, y, r-1, b-1), fill=255)
    grade = {tuple(bytes.fromhex(a)): tuple(bytes.fromhex(b)) for a, b in control.get('grade', {}).items()}
    assert all(len(a) == 3 and b in palette_cache for a, b in grade.items()), 'grade must use admitted inks'
    native.putdata([(0,0,0,0) if e else grade.get(c[:3], c[:3])+(c[3],) for c,e in zip(native.get_flattened_data(), erase.get_flattened_data())])
    mask = None
    descriptor = control.get('material_mask')
    if descriptor:
        path = dest/descriptor['path']
        assert path.resolve().is_relative_to(dest.resolve()), 'mask must be kept with its map'
        assert digest(path) == descriptor['sha256'], 'reviewed material mask changed'
        mask = Image.open(path).convert('RGBA')
        if descriptor['space'] == 'source':
            assert mask.size == tuple(piece['source_rect_xywh'][2:]), 'source mask canvas differs from measured crop'
            mask, pivot = registered_crop(piece, mask)
            # Register exactly like the art, without palette-quantizing shades.
            mask, _ = sample(mask, native.size, pivot, anchor, scale)
        else:
            assert descriptor['space'] == 'native' and mask.size == native.size
    runs = kit_value(control, 'material_runs', kit)
    if runs or control.get('replace_inferred_material'):
        mask = mask or Image.new('RGBA', native.size)
        draw = ImageDraw.Draw(mask)
        for y, x, w, shade in runs:
            assert type(shade) is int and 0 <= shade <= 3
            x, y, r, b = checked_rect([x, y, w, 1], native.size)
            draw.rectangle((x, y, r-1, b-1), fill=(shade*70,)*3+(255,))
    if mask is not None:
        mask.putdata([(min(3,c[0]//70)*70,)*3+(255,) if c[3]>=128 and p[3] else (0,0,0,0) for c,p in zip(mask.get_flattened_data(), native.get_flattened_data())])
    return native, mask, hashlib.sha256(js(control)).hexdigest()


def sample(crop, size, pivot, anchor, scale, sampling='contain', grade=None, inset=0, trim_alpha=False, median=0):
    if trim_alpha:
        bounds=crop.getchannel('A').point(lambda a:255 if a>=128 else 0).getbbox()
        assert bounds, 'empty alpha bounds'
        crop=crop.crop(bounds)
    if inset:
        dx,dy=round(crop.width*inset),round(crop.height*inset)
        crop=crop.crop((dx,dy,crop.width-dx,crop.height-dy))
    if sampling == 'cover':
        # Crop to the canvas aspect before BOX reduction; never stretch a tile.
        ratio=min(crop.width/size[0],crop.height/size[1])
        assert ratio>=1, 'upscaling is not allowed'
        w,h=round(size[0]*ratio),round(size[1]*ratio)
        x,y=(crop.width-w)//2,(crop.height-h)//2
        crop=crop.crop((x,y,x+w,y+h))
        pivot=[crop.width/2,crop.height/2];anchor=[size[0]/2,size[1]/2]
        scale=1/ratio
    dims=(max(1,round(crop.width*scale)),max(1,round(crop.height*scale)))
    assert dims[0]<=crop.width and dims[1]<=crop.height, 'upscaling is not allowed'
    small=crop.resize(dims,Image.Resampling.BOX)
    out=Image.new('RGBA',size)
    at=(round(anchor[0]-pivot[0]*scale),round(anchor[1]-pivot[1]*scale))
    out.paste(small,at)
    if grade:
        alpha=out.getchannel('A')
        colour=out.convert('RGB')
        for key,enhancer in [('saturation',ImageEnhance.Color),('contrast',ImageEnhance.Contrast),('value',ImageEnhance.Brightness)]:
            colour=enhancer(colour).enhance(grade.get(key,1))
        out=colour.convert('RGBA');out.putalpha(alpha)
    if median:
        alpha=out.getchannel('A')
        out=out.convert('RGB').filter(ImageFilter.MedianFilter(median)).convert('RGBA')
        out.putalpha(alpha)
    native=Image.new('RGBA',size)
    native.putdata([ink(c[:3])+(255,) if c[3]>=128 else (0,0,0,0) for c in out.get_flattened_data()])
    return out,native


def run(slot, req, args, status, duplicate):
    source=getattr(args,'source_dir',None) or SOURCES/slot/'v01'; sheet_path=source/'source-sheet.png'
    measured=source/'source-sheet.json'
    if measured.exists():
        meta=json.loads(measured.read_text())
    else:
        assert req['role'] in ('backdrop','plate'), 'missing measured pieces for isolated subjects'
        im=Image.open(sheet_path).convert('RGBA'); bounds=im.getchannel('A').point(lambda a:255 if a>=128 else 0).getbbox()
        assert bounds, 'empty backdrop'
        x,y,r,b=bounds
        meta={'pieces':[{'index':0,'source_rect_xywh':[x,y,r-x,b-y],'alpha_area':sum(a>=128 for a in im.getchannel('A').get_flattened_data())}], 'warnings':['Missing crop metadata: inferred single backdrop alpha bounds']}
    notes=(source/'notes.md').read_text()
    sheet=Image.open(sheet_path).convert('RGBA'); dest=OUTPUT/slot; map_path=dest/(slot+'.map.json')
    m=json.loads(map_path.read_text()) if map_path.exists() else initial_map(slot,req,meta,notes,sheet,source)
    m.setdefault('anchor_canvas_wh',list(map(int,re.search(r'(\d+)\s*[×x]\s*(\d+)',req['fit']).groups())) if re.search(r'(\d+)\s*[×x]\s*(\d+)',req['fit']) else m['sizes']['v1'].copy())
    for kit,height in [('v1',args.v1_height),('v2',args.v2_height)]:
        if height:
            w,h=m['sizes'][kit]; m['sizes'][kit]=[round(w*height/h),height]
    validate(m,req,meta,sheet_path)
    write(map_path,js(m),args.check)
    crops=[]; metrics=[]; pivots=[]
    for p in m['pieces']:
        x,y,w,h=p['source_rect_xywh']; c=sheet.crop((x,y,x+w,y+h));c,pivot=registered_crop(p,c);clean,metric=despill(c)
        crops.append(clean);metrics.append(metric);pivots.append(pivot)
    output={}; flags=[]
    if len(crops)!=m['expected_existing_pieces']:flags.append('piece-count mismatch')
    if not re.search(r'(\d+)\s*[×x]\s*(\d+)',req['fit']):flags.append('unknown fit; provisional 48px canvas')
    if not measured.exists():flags.append('inferred backdrop bounds; missing metadata')
    if meta.get('warnings'):flags.append('delivery crop warnings')
    if not all(p['approved'] for p in m['pieces']):flags.append('unreviewed mapping/components')
    if '?' in status or 'FOREIGN' in status or duplicate:flags.append('foreign/duplicate suspicion'+(' '+duplicate if duplicate else ''))
    if req['role']!='plate' and any(x==0 or y==0 or x+w>=sheet.width or y+h>=sheet.height for x,y,w,h in [p['source_rect_xywh'] for p in m['pieces']]):flags.append('sheet-edge contact')
    if any(v['fringe_after_boundary'] for v in metrics):flags.append('fringe residue')
    terminal={}
    if args.terminal_metrics:
        all_metrics=json.loads(args.terminal_metrics.read_text()); terminal=all_metrics.get(slot,{})
        if not terminal:flags.append('missing real terminal metrics')
    for kit,size in m['sizes'].items():
        # All actor poses share one scale, baseline and native pivot. No pose is stretched.
        basis=m.get('anchor_canvas_wh') or list(map(int,re.search(r'(\d+)\s*[×x]\s*(\d+)',req['fit']).groups()))
        anchor=[round(m['anchor_xy'][0]*size[0]/basis[0]),round(m['anchor_xy'][1]*size[1]/basis[1])]
        scales=[]
        for c,pivot in zip(crops,pivots):
            px,py=pivot
            ratios=[1.0]
            for reach,space in [(px,anchor[0]),(c.width-px,size[0]-anchor[0]),(py,anchor[1]),(c.height-py,size[1]-anchor[1])]:
                if reach>0:ratios.append(space/reach)
            scales.append(min(ratios))
        references=set(m.get('scale_reference_pieces', [p['piece'] for p in m['pieces']]))
        shared=min(s for s,p in zip(scales,m['pieces']) if p['piece'] in references)
        for i,(c,p) in enumerate(zip(crops,m['pieces'])):
            canvas=p.get('sizes',{}).get(kit,size)
            local_anchor=[round(anchor[0]*canvas[0]/size[0]),round(anchor[1]*canvas[1]/size[1])]
            if 'anchor_fraction' in p:
                local_anchor=[round(v*n) for v,n in zip(p['anchor_fraction'],canvas)]
            scale=shared if m['registration'] in ('shared-feet','shared-pivot') else scales[i]
            if canvas!=size:
                px,py=pivots[i]
                scale=min([1.0]+[space/reach for reach,space in [(px,local_anchor[0]),(c.width-px,canvas[0]-local_anchor[0]),(py,local_anchor[1]),(c.height-py,canvas[1]-local_anchor[1])] if reach>0])
            ref,native=sample(c,tuple(canvas),pivots[i],local_anchor,scale,p.get('sampling','contain'),p.get('grade'),p.get('inset_fraction',0),p.get('trim_alpha_bounds',False),p.get('median_filter',0))
            native, reviewed_mask, controls_hash = reviewed_native(ref,native,p,kit,dest,scale,local_anchor)
            path=f'{kit}/{p["name"]}.png';data=png(native);write(dest/path,data,args.check)
            if req['role']=='player knight' or reviewed_mask is not None:
                mask=Image.new('RGBA',tuple(canvas))
                mask.putdata([(min(3,max(0,(max(c[:3])-50)//45))*70,)*3+(255,) if c[3]>=128 and c[2]>c[0]+18 and c[2]>c[1]+8 else (0,0,0,0) for c in ref.get_flattened_data()])
                if reviewed_mask is not None: mask = reviewed_mask
                write(dest/f'{kit}/{p["name"]}-seat-mask.png',png(mask),args.check)
            pixels=list(native.get_flattened_data());opaque=sum(c[3]>0 for c in pixels)
            common=[(a,b) for a,b in zip(ref.get_flattened_data(),pixels) if b[3]]
            agreement=sum(sum((x-y)**2 for x,y in zip(a[:3],b[:3]))<=3600 for a,b in common)/max(1,len(common))
            # This is a reduction diagnostic, not a fake terminal writer.
            reduced=native.resize((max(1,canvas[0]//4),max(1,canvas[1]//4)),Image.Resampling.BOX)
            cell_count=sum(c[3]>=64 for c in reduced.get_flattened_data())
            if agreement<.90:flags.append('source colour fidelity below 0.90')
            if opaque<4 or cell_count<2:flags.append('weak v1 cell silhouette' if kit=='v1' else 'weak v2 silhouette')
            output[path]=dict(sha256=hashlib.sha256(data).hexdigest(),canvas_wh=canvas,anchor_xy=local_anchor,source_scale=scale,opaque_pixels=opaque,half_cell_coverage=cell_count,colour_agreement=round(agreement,4),piece=p['piece'],state=p['state'],frame=p['frame'],approved=p['approved'])
            if req['role']=='player knight' or reviewed_mask is not None:output[path]['seat_mask_sha256']=digest(dest/f'{kit}/{p["name"]}-seat-mask.png')
            if controls_hash: output[path]['reviewed_processing_sha256']=controls_hash
    verdict='needs-rerun' if any('suspicion' in f or 'sheet-edge' in f or 'fidelity' in f for f in flags) else 'needs-manual-map' if flags else 'OK'
    record=dict(tool_version=VERSION,slot=slot,source_path=str(sheet_path),source_sha256=digest(sheet_path),metadata_sha256=digest(measured) if measured.exists() else None,notes_sha256=digest(source/'notes.md'),map_sha256=digest(map_path),resample='BOX premultiplied alpha',binary_alpha_threshold=128,palette='shared 75 inks; black/signal ivory excluded',registration=m['registration'],flags=sorted(set(flags)),quality=verdict,despill=metrics,outputs=output,runtime=m['runtime'])
    if terminal:
        for path,value in terminal.items():
            assert value['native_sha256']==output[path]['sha256'], f'stale terminal capture: {slot}/{path}'
            if value['weak']:flags.append('weak actual terminal silhouette')
        record['terminal_96_columns']=terminal
        record['flags']=sorted(set(flags))
        if any(value['weak'] for value in terminal.values()) and record['quality']=='OK':record['quality']='needs-manual-map'
    previous=dest/'slot.json'
    if getattr(args,'source_dir',None) and previous.exists():
        prior=json.loads(previous.read_text())
        if args.check and prior['source_sha256'] == record['source_sha256']:
            record['source_path'] = prior['source_path']
        if prior.get('incoming_delivery'):
            record['incoming_delivery']=prior['incoming_delivery']
            record['flags']=sorted(set(record['flags']+['delivery changed/incomplete; retain previous mapped assets']))
            if record['quality']=='OK':record['quality']='needs-manual-map'
    write(dest/'slot.json',js(record),args.check)
    if not args.check:
        for kit in m['sizes']:
            names={Path(path).name for path in output if path.startswith(kit+'/')}
            names|={Path(path).stem+'-seat-mask.png' for path,value in output.items() if path.startswith(kit+'/') and 'seat_mask_sha256' in value}
            for old in (dest/kit).glob('*.png'):
                if old.name not in names:old.unlink()
    # Twelve frame cells per page; kit images share one labelled frame cell.
    cw=min(440,max(160,max(m['sizes']['v1'][0],m['sizes']['v2'][0])+8))
    ch=max(110,min(260,max(m['sizes']['v1'][1],m['sizes']['v2'][1])+28))
    for start in range(0,len(m['pieces']),12):
        chunk=m['pieces'][start:start+12]
        review=Image.new('RGB',(cw*min(4,len(chunk)),ch*((len(chunk)+3)//4)),(11,13,17));d=ImageDraw.Draw(review)
        for i,p in enumerate(chunk):
            im=Image.open(dest/'v1'/(p['name']+'.png')).convert('RGBA');im.thumbnail((cw-8,ch-28),Image.Resampling.NEAREST)
            x=i%4*cw;y=i//4*ch;review.paste(im,(x+4,y+24),im);d.text((x+4,y+4),f'{p["piece"]}: {p["name"]}'[:50],fill='#b0b2a8')
        write(dest/f'review-{start//12+1}.png',png(review),args.check)
    if not args.check and (dest/'review.png').exists():(dest/'review.png').unlink()
    print(f'{slot}: {verdict}; {len(crops)} pieces; '+', '.join(sorted(set(flags))))
    return record


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('slot',nargs='?');p.add_argument('--all',action='store_true');p.add_argument('--v1-height',type=int);p.add_argument('--v2-height',type=int);p.add_argument('--check',action='store_true');p.add_argument('--quality',type=Path);p.add_argument('--terminal-metrics',type=Path);p.add_argument('--source-dir',type=Path,help='reproduce an existing map from its SHA-matching archived delivery');p.add_argument('--resume',action='store_true',help='reuse SHA-matching staged outputs; still detect changed deliveries')
    p.add_argument('--requests',type=Path,help='additional request manifest, with the same games/assets schema')
    a=p.parse_args();assert bool(a.slot)!=a.all, 'choose slot or --all'
    assert not (a.source_dir and a.all), '--source-dir selects one SHA-bound delivery'
    req=requests(a.requests);status=json.loads((SOURCES/'status.json').read_text())
    names=[s for s,v in status.items() if v.startswith('OK') and s in req] if a.all else [a.slot]
    hashes={};records=[];room_hashes=[]
    for s in names:
        path=(a.source_dir or SOURCES/s/'v01')/'source-sheet.png';h=digest(path) if path.exists() else None;duplicate=hashes.get(h,'') if h else '';hashes[h]=s
        if req[s]['role']=='backdrop' and path.exists():
            gray=Image.open(path).convert('L').resize((9,8),Image.Resampling.BOX)
            signature=sum((gray.getpixel((x,y))>gray.getpixel((x+1,y)))<<(y*8+x) for y in range(8) for x in range(8))
            similar=[other for other,bits in room_hashes if (bits^signature).bit_count()<=4]
            if similar:duplicate='near-identical room layout: '+', '.join(similar)
            room_hashes.append((s,signature))
        previous=OUTPUT/s/'slot.json'
        old=json.loads(previous.read_text()) if previous.exists() else None
        delivery=a.source_dir or SOURCES/s/'v01'
        meta_path=delivery/'source-sheet.json';notes_path=delivery/'notes.md';map_path=OUTPUT/s/(s+'.map.json')
        same=old and old['source_sha256']==h and old.get('metadata_sha256')==(digest(meta_path) if meta_path.exists() else None) and notes_path.exists() and old['notes_sha256']==digest(notes_path) and old['map_sha256']==digest(map_path)
        has_reviewed_controls = map_path.exists() and any(p.get('reviewed_processing') for p in json.loads(map_path.read_text())['pieces'])
        if a.resume and same and not a.check and not has_reviewed_controls:
            records.append(old);print(s+': reused SHA-matching native outputs');continue
        try:
            records.append(run(s,req[s],a,status.get(s,''),duplicate))
        except (AssertionError,FileNotFoundError) as error:
            if a.check:raise
            if not old:raise
            old['quality']='needs-manual-map'
            old['flags']=sorted(set(old['flags']+['delivery changed/incomplete; retain previous mapped assets']))
            old['incoming_delivery']={'source_path':str(path),'source_sha256':h,'metadata_sha256':digest(meta_path) if meta_path.exists() else None,'reason':str(error)}
            # A production rerun may archive its old v01. Keep the original hash
            # bound to the archival path rather than attributing it to new art.
            if old['source_sha256']!=h:
                for archived in (SOURCES/'_old').glob('*/v01/source-sheet.png'):
                    if digest(archived)==old['source_sha256']:old['source_path']=str(archived);break
            write(previous,js(old));records.append(old);print(s+': delivery changed; retained previous assets for manual map review')
    if a.quality:
        counts=Counter(r['quality'] for r in records)
        rows=['# Slot import quality','',f'{len(records)} verified-status deliveries staged: '+', '.join(f'{k} {v}' for k,v in sorted(counts.items()))+'.','Diagnostics are flags, not art approval. Reading-order maps require human review; runtime activation is separate.','', '| Slot | Status | Pieces actual/request | Flags |','|---|---|---:|---|']
        for r in records:
            m=json.loads((OUTPUT/r['slot']/(r['slot']+'.map.json')).read_text());rows.append(f'| {r["slot"]} | {r["quality"]} | {m["delivered_pieces"]}/{m["expected_existing_pieces"]} | '+('; '.join(r['flags']) or 'Reviewed native/cells; source retained')+' |')
        rows+=['','FOREIGN/NOSHEET deliveries are excluded by --all. New OK deliveries are picked up on the next run.','The half-cell metric is diagnostic only; the pilots use the real cockpit writer (see review images).']
        write(a.quality, ('\n'.join(rows)+'\n').encode(),a.check)

if __name__=='__main__':main()
