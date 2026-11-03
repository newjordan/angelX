#!/usr/bin/env python3
"""Build the cockpit's existing host-native reload modules, without Cargo.

Opt in at launch with ANGEL_NATIVE_MODULES=1; publish with angelX build --modules.
Each library is replaced atomically, never overwritten under a running dlopen.
"""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time

MODULES = ('skill_router', 'activity_classifier', 'verify_shell', 'moa_router')
FLAGS = ('-std=c11', '-O2', '-fPIC', '-fno-fast-math', '-ffp-contract=off', '-frounding-math')


def build_jobs():
    setting = os.environ.get('ANGEL_NATIVE_BUILD_JOBS')
    if setting is not None:
        try:
            jobs = int(setting)
        except ValueError:
            raise ValueError('ANGEL_NATIVE_BUILD_JOBS must be a positive integer') from None
        if jobs < 1:
            raise ValueError('ANGEL_NATIVE_BUILD_JOBS must be a positive integer')
    else:
        jobs = getattr(os, 'process_cpu_count', os.cpu_count)() or 1
    # This standalone launcher does not borrow jobserver tokens. Under a parent
    # make/Cargo jobserver, keep its implicit single slot instead of oversubscribing.
    if any('jobserver' in os.environ.get(key, '') for key in ('CARGO_MAKEFLAGS', 'MAKEFLAGS', 'MFLAGS')):
        return 1
    return min(jobs, len(MODULES))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def compiler_command():
    setting = os.environ.get('CC') or 'cc'
    command = [setting] if Path(setting).is_file() else shlex.split(setting)
    if not command:
        raise ValueError('CC must name a host C compiler')
    # Include executable identities, including a compiler behind a wrapper.
    identities = []
    for word in command:
        resolved = shutil.which(word)
        if resolved:
            path = Path(resolved).resolve()
            meta = path.stat()
            identities.append((str(path), meta.st_dev, meta.st_ino, meta.st_size, meta.st_mtime_ns, meta.st_ctime_ns))
    if not identities:
        raise ValueError('no host C compiler found; set CC')
    command[0] = shutil.which(command[0]) or command[0]
    return command, identities


def build(root):
    started = time.perf_counter()
    jobs = build_jobs()
    out = root / 'cockpit/target/native'
    out.mkdir(parents=True, exist_ok=True)
    # flock releases on failure/crash; simultaneous builders cannot publish stale
    # cache entries over one another or mutate the same output image.
    with (out / '.build.lock').open('a') as lease:
        fcntl.flock(lease, fcntl.LOCK_EX)
        compiler, identities = compiler_command()
        flags = [*FLAGS, *shlex.split(os.environ.get('CFLAGS', ''))]
        flags.append('-dynamiclib' if sys.platform == 'darwin' else '-shared')
        recipe = json.dumps([compiler, identities, flags, sys.platform], sort_keys=True).encode() + Path(__file__).read_bytes()
        pending = []
        originals = {}
        try:
            for name in MODULES:
                source = root / 'cockpit/native' / f'{name}.c'
                original = source.read_bytes()
                originals[source] = original
                key = digest(recipe + original)
                image = out / f'{name}.so'
                stamp = out / f'{name}.json'
                try:
                    saved = json.loads(stamp.read_text())
                    if saved == {'input': key, 'output': digest(image.read_bytes())}:
                        continue
                except (OSError, ValueError):
                    pass
                fd, temporary = tempfile.mkstemp(prefix=f'.{name}-', suffix='.tmp', dir=out)
                os.close(fd)
                pending.append((source, Path(temporary), image, stamp, key))

            def compile_one(entry):
                source, temporary, _, _, key = entry
                subprocess.run([*compiler, *flags, str(source), '-o', str(temporary), '-lm'], check=True)
                return {'input': key, 'output': digest(temporary.read_bytes())}

            # The executor joins all workers, including after a compiler fails.
            # No library is published until every compilation and input check
            # succeeds. A live dlopen generation is never modified in place.
            if pending:
                # Keep pool/thread imports off the common all-unchanged path.
                from concurrent.futures import ThreadPoolExecutor

                with ThreadPoolExecutor(max_workers=min(jobs, len(pending))) as pool:
                    records = list(pool.map(compile_one, pending))
                for source, original in originals.items():
                    if source.read_bytes() != original:
                        raise ValueError(f'{source} changed during compilation; retry')
                for (_, temporary, image, stamp, _), record in zip(pending, records):
                    # Image first: interruption can cost a compile, not a false
                    # cache hit. Per-image publication remains atomic, not a
                    # multi-file transaction on disk or across running loaders.
                    os.replace(temporary, image)
                    temporary.write_text(json.dumps(record))
                    os.replace(temporary, stamp)
        finally:
            for _, temporary, _, _, _ in pending:
                temporary.unlink(missing_ok=True)
        compiled = len(pending)
        print(f'angelX: native modules: {compiled} compiled, {len(MODULES) - compiled} unchanged in {time.perf_counter() - started:.3f}s ({jobs} jobs max); {out}', file=sys.stderr)


def main():
    if len(sys.argv) != 2:
        raise ValueError('usage: build-native-modules.py REPOSITORY')
    if os.environ.get('ANGEL_NO_BUILD') == '1':
        raise ValueError('ANGEL_NO_BUILD=1 forbids builds')
    if sys.platform not in ('linux', 'darwin'):
        raise ValueError('native modules require Linux or macOS')
    build(Path(sys.argv[1]).resolve())


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'angelX: native module build failed: {error}', file=sys.stderr)
        sys.exit(1)
