#!/usr/bin/env python3
"""Make bounded 1x room sheets and HTML from Latch Duel's real ANSI buffer."""
import argparse, importlib.util
from PIL import Image
from pathlib import Path
spec=importlib.util.spec_from_file_location('cinder_review',Path(__file__).with_name('cinder-review.py'))
review=importlib.util.module_from_spec(spec);spec.loader.exec_module(review)
p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);a=p.parse_args();r=a.root
states=['idle','high-tell','high-window','low-tell','low-window','high-miss','low-miss','perfect','chain','damage-1','damage-2','defeat']
review.sheet([(r/'rooms'/(n+'.png'),n) for n in states],r/'room-states-1x.png')
review.sheet([(r/'rooms'/(n+'.png'),n) for n in ['entrance','live-seat-1','live-seat-2','podium','podium-zero']],r/'room-live-1x.png',2)
cells=['high-tell','high-window','low-tell','low-window','perfect','chain','damage-1','damage-2','miss','podium','podium-zero','defeat']
review.ansi([r/'terminal'/(n+'-cells.ansi') for n in cells],r/'terminal-review.html')

poses=['high-window','low-window','thrust-brace','thrust-contact','thrust-recover']
review.sheet([(r/'rooms'/(n+'.png'),n) for n in poses]+[(r/'rooms/chain.png','chain')],r/'fight-1x.png')
crops=r/'pose-crops';crops.mkdir(exist_ok=True)
for n in poses:
    Image.open(r/'rooms'/(n+'.png')).crop((116,76,244,156)).save(crops/(n+'.png'))
review.sheet([(crops/(n+'.png'),n) for n in poses],r/'pose-pixels-3x.png',3,(384,240))
review.sheet([(r/'rooms'/(f'identity-{n}.png'),f'seat {n}') for n in range(1,5)],r/'seat-identities-1x.png',2)
