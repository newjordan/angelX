#!/usr/bin/env python3
"""Install or verify/reuse checkout-local pinned Blaze; never change global Cargo."""
import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile

REVISION = "b383b0bcf5da33816809b00a37e1c36a0e761efb"
REPOSITORY = "https://github.com/indexable-inc/blaze.git"
TOOLCHAIN = "nightly-2026-10-27"
IMAGES = ("blaze", "cargo-blaze", "ra-jit", "ra-jit-rustc")
ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "cockpit/target/tools"
SOURCE = TOOLS / "blaze-src"
DESTINATION = TOOLS / "blaze"


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def digest(path):
    with path.open("rb") as image:
        return hashlib.file_digest(image, "sha256").hexdigest()


def present(path):
    # A dangling symlink is not an absent installation eligible for replacement.
    return path.exists() or path.is_symlink()


def validate_install(version):
    """Check bounded receipt and every executable, not just a selector or hash list.

    This detects accidental corruption of a local installation. The receipt is
    not a signature or an attestation against a malicious local writer.
    """
    try:
        if not stat.S_ISDIR(version.lstat().st_mode):
            raise ValueError("installation must be a regular directory, not a symlink")
        path = version / "install.json"
        if not stat.S_ISREG(path.lstat().st_mode):
            raise ValueError("receipt must be a regular file, not a symlink")
        with path.open("rb") as source:
            raw = source.read(16385)
        if len(raw) > 16384:
            raise ValueError("receipt exceeds 16 KiB")
        receipt = json.loads(raw)
        if not isinstance(receipt, dict):
            raise ValueError("receipt must be an object")
        for key, expected in (("repository", REPOSITORY), ("revision", REVISION), ("toolchain", TOOLCHAIN)):
            if receipt.get(key) != expected:
                raise ValueError(f"receipt {key} does not match pinned {expected}")
        hashes = receipt.get("sha256")
        if not isinstance(hashes, dict) or set(hashes) != set(IMAGES):
            raise ValueError("receipt must name exactly the four compiler images")
        for name in IMAGES:
            path = version / name
            mode = path.lstat().st_mode
            if not stat.S_ISREG(mode):
                raise ValueError(f"{name} must be a regular file, not a symlink")
            if not os.access(path, os.X_OK):
                raise ValueError(f"{name} is not executable")
            expected = hashes[name]
            if not isinstance(expected, str) or len(expected) != 64 or any(
                char not in "0123456789abcdef" for char in expected
            ):
                raise ValueError(f"{name} receipt SHA-256 is invalid")
            if digest(path) != expected:
                raise ValueError(f"{name} SHA-256 hash mismatch")
        return receipt
    except (OSError, ValueError) as error:
        raise SystemExit(f"Invalid Blaze installation/receipt at {version}: {error}; "
                         "refusing to replace a live compiler or change selection") from error


def check_source():
    actual = run("git", "-C", str(SOURCE), "rev-parse", "HEAD", capture_output=True).stdout.strip()
    if actual != REVISION:
        raise SystemExit(f"Blaze checkout is {actual}, expected {REVISION}; refusing to overwrite it")
    dirty = run("git", "-C", str(SOURCE), "status", "--porcelain", "--untracked-files=all",
                "--ignore-submodules=none", capture_output=True).stdout
    if dirty:
        raise SystemExit("Blaze source has local edits; refusing to install an unpinned compiler")
    toolchain = (SOURCE / "harness/toolchain").read_text().strip()
    if toolchain != TOOLCHAIN:
        raise SystemExit(f"Blaze source toolchain is {toolchain}, expected pinned {TOOLCHAIN}")


def local_toolchain():
    """Probe locally with auto-install disabled; provisioning is a separate action."""
    env = dict(os.environ, RUSTUP_AUTO_INSTALL="0")
    try:
        cargo, rustc = (Path(run("rustup", "which", "--toolchain", TOOLCHAIN, name,
                                 capture_output=True, env=env).stdout.strip())
                        for name in ("cargo", "rustc"))
        if not all(path.is_file() and os.access(path, os.X_OK) for path in (cargo, rustc)):
            return None
        sysroot = Path(run(str(rustc), "--print", "sysroot", capture_output=True, env=env).stdout.strip())
        host = run(str(rustc), "--print", "host-tuple", capture_output=True, env=env).stdout.strip()
        if not host or any(char not in "abcdefghijklmnopqrstuvwxyz0123456789_-" for char in host):
            return None
        # Check rustup's installed-file inventories, not just one sentinel per
        # component. Missing alloc/host libraries must not pass as a complete setup.
        rustlib = sysroot / "lib/rustlib"
        components = (rustlib / "components").read_text().splitlines()
        for component in ("rust-src", f"rust-std-{host}", "rust-std-wasm32-unknown-unknown"):
            if component not in components:
                return None
            with (rustlib / f"manifest-{component}").open("rb") as source:
                raw = source.read(2 * 1024 * 1024 + 1)
            if not raw or len(raw) > 2 * 1024 * 1024:
                return None
            for entry in raw.decode("utf-8").splitlines():
                kind, separator, name = entry.partition(":")
                relative = Path(name)
                if not separator or not name or relative.is_absolute() or ".." in relative.parts:
                    return None
                path = sysroot / relative
                if kind == "file" and path.is_file():
                    continue
                if kind == "dir" and path.is_dir():
                    continue
                return None
        library = sysroot / "lib/rustlib/src/rust/library"
        wasm = sysroot / "lib/rustlib/wasm32-unknown-unknown/lib"
        if not all((library / name / "src/lib.rs").is_file() and
                   any(path.is_file() for path in wasm.glob(f"lib{name}-*.rlib"))
                   for name in ("core", "alloc", "std")):
            return None
        return str(cargo), str(rustc)
    except (OSError, UnicodeError, subprocess.CalledProcessError):
        return None


