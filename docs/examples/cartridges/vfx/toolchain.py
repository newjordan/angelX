#!/usr/bin/env python3
"""Fetch a checksum-pinned Bend toolchain into this cartridge, never shell rc files."""
import hashlib
import io
import os
from pathlib import Path
import platform
import shutil
import tarfile
import tempfile
import urllib.request

VERSION = "2.0.39"
HASHES = {
    ("Linux", "x86_64"): ("linux-x64", "6cf462fd8f70b094a4950ebb709a9c4ffd8f1d1189d9cedf1fd63cd5d041c7ab"),
    ("Linux", "aarch64"): ("linux-arm64", "71b43e1bb9313dd14ac2b0553b854e41c39d65ecaa47040796ebff4a46f54a97"),
    ("Darwin", "arm64"): ("darwin-arm64", "d4aaef68de545727429ce83b9e88ff3273bf068df599e3542e446755b5f3d83a"),
    ("Darwin", "x86_64"): ("darwin-x64", "a512cfd1d29ec24c640126a6a640e597b4ea1abe0aefbda7cc57efd85ab56e22"),
}


def main():
    root = Path(__file__).resolve().parent / ".toolchain"
    dest = root / f"bend-{VERSION}"
    binary = dest / "bend" / "bin" / "bend"
    if binary.is_file():
        print(binary)
        return
    key = (platform.system(), platform.machine())
    if key not in HASHES:
        raise SystemExit(f"No pinned Bend release for {key}; set BEND to a Bend {VERSION} binary")
    target, digest = HASHES[key]
    url = f"https://github.com/bendlang/bend/releases/download/v{VERSION}/bend-{VERSION}-{target}.tar.gz"
    data = urllib.request.urlopen(url, timeout=90).read()
    if hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit("Bend download failed SHA-256 verification")
    root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="download-", dir=root) as scratch:
        temp = Path(scratch)
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            for member in archive.getmembers():
                path = (temp / member.name).resolve()
                if not path.is_relative_to(temp.resolve()) or member.issym() or member.islnk():
                    raise SystemExit("Unexpected path or link in Bend archive")
            archive.extractall(temp, filter="data")
        # Publish a complete directory in one rename.
        staging = temp / "package"
        staging.mkdir()
        shutil.move(str(temp / "bend"), str(staging / "bend"))
        try:
            os.rename(staging, dest)
        except FileExistsError:
            if not binary.is_file():
                raise
    print(binary)


if __name__ == "__main__":
    main()
