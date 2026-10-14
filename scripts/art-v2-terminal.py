#!/usr/bin/env python3
"""Show the application's serialized terminal buffers in Kitty and capture PNGs.

Requires a running Wayland compositor, kitty, grim and hyprctl. Uses an isolated
review window on the existing ANGELCAP headless monitor; never edits config.
"""
import argparse
import atexit
import fcntl
import json
from pathlib import Path
import struct
import subprocess
import sys
import time
import uuid


def surface(directory):
    request = directory / 'terminal-request.json'
    ready = directory / 'terminal-ready.json'
    sequence = None
    print('\x1b[?25l\x1b[2J', end='', flush=True)
    while True:
        try:
            job = json.loads(request.read_text())
        except (OSError, json.JSONDecodeError):
            time.sleep(.05)
            continue
        if job.get('stop'):
            return
        if job['sequence'] != sequence:
            sequence = job['sequence']
            payload = Path(job['path']).read_text()
            sys.stdout.write('\x1b_Ga=d,d=A\x1b\\' + payload)
            sys.stdout.flush()
            rows, columns, width, height = struct.unpack('HHHH', fcntl.ioctl(sys.stdout.fileno(), 0x5413, bytes(8)))
            ready.write_text(json.dumps(dict(sequence=sequence, columns=columns, rows=rows, width=width, height=height)))
        time.sleep(.05)


def run(directory):
    for name in ['terminal-request.json','terminal-ready.json']:
        (directory/name).unlink(missing_ok=True)
    clients = lambda: json.loads(subprocess.check_output(['hyprctl', '-j', 'clients']))
    monitors = json.loads(subprocess.check_output(['hyprctl', '-j', 'monitors']))
    monitor = next(m for m in monitors if m['name'] == 'ANGELCAP')
    address = None
    suffix=uuid.uuid4().hex
    review_class='angel-art-review-'+suffix
    rule_handle='angel_art_review_rule_'+suffix
    rule=rule_handle+'=hl.window_rule({name='+json.dumps(review_class)+',match={class='+json.dumps('^'+review_class+'$')+'},opacity="1.0 override 1.0 override",opaque=true,no_anim=true})'
    subprocess.run(["hyprctl","eval",rule],check=True,stdout=subprocess.DEVNULL)
    def cleanup_rule():
        subprocess.run(["hyprctl","eval",f"if {rule_handle} then {rule_handle}:set_enabled(false); {rule_handle}=nil end"],stdout=subprocess.DEVNULL)
    atexit.register(cleanup_rule)
    command = ['kitty', '--class', review_class, '--title', 'angelX art review',
               '--config', 'NONE', '-o', 'font_family=Liberation Mono', '-o', 'font_size=12',
               '-o', 'background=#000000', '-o', 'foreground=#a7b4b3',
               '-o', 'background_opacity=1.0', '-o', 'background_image=none',
               '-o', 'modify_font=cell_height 1px',
               '-o', 'window_padding_width=0', '-o', 'shell_integration=disabled',
               '-o', 'enable_audio_bell=no', '-o', 'allow_remote_control=no',
               sys.executable, str(Path(__file__).resolve()), str(directory), '--surface']
    import shlex
    rules = f'[workspace {monitor["activeWorkspace"]["id"]} silent; float; size 964 564; nofocus] '
    subprocess.run(['hyprctl', 'dispatch', 'hl.dsp.exec_cmd(' + json.dumps(rules + shlex.join(command)) + ')'], check=True, stdout=subprocess.DEVNULL)
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        own = next((c for c in clients() if c['class'] == review_class), None)
        if own:
            address = own['address']
            break
        time.sleep(.1)
    if not address:
        raise RuntimeError('Kitty review window did not open')
    time.sleep(1.25)  # Let the review window's opening animation settle.
    request = directory / 'terminal-request.json'
    ready = directory / 'terminal-ready.json'
    try:
        for sequence, stream in enumerate(sorted(directory.glob('*.ansi')), 1):
            request.write_text(json.dumps(dict(sequence=sequence, path=str(stream))))
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                try:
                    info = json.loads(ready.read_text())
                    if info['sequence'] == sequence:
                        break
                except (OSError, json.JSONDecodeError):
                    pass
                time.sleep(.05)
            else:
                raise RuntimeError(f'Kitty did not show {stream.name}')
            time.sleep(.35)
            own = next(c for c in clients() if c['address'] == address)
            # Zero-padding Kitty: remove the compositor's two-pixel border.
            cw, ch = info['width'] // info['columns'], info['height'] // info['rows']
            if (cw,ch)!=(10,20):
                raise RuntimeError(f'Review font differs from the application fixture: {cw}x{ch}')
            if info['columns'] < 96 or info['rows'] < 28:
                raise RuntimeError(f'Review window too small: {info}')
            border_x = (own['size'][0] - info['width']) // 2
            border_y = (own['size'][1] - info['height']) // 2
            x, y = own['at'][0] + border_x, own['at'][1] + border_y
            geometry = f'{x},{y} {96*cw}x{28*ch}'
            output = stream.with_suffix('.png')
            subprocess.run(['grim', '-g', geometry, str(output)], check=True)
            print(output.name, geometry)
        (directory / 'terminal-capture.json').write_text(json.dumps(dict(
            terminal='Kitty', font='Liberation Mono 12', cells=[96,28], cell_pixels=[cw,ch],
            transport='Viewer.render_game_img / Viewer.render_map_pixels PNG upload + Unicode placeholders',
            fallback='paint_room_ui / art.paint_cells half blocks + ride.frame_to_braille_graded',
            screenshot='grim; real Wayland surface',
        ), indent=2))
    finally:
        request.write_text(json.dumps(dict(stop=True)))
        cleanup_rule()
        atexit.unregister(cleanup_rule)
        selector=json.dumps('address:'+address)
        subprocess.run(['hyprctl','eval','local w=hl.get_window('+selector+'); if w then hl.dispatch(hl.dsp.window.signal({window=w,signal=15})) end'],stdout=subprocess.DEVNULL,check=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--surface', action='store_true')
    args = parser.parse_args()
    (surface if args.surface else run)(args.directory.resolve())
