#!/usr/bin/env python3
"""Effect integration contracts: palette, phase slots, exact warning coverage and provenance."""
import json
import runpy
import unittest
from PIL import Image
M=runpy.run_path('scripts/boss-effect-import.py')

class Effects(unittest.TestCase):
 def test_flight_exposure_hue_and_geometry_are_source_owned(self):
  manifest=json.loads(M['SPEC'].read_text())
  for boss in M['BOSSES']:
   entry=manifest[boss]; policy=entry['readability']
   self.assertGreaterEqual(policy['exposure'],1.0)
   record=json.loads((M['ASSETS']/boss/'effect-import.json').read_text())
   source=M['load_frames'](boss,'projectile',record['mapping']['projectile'])
   signals=M['projectile_signals'](source,policy)
   self.assertEqual(entry['projectile_signal_inks'],[list(c) for c in signals])
   self.assertTrue(all(M['LIGHTNESS'](c)>65 and max(c)<=242 and min(c)<198 for c in signals))
   for kit,role in [('v1','projectile'),('v2','projectile_v2')]:
    refs=M['registered'](source,entry['display'][kit],entry['source_socket']['projectile'])
    for name,ref in zip(entry[role],refs):
     im=Image.open(M['ASSETS']/boss/name).convert('RGBA')
     alpha=bytes(255 if c[3]>=128 else 0 for c in ref.get_flattened_data())
     self.assertEqual(im.getchannel('A').tobytes(),alpha)
     colours={c[:3] for c in im.get_flattened_data() if c[3]}
     self.assertTrue(set(signals)<=colours,(boss,kit,name,signals,colours))
     self.assertLessEqual(len(colours),12)
     if boss=='mother-of-spores':
      self.assertTrue(all(c[0]>c[1] and c[2]>c[1] for c in colours))
   flight=[f for f in record['outputs'] if 'projectile' in f['file']]
   self.assertTrue(all(f['source_exposure']>=1.0 for f in flight))

 def test_registered_banks_keep_point_warning_geometry_and_source_hashes(self):
  palette=set(M['lib'].palette())
  manifest=json.loads(M['SPEC'].read_text())
  for boss,(v1,v2,_) in M['BOSSES'].items():
   entry=manifest[boss]
   v1=entry['display']['v1'];v2=entry['display']['v2']
   if 'point_rim' in entry:
    levels=[M['LIGHTNESS'](c) for c in entry['point_rim']]
    self.assertEqual(levels,sorted(set(levels)))
    self.assertTrue(all(tuple(c) in palette and M['LIGHTNESS'](c)<=80 for c in entry['point_rim']))
   for role,count in [('projectile',2),('projectile_v2',2),('tell',4),('impact',5)]:
    self.assertEqual(len(entry[role]),count)
    for i,name in enumerate(entry[role]):
     im=Image.open(M['ASSETS']/boss/name).convert('RGBA')
     record=json.loads((M['ASSETS']/boss/'effect-import.json').read_text())
     allowed=set(map(tuple,record['projectile_inks'])) if role.startswith('projectile') else palette
     self.assertTrue(all(c[3] in (0,255) and (not c[3] or c[:3] in allowed) for c in im.get_flattened_data()))
     if role=='tell':
      self.assertTrue(all(not c[3] or M['LIGHTNESS'](c[:3])<=80 for c in im.get_flattened_data()))
      old=Image.open(M['ASSETS']/entry.get('legacy_mask_boss',boss)/('tell'+(f'-{i}' if i else '')+'.png')).convert('RGBA')
      self.assertEqual(im.getchannel('A').tobytes(),old.getchannel('A').tobytes(),boss)
     if role.startswith('projectile'):self.assertEqual(list(im.size),v2 if role.endswith('v2') else v1)
   record=json.loads((M['ASSETS']/boss/'effect-import.json').read_text())
   for f in record['outputs']:self.assertEqual(M['digest'](M['ASSETS']/boss/f['file']),f['sha256'])
   for role in ('projectile','tell','impact'):
    folder=M['RETAINED']/boss/role; rec=json.loads((folder/'provenance.json').read_text())
    for f in rec['frames']:self.assertEqual(M['digest'](folder/f['file']),f['sha256'])
 def test_rejected_opaque_tells_keep_byte_exact_legacy_masks(self):
  manifest=json.loads(M['SPEC'].read_text())
  for boss,entry in manifest.items():
   for role,reason in entry.get('rejected_roles',{}).items():
    self.assertEqual(role,'tell');self.assertIn('opaque checkerboard',reason)
    folder=M['RETAINED']/boss/role
    record=json.loads((folder/'provenance.json').read_text())
    self.assertEqual(len(record['frames']),1)
    for i,name in enumerate(entry[role]):
     original=M['ASSETS']/entry['legacy_mask_boss']/('tell'+(f'-{i}' if i else '')+'.png')
     self.assertEqual((M['ASSETS']/boss/name).read_bytes(),original.read_bytes())

 def test_source_socket_stays_on_the_same_canvas_pixel_across_sizes(self):
  source=Image.new('RGBA',(40,20))
  for y in range(7,13):
   for x in range(17,23):source.putpixel((x,y),(160,120,80,255))
  for size in (12,24,40):
   ref=M['registered']([source],[size,size],[.5,.5])[0]
   self.assertGreater(ref.getpixel((size//2,size//2))[3],0)

if __name__=='__main__':unittest.main()
