"""spriterig: rapid CPU 2D sprite skinner/animator (numpy + Pillow)."""
__version__ = "0.2.0"
from .segment import load_sprite, segment
from .rig import build_rig
from .anim import ANIMS, DEFAULT_SET
from .render import render_frames
