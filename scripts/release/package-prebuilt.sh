#!/usr/bin/env bash
# package-prebuilt.sh — wrap a source release and one target's binaries into
# the package that install.sh downloads.
#
#   scripts/release/package-prebuilt.sh MANIFEST BIN_DIR OUT_DIR
#
# MANIFEST is the source release manifest (`npm run release:evidence`), with
# its archive beside it. BIN_DIR is build-prebuilt.sh's output for one target.
# OUT_DIR receives angelX-<target>.tar.gz, a copy of install.sh, and a
# SHA256SUMS covering every package in OUT_DIR. Upload all of them to the
# GitHub release: install.sh fetches them by these fixed names.
#
# A package is the source release unpacked, plus the binaries in
# cockpit/target/release/ and PREBUILT.json at the top. bin/angelX sees
# PREBUILT.json and runs the shipped binaries instead of building them.
set -euo pipefail

usage() { echo "usage: package-prebuilt.sh MANIFEST BIN_DIR OUT_DIR" >&2; exit 2; }
[ $# -eq 3 ] || usage
manifest="$1"
bin_dir="$2"
out="$3"
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
fail() { echo "package-prebuilt: $*" >&2; exit 1; }

[ -f "$manifest" ] || fail "no manifest at $manifest"
[ -f "$bin_dir/build.json" ] || fail "no build.json in $bin_dir"

# Every identity the package claims is checked here, before anything is written.
ids=$(
  python3 - "$manifest" "$bin_dir" <<'PY'
import hashlib, json, os, sys
manifest_path, bin_dir = sys.argv[1], sys.argv[2]
def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()
def fail(message):
    sys.exit(f"package-prebuilt: {message}")
def sidecar(path):
    want = f"{sha(path)}  {os.path.basename(path)}\n"
    if open(path + ".sha256").read() != want:
        fail(f"checksum sidecar differs for {os.path.basename(path)}")
m = json.load(open(manifest_path))
b = json.load(open(os.path.join(bin_dir, "build.json")))
archive = os.path.join(os.path.dirname(manifest_path), m["artifact"]["name"])
sidecar(manifest_path)
sidecar(archive)
if sha(archive) != m["artifact"]["sha256"]:
    fail("source archive differs from its manifest")
if m["source"]["release_inputs_dirty"]:
    fail("source release was cut from dirty inputs")
for key, want in (("version", m["version"]), ("commit", m["source"]["commit"]),
                  ("cockpit_source_sha256", m["source"]["cockpit_source_sha256"]),
                  ("resources_sha256", m["entries_manifest_sha256"])):
    if b.get(key) != want:
        fail(f"build.json {key} is {b.get(key)}, the manifest says {want}")
for name, want in b["binaries"].items():
    if sha(os.path.join(bin_dir, name)) != want:
        fail(f"{name} differs from build.json")
print(m["version"], b["target"], b["runs_on"].replace(" ", "_"), m["source"]["commit"],
      archive, m["artifact"]["sha256"], m["source"]["cockpit_source_sha256"],
      m["entries_manifest_sha256"])
PY
)
read -r version target runs_on commit archive archive_sha source_sha resource_sha <<< "$ids"
runs_on="${runs_on//_/ }"

mkdir -p "$out"
stage=$(mktemp -d "$out/.stage.XXXXXX")
trap 'rm -rf "$stage"' EXIT
top="angelX-$version-$target"
mkdir "$stage/$top"
tar -xf "$archive" -C "$stage/$top"
[ -f "$stage/$top/install.sh" ] || fail "the source release has no install.sh"
mkdir -p "$stage/$top/cockpit/target/release"
for name in angel angel-sandbox; do
  install -m 0755 "$bin_dir/$name" "$stage/$top/cockpit/target/release/$name"
done

python3 - "$stage/$top/PREBUILT.json" "$manifest" "$bin_dir/build.json" <<'PY'
import json, os, sys
out, manifest_path, build_path = sys.argv[1:]
m = json.load(open(manifest_path))
b = json.load(open(build_path))
prebuilt = {
    "schema": "angelX-prebuilt/v1",
    "version": m["version"],
    "target": b["target"],
    "runs_on": b["runs_on"],
    "commit": m["source"]["commit"],
    "cockpit_source_sha256": m["source"]["cockpit_source_sha256"],
    "resources_sha256": m["entries_manifest_sha256"],
    "source_release": {
        "archive": m["artifact"]["name"],
        "archive_sha256": m["artifact"]["sha256"],
        "manifest_sha256": m["manifest_sha256"],
    },
    "rustc": b["rustc"],
    "features": b["features"],
    "binaries": {f"cockpit/target/release/{k}": v for k, v in b["binaries"].items()},
}
with open(out, "w") as f:
    json.dump(prebuilt, f, indent=2)
    f.write("\n")
PY

# Same bytes for the same inputs: sorted names, no owners, the commit's time.
mtime=$(git -C "$root" show -s --format=%ct "$commit" 2>/dev/null || echo 0)
package="$out/angelX-$target.tar.gz"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$mtime" \
  -C "$stage" -cf - "$top" | gzip -n -9 > "$package.tmp"
mv -f "$package.tmp" "$package"
cp -f "$stage/$top/install.sh" "$out/install.sh"

(cd "$out" && sha256sum angelX-*.tar.gz install.sh > SHA256SUMS.tmp && mv -f SHA256SUMS.tmp SHA256SUMS)
echo "package-prebuilt: $(basename "$package") — angelX $version for $runs_on, $(du -h "$package" | cut -f1)"
