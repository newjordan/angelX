"""Create the small, deliberately asymmetric authoring example (not body art)."""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from PIL import Image
from spriterig.native import root_path, rows_rgba
from spriterig.delve.source import palette

ROWS = ['..W....W.', '.WJWWWWJ.', 'WJhhhhJhW', '.JhhJhhJ.', '..JJJJJ..']

if __name__ == '__main__':
    path = Path(__file__).with_name('odd_helm.png')
    Image.fromarray(rows_rgba(ROWS, palette(root_path()))).save(path)
    print(path)
