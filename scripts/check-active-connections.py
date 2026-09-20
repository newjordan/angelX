#!/usr/bin/env python3
"""Static connection checks for explicitly active source roots; never traverse archives.

This detects missing literal JS imports/URL targets, declared Rust source targets,
and Rust files with no module/include/bin anchor. It is not a runtime reachability
proof: dynamic imports, same-named modules, cfg and runtime behavior need tests.
"""
import argparse
import json
from pathlib import Path
import os
import re
import tomllib

ROOTS = ('cockpit/src', 'scripts', 'lib', 'tests', 'bin')
SKIP = {'off-limits', 'target', 'node_modules', '__pycache__'}
JS_REF = re.compile(r'''(\bfrom\s+|\bimport\s*\(|\bimport\s+|\bnew\s+URL\(\s*)['"](\.[^'"\n]+)['"]''')
RUST_REF = re.compile(r'''(?:include!\s*\(|#\[path\s*=\s*)"([^"\n]+\.rs)"''')


def safe_file(root, path, allow_directory=False):
    """Check ancestors before opening or statting a source or referenced target."""
    absolute = Path(os.path.abspath(path))
    try:
        parts = absolute.relative_to(root).parts
    except ValueError:
        return False
    current = root
    for part in parts:
        if part in SKIP or part.startswith('.'):
            return False
        current /= part
        if current.is_symlink():
            return False
    return current.is_file() or (allow_directory and current.is_dir())


def audit(root):
    root = root.resolve()
    sources = {}
    for base in ROOTS:
        directory = root / base
        if not safe_file(root, directory, allow_directory=True):
            continue
        for parent, dirs, files in os.walk(directory, followlinks=False):
            dirs[:] = sorted(d for d in dirs if d not in SKIP and not d.startswith(('.', 'target'))
                             and not (Path(parent) / d).is_symlink())
            for name in sorted(files):
                path = Path(parent) / name
                if path.suffix in {'.rs', '.js', '.mjs'} and safe_file(root, path):
                    sources[path.relative_to(root).as_posix()] = path.read_text()
    missing = []
    refs = set()
    modules = set()
    for name, content in sources.items():
        path = root / name
        if path.suffix == '.rs':
            modules.update(re.findall(r'\bmod\s+(\w+)\s*;', content))
            targets = [(ref, False) for ref in RUST_REF.findall(content)]
        else:
            targets = [(ref, kind.startswith('new')) for kind, ref in JS_REF.findall(content)]
        for target, allow_directory in targets:
            resolved = Path(os.path.abspath(path.parent / target))
            if not safe_file(root, resolved, allow_directory):
                missing.append({'source': name, 'target': target})
            else:
                refs.add(resolved.relative_to(root).as_posix())
    manifest = root / 'cockpit/Cargo.toml'
    if safe_file(root, manifest):
        bins = tomllib.loads(manifest.read_text()).get('bin', [])
        for item in bins:
            target = manifest.parent / item.get('path', 'src/main.rs')
            if safe_file(root, target):
                refs.add(target.relative_to(root).as_posix())
            else:
                missing.append({'source': 'cockpit/Cargo.toml', 'target': item.get('path')})
    unanchored = sorted(name for name in sources if name.startswith('cockpit/src/')
                        and name.endswith('.rs') and Path(name).name != 'mod.rs'
                        and Path(name).stem not in modules and name not in refs)
    return {'schema': 'angel.active-connections/v1',
            'scope': list(ROOTS), 'source_files_checked': len(sources),
            'missing_literal_targets': missing, 'rust_files_without_anchor': unanchored,
            'limits': 'Static source connections only; compiler and runtime tests remain required.',
            'ok': not missing and not unanchored}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    report = audit(args.root)
    print(json.dumps(report, indent=2))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