def ensure_toolchain(offline):
    found = local_toolchain()
    if found:
        return found
    if offline:
        raise SystemExit(f"Offline setup requires installed toolchain {TOOLCHAIN}, rust-src and "
                         "wasm32-unknown-unknown; rerun without --offline to provision them")
    run("rustup", "toolchain", "install", TOOLCHAIN, "--profile", "minimal", "--component", "rust-src",
        "--target", "wasm32-unknown-unknown")
    found = local_toolchain()
    if not found:
        raise SystemExit(f"Toolchain {TOOLCHAIN} is incomplete after provisioning; selection unchanged")
    return found


def build_settings(cargo, rustc, offline):
    env = dict(os.environ, RUSTUP_TOOLCHAIN=TOOLCHAIN, RUSTC=rustc, RUSTUP_AUTO_INSTALL="0")
    # Never inherit the host application's target directory or compiler wrapper.
    for name in ("CARGO_TARGET_DIR", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"):
        env.pop(name, None)
    # Explicit overrides beat Cargo's config-file and CARGO_BUILD_* forms.
    # An empty encoded flag list beats target-specific configured rustflags.
    env.update(CARGO_BUILD_TARGET_DIR=str(SOURCE / "target"),
               CARGO_BUILD_RUSTC_WRAPPER="", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER="",
               CARGO_ENCODED_RUSTFLAGS="")
    host = run(rustc, "--print", "host-tuple", capture_output=True, env=env).stdout.strip()
    if not host or any(char not in "abcdefghijklmnopqrstuvwxyz0123456789_-" for char in host):
        raise SystemExit("Invalid host target from pinned Rust compiler")
    command = [cargo, "build", "--locked", "--target-dir", str(SOURCE / "target"),
               "--target", host, "--config", 'build.rustc-wrapper=""',
               "--config", 'build.rustc-workspace-wrapper=""',
               *(["--offline"] if offline else []),
               "-p", "ra-jit", "--release", "--no-default-features", "--bins"]
    return command, env, SOURCE / "target" / host / "release"


def select():
    # A valid already-selected generation needs no inode/mtime churn.
    selected = DESTINATION / "selected"
    if selected.is_file() and not selected.is_symlink():
        with selected.open("rb") as source:
            if source.read(128) == (REVISION + "\n").encode():
                return
    # The launcher reads this regular-file selector, not a symlink.
    with tempfile.NamedTemporaryFile(mode="w", prefix="selected-", dir=DESTINATION, delete=False) as output:
        selector = Path(output.name)
        output.write(REVISION + "\n")
    try:
        os.replace(selector, selected)
    finally:
        selector.unlink(missing_ok=True)


@contextmanager
def install_lock():
    """One cooperating installer at a time, including validation and publication."""
    lock = TOOLS / ".blaze-install.lock"
    try:
        lock.mkdir()
    except FileExistsError as error:
        raise SystemExit(f"Blaze installer already running (lock {lock}); if interrupted, "
                         "confirm no installer is running before removing the lock") from error
    try:
        yield
    finally:
        lock.rmdir()


def install(offline):
    version = DESTINATION / REVISION
    installed = validate_install(version) if present(version) else None
    if SOURCE.exists():
        # Reuse must not hide an existing locally modified/mismatched checkout.
        check_source()
    elif installed is None:
        if offline:
            raise SystemExit("offline setup requires a pinned Blaze source checkout or verified installation")
        run("git", "init", str(SOURCE))
        run("git", "-C", str(SOURCE), "remote", "add", "origin", REPOSITORY)
        run("git", "-C", str(SOURCE), "fetch", "--depth=1", "origin", REVISION)
        run("git", "-C", str(SOURCE), "checkout", "--detach", "FETCH_HEAD")
        check_source()
    cargo, rustc = ensure_toolchain(offline)
    if installed is not None:
        select()
        print(f"Blaze {REVISION[:12]} verified and reused at {version}")
    else:
        command, env, artifacts = build_settings(cargo, rustc, offline)
        run(*command, cwd=SOURCE, env=env)
        # Keep frontend, daemon and rustc wrapper together; publish only complete generations.
        DESTINATION.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="install-", dir=DESTINATION) as scratch:
            stage = Path(scratch)
            hashes = {}
            for name in IMAGES:
                shutil.copy2(artifacts / name, stage / name)
                hashes[name] = digest(stage / name)
            receipt = {"repository": REPOSITORY, "revision": REVISION, "toolchain": TOOLCHAIN, "sha256": hashes}
            (stage / "install.json").write_text(json.dumps(receipt, indent=2) + "\n")
            validate_install(stage)
            if not present(version):
                os.rename(stage, version)
            elif validate_install(version) != receipt:
                raise SystemExit("Existing Blaze installation differs; refusing to replace a live compiler")
            select()
        print(f"Blaze {REVISION[:12]} installed at {version}")
    print("Experimental opt-in: ANGEL_BUILD_BACKEND=blaze angelX. Auto retains qualified Cargo.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true",
                        help="never fetch/provision; any necessary Cargo build uses --offline")
    args = parser.parse_args()
    TOOLS.mkdir(parents=True, exist_ok=True)
    with install_lock():
        install(args.offline)


if __name__ == "__main__":
    main()
