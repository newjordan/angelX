"""One-command quick-rig gate, with a fresh native Rust baseline by default."""
import argparse
import os
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from spriterig import native as N
from tools.quick_quality import review


def run(game, rig, image, out, *, native=None, root=None):
    root = N.root_path(root)
    out = Path(out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    if native is None:
        native = out / 'native' / 'frames.json'
        native.parent.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        env.update(WORN_NATIVE_LEGACY='1', WORN_NATIVE_OUT=str(native))
        env.setdefault('CARGO_TARGET_DIR', str(root / 'crates' / 'target' / 'quick-rig'))
        env.setdefault('CARGO_BUILD_JOBS', '4')
        # A failed export is a prerequisite failure, never a zero quality score.
        subprocess.run(['cargo', 'test', '--manifest-path', str(root/'crates/Cargo.toml'),
                        '-p', 'angel-delve', '--profile', 'fast', 'export_worn_native'],
                       cwd=root, env=env, check=True)
    return review(game, native, out, rig=rig, image=image, root=root)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', required=True)
    parser.add_argument('--rig', required=True)
    parser.add_argument('--input', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--native', help='Explicit existing Rust baseline; skips fresh Rust export')
    parser.add_argument('--angelx', help='Checkout owning native art and palette')
    args = parser.parse_args(argv)
    try:
        report = run(args.game, args.rig, args.input, args.out,
                     native=args.native, root=args.angelx)
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        parser.exit(2, f'Prerequisite failed: {exc}\n')
    print(f'{"PASS" if report["ok"] else "FAIL"}: {Path(args.out)/"checks.json"}')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
