"""CLI: ``python -m spriterig.moves render|check``."""

import argparse
import json
from pathlib import Path

from PIL import Image

from .. import quick as Q
from .. import smartfit as S
from . import MoveError, apply, check, library, parse, resolve


GROUND = (0x26, 0x28, 0x2f, 255)
SCALE = 4


def repo_root():
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / 'delve').is_dir() and (parent / 'tools' / 'sprite-rig').is_dir():
            return parent
    return Path.cwd()


def _fit(sprite, root):
    return S.fit(str(sprite), root=root)


def _raw_frame(game_frame, root):
    return Q.frame_image(game_frame, root=root)


def _union_box(images, pad=3):
    boxes = [image.getbbox() for image in images]
    boxes = [box for box in boxes if box]
    if not boxes:
        return (0, 0, images[0].width, images[0].height)
    x0 = max(0, min(box[0] for box in boxes) - pad)
    y0 = max(0, min(box[1] for box in boxes) - pad)
    x1 = min(images[0].width, max(box[2] for box in boxes) + pad)
    y1 = min(images[0].height, max(box[3] for box in boxes) + pad)
    return (x0, y0, x1, y1)


def _pose_image(game_frame, root, scale, box=None):
    native = _raw_frame(game_frame, root)
    if box:
        native = native.crop(box)
    big = native.resize((native.width * scale, native.height * scale), Image.Resampling.NEAREST)
    sheet = Image.new('RGBA', big.size, GROUND)
    sheet.alpha_composite(big)
    return sheet


def render_sprite(sprite, moves, out, *, root=None, scale=SCALE):
    """Write game.json (moves in their slots), a contact sheet and a GIF per move."""
    root = Path(root) if root else repo_root()
    sprite = Path(sprite)
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    rig = _fit(sprite, root)
    # Later moves overwrite a shared slot in the playable game; the sheet keeps each.
    played = rig
    slot_owner = {}
    built = []
    for move in moves:
        played = apply(played, move)
        slot_owner[move['slot']] = move['name']
        one = apply(rig, move)
        game = Q.build(sprite, one, root=root, clips=['idle', move['slot']])
        Q.validate_game(game, root=root)
        built.append((move, game['clips'][move['slot']], game['canvas']))
    full = Q.build(sprite, played, root=root)
    Q.validate_game(full, root=root)
    (out / 'game.json').write_text(json.dumps(full, indent=2) + '\n', encoding='utf-8')
    (out / 'slots.json').write_text(json.dumps(slot_owner, indent=2) + '\n', encoding='utf-8')

    gap = 2
    row_gap = 4
    widths = []
    heights = []
    rows = []
    look = out / 'look'
    look.mkdir(exist_ok=True)
    for move, frames, canvas in built:
        raw = [_raw_frame(frame, root) for frame in frames]
        box = _union_box(raw)
        w, h = box[2] - box[0], box[3] - box[1]
        pieces = [_pose_image(frame, root, scale, box) for frame in frames]
        row_w = len(pieces) * w * scale + (len(pieces) - 1) * gap * scale
        row_h = h * scale
        row = Image.new('RGBA', (row_w, row_h), GROUND)
        x = 0
        for piece in pieces:
            row.alpha_composite(piece, (x, 0))
            x += piece.width + gap * scale
        rows.append(row)
        row.save(look / f'{move["name"]}.png')
        durations = [max(20, round(frame['ticks'] * 1000 / 30)) for frame in frames]
        rgb = [piece.convert('RGB') for piece in pieces]
        rgb[0].save(out / f'{move["name"]}.gif', save_all=True, append_images=rgb[1:],
                    duration=durations, loop=0, disposal=2)
        widths.append(row_w)
        heights.append(row_h)
    sheet = Image.new('RGBA', (max(widths), sum(heights) + row_gap * scale * (len(rows) - 1)), GROUND)
    y = 0
    for row in rows:
        sheet.alpha_composite(row, (0, y))
        y += row.height + row_gap * scale
    sheet.save(out / 'sheet.png')
    return out / 'sheet.png'


def render_reel(out, *, root=None, scale=SCALE, names=('frog', 'ogre', 'skeleton')):
    """Side-by-side reel: each library move played in turn on the named sprites."""
    root = Path(root) if root else repo_root()
    src = root / '.we-out' / 'src'
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    moves = library()
    columns = []
    for name in names:
        rig = _fit(src / f'{name}.png', root)
        clips = []
        for move in moves:
            game = Q.build(src / f'{name}.png', apply(rig, move), root=root, clips=['idle', move['slot']])
            clips.append(game['clips'][move['slot']])
        columns.append((rig['canvas'], clips))
    gap = 4 * scale
    cell_w = max(canvas[0] for canvas, _ in columns) * scale
    cell_h = max(canvas[1] for canvas, _ in columns) * scale
    width = cell_w * len(columns) + gap * (len(columns) - 1)
    frames = []
    durations = []
    for move_i, move in enumerate(moves):
        count = len(columns[0][1][move_i])
        for frame_i in range(count):
            plate = Image.new('RGB', (width, cell_h), GROUND[:3])
            for col, ((cw, ch), clips) in enumerate(columns):
                piece = _pose_image(clips[move_i][frame_i], root, scale).convert('RGB')
                x = col * (cell_w + gap) + (cell_w - cw * scale) // 2
                y = cell_h - ch * scale
                plate.paste(piece, (x, y))
            frames.append(plate)
            durations.append(max(20, round(move['frames'][frame_i]['ticks'] * 1000 / 30)))
    frames[0].save(out, save_all=True, append_images=frames[1:], duration=durations,
                   loop=0, disposal=2)
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(prog='spriterig.moves')
    sub = ap.add_subparsers(dest='command', required=True)
    render_ap = sub.add_parser('render', help='fit a sprite and render moves')
    render_ap.add_argument('sprite')
    render_ap.add_argument('moves', nargs='+')
    render_ap.add_argument('--out', required=True)
    render_ap.add_argument('--root')
    check_ap = sub.add_parser('check', help='parse and check move files')
    check_ap.add_argument('files', nargs='+')
    reel_ap = sub.add_parser('reel', help='side-by-side frog/ogre/skeleton reel')
    reel_ap.add_argument('--out', required=True)
    reel_ap.add_argument('--root')
    args = ap.parse_args(argv)
    if args.command == 'check':
        failed = 0
        for name in args.files:
            path = Path(name)
            try:
                check(parse(path.read_text(encoding='utf-8')))
            except MoveError as exc:
                failed += 1
                print(f'{path}: {exc}')
            else:
                print(f'{path}: ok')
        raise SystemExit(1 if failed else 0)
    root = Path(args.root) if args.root else repo_root()
    if args.command == 'reel':
        print(render_reel(args.out, root=root))
        return
    moves = [resolve(name) for name in args.moves]
    print(render_sprite(args.sprite, moves, args.out, root=root))


if __name__ == '__main__':
    main()
