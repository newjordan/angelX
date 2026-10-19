#!/usr/bin/env python3
"""Compose bounded native room reviews and the actual Brasswake ANSI buffers."""
from pathlib import Path
from PIL import Image
import argparse,importlib.util
spec=importlib.util.spec_from_file_location('review',Path(__file__).with_name('cinder-review.py'))
r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);a=p.parse_args();root=a.root
r.sheet([(root/'rooms'/(n+'.png'),n) for n in ['idle','slam-tell','slam','rush-tell','rush','flinch','damage-1','damage-2','defeat','guard','kick','down']],root/'room-states-1x.png')
r.sheet([(root/'rooms'/(n+'.png'),n) for n in ['entrance','throw','live-seat-1','live-seat-2','live-seat-3','live-seat-4','podium']],root/'room-live-1x.png')
r.sheet([(root/'rooms'/(n+'.png'),n) for n in ['wave-1','wave-2','wave-3','idle']],root/'whole-stage-1x.png',4)
if (root/'terminal').exists():r.ansi([root/'terminal'/(n+'-cells.ansi') for n in ['idle','slam-tell','slam','rush-tell','rush','flinch','damage-1','damage-2','defeat','throw','down','podium']],root/'terminal-review.html')
