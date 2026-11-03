"""Generated item GLBs (TRELLIS.2) -> the Delve's floor-pickup mesh pack.

    uv run --no-project --with numpy,pillow,pygltflib,fast-simplification,scipy \
        scripts/delve/item_meshes.py GLB_DIR OUT.mesh [--faces N] [--preview DIR] [--only a,b]

Every NAME.glb in GLB_DIR becomes one mesh named NAME. Each is welded at its
texture seams and decimated to about N faces (thin arms keep a few more), then
coloured per vertex from the base-colour texture: each original face's texel,
auto-levelled (PBR albedo is darker than the painted look it came from), and
area-weighted onto the nearest decimated vertices. Normals are split at
creases sharper than 45 degrees, so a blade keeps its edge and a coin its rim.

glTF is Y-up with +Z towards the viewer, as the Delve's stage is (`x` across,
`y` up, `z` down the room): no turn. Each mesh is centred on its box and
scaled so its longest side is 1; the game sizes it.

The pack (`crates/delve/src/render/overworld/arena/item_pack.rs` reads it),
little-endian:

    "AIM1", count u32
    each mesh: name length u8, name bytes, vertices u32, triangles u32,
               each vertex: position i16 x3 (/32767), normal i8 x3 (/127),
                            pad u8, colour u8 x4 (straight RGBA)
               each triangle: u16 x3, counter-clockwise from the front
"""
import argparse, io, json, os, struct, sys
import numpy as np
from PIL import Image, ImageDraw
import pygltflib, fast_simplification
from scipy.spatial import cKDTree

# Arms are long and thin: their faces go further along them, and they are
# stood up along their length (`orient`).
THIN = {"bow", "sword", "axe", "staff", "spear", "hammer", "dagger", "crossbow", "handgonne", "mace", "key"}

# Flat things, turned to face the camera (`orient`).
FLAT = {"shield", "gear", "scale"}

# After `orient`, the arms whose head came out at the bottom, turned over
# (judged on the previews; the head is the business end: tip, blade, bit).
FLIP = {"sword", "spear", "hammer", "mace"}


def load_glb(path):
    g = pygltflib.GLTF2().load(path)
    blob = g.binary_blob()

    def acc(i):
        a = g.accessors[i]
        bv = g.bufferViews[a.bufferView]
        comp = {5126: np.float32, 5125: np.uint32, 5123: np.uint16, 5121: np.uint8}[a.componentType]
        n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[a.type]
        start = (bv.byteOffset or 0) + (a.byteOffset or 0)
        stride = bv.byteStride or n * np.dtype(comp).itemsize
        arr = np.lib.stride_tricks.as_strided(
            np.frombuffer(blob[start:], dtype=comp), shape=(a.count, n), strides=(stride, np.dtype(comp).itemsize)
        )
        return np.array(arr)

    def node_matrix(mesh_index):
        for n in g.nodes:
            if n.mesh == mesh_index:
                if n.matrix:
                    return np.array(n.matrix, dtype=np.float64).reshape(4, 4).T
                x, y, z, w = n.rotation or [0, 0, 0, 1]
                r = np.array([
                    [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                    [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                    [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
                ])
                m = np.eye(4)
                m[:3, :3] = r * np.array(n.scale or [1, 1, 1])
                m[:3, 3] = n.translation or [0, 0, 0]
                return m
        return np.eye(4)

    verts, faces, uvs, tex = [], [], [], None
    base = 0
    for mi, mesh in enumerate(g.meshes):
        m = node_matrix(mi)
        for p in mesh.primitives:
            v = acc(p.attributes.POSITION).astype(np.float64)
            v = v @ m[:3, :3].T + m[:3, 3]
            f = acc(p.indices).reshape(-1, 3) if p.indices is not None else np.arange(len(v)).reshape(-1, 3)
            uv = acc(p.attributes.TEXCOORD_0) if p.attributes.TEXCOORD_0 is not None else np.zeros((len(v), 2), np.float32)
            verts.append(v)
            faces.append(f + base)
            uvs.append(uv)
            base += len(v)
            if tex is None and p.material is not None:
                pbr = g.materials[p.material].pbrMetallicRoughness
                if pbr and pbr.baseColorTexture is not None:
                    t = g.textures[pbr.baseColorTexture.index]
                    src = t.source if t.source is not None else (t.extensions or {}).get("EXT_texture_webp", {}).get("source")
                    img = g.images[src]
                    bv = g.bufferViews[img.bufferView]
                    data = blob[(bv.byteOffset or 0):(bv.byteOffset or 0) + bv.byteLength]
                    tex = np.asarray(Image.open(io.BytesIO(data)).convert("RGB")).astype(np.float64)
    return np.concatenate(verts), np.concatenate(faces).astype(np.int64), np.concatenate(uvs), tex


def face_colours(v, f, uv, tex):
    """Each face's base colour: a 3x3 texel patch at its middle's uv."""
    if tex is None:
        return np.full((len(f), 3), 150.0)
    c = uv[f].mean(1)
    h, w = tex.shape[:2]
    x = np.clip((c[:, 0] % 1.0) * (w - 1), 0, w - 1).astype(int)
    y = np.clip((c[:, 1] % 1.0) * (h - 1), 0, h - 1).astype(int)
    acc = np.zeros((len(f), 3))
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            acc += tex[np.clip(y + dy, 0, h - 1), np.clip(x + dx, 0, w - 1)]
    return acc / 9.0


def tri_normals(p, f):
    n = np.cross(p[f[:, 1]] - p[f[:, 0]], p[f[:, 2]] - p[f[:, 0]])
    area = np.linalg.norm(n, axis=1)
    return n / np.maximum(area[:, None], 1e-12), area * 0.5


def crease_split(p, f, colour, crease_deg=45.0):
    """Vertices split where faces meet at more than `crease_deg`; each corner
    takes the area-weighted normal of the faces on its side of the crease."""
    fn, area = tri_normals(p, f)
    cos = np.cos(np.radians(crease_deg))
    incident = [[] for _ in range(len(p))]
    for fi, tri in enumerate(f):
        for vi in tri:
            incident[vi].append(fi)
    out_p, out_n, out_c, key_of = [], [], [], {}
    out_f = np.zeros_like(f)
    for fi, tri in enumerate(f):
        for k, vi in enumerate(tri):
            near = [g for g in incident[vi] if fn[g] @ fn[fi] >= cos]
            n = (fn[near] * area[near, None]).sum(0)
            n = n / max(np.linalg.norm(n), 1e-12)
            key = (int(vi), *np.round(n * 20).astype(int))
            idx = key_of.get(key)
            if idx is None:
                idx = len(out_p)
                key_of[key] = idx
                out_p.append(p[vi])
                out_n.append(n)
                out_c.append(colour[vi])
            out_f[fi, k] = idx
    return np.array(out_p), np.array(out_n), np.array(out_c), out_f


def orient(v, f, name):
    """An arm stood up: its longest extent along y, its thinnest along z
    (its broad side to the camera). Something flat (a shield, a cog) turned
    to face the camera, its bulge (a boss) towards it, keeping the side
    nearest up as up. Anything else as it was made."""
    c = v[f].mean(1)
    _, area = tri_normals(v, f)
    mid = (c * area[:, None]).sum(0) / max(area.sum(), 1e-12)
    d = c - mid
    cov = (d * area[:, None]).T @ d
    val, vec = np.linalg.eigh(cov)  # ascending: thinnest first
    if name in THIN:
        r = np.stack([vec[:, 1], vec[:, 2], vec[:, 0]])  # rows: new x, y, z
    elif name in FLAT:
        # Flat: the thinnest axis to z; of the other two, the one nearer the
        # model's own up is y.
        a, b = vec[:, 1], vec[:, 2]
        up = a if abs(a[1]) > abs(b[1]) else b
        up = up if up[1] >= 0 else -up
        z = vec[:, 0]
        x = np.cross(up, z)
        r = np.stack([x, up, z])
    else:
        return v
    if np.linalg.det(r) < 0:
        r[0] = -r[0]
    out = (v - mid) @ r.T
    if name in FLAT:
        # The bulge (a boss, a rim) faces the camera.
        if (out[f].mean(1)[:, 2] * area).sum() < 0:
            out[:, 0] = -out[:, 0]
            out[:, 2] = -out[:, 2]
    if name in FLIP:
        out[:, :2] = -out[:, :2]
    return out


def convert(path, faces, name=""):
    v, f, uv, tex = load_glb(path)
    v = orient(v, f, name)
    raw = face_colours(v, f, uv, tex)
    _, area0 = tri_normals(v, f)
    # Auto-level: lift the area-weighted 95th percentile of brightness to ~205.
    lum = raw.mean(1)
    order = np.argsort(lum)
    cum = np.cumsum(area0[order]) / max(area0.sum(), 1e-12)
    p95 = lum[order][min(int(np.searchsorted(cum, 0.95)), len(lum) - 1)]
    gain = float(np.clip(205.0 / max(p95, 1.0), 1.0, 1.7))
    raw = np.clip(raw * gain, 0, 255)
    # Weld the seams, or the simplifier keeps every one of them.
    _, first, inv = np.unique(np.round(v, 5), axis=0, return_index=True, return_inverse=True)
    vw, fw = v[first], inv.reshape(-1)[f]
    keep = (fw[:, 0] != fw[:, 1]) & (fw[:, 1] != fw[:, 2]) & (fw[:, 0] != fw[:, 2])
    fw = fw[keep]
    target = max(0.0, 1.0 - faces / max(len(fw), 1))
    v2, f2 = fast_simplification.simplify(vw.astype(np.float32), fw.astype(np.int32), target_reduction=target, agg=7)
    v2 = v2.astype(np.float64)
    f2 = f2.astype(np.int64)
    # Drop slivers: no area, no facing to shade.
    _, a2 = tri_normals(v2, f2)
    f2 = f2[a2 > 1e-10]
    # A vertex's colour: the original faces nearest it, by area.
    tree = cKDTree(v[f].mean(1))
    k = min(12, len(f))
    d, idx = tree.query(v2, k=k)
    w = area0[idx] / np.maximum(d, 1e-6)
    colour = (raw[idx] * w[..., None]).sum(1) / np.maximum(w.sum(1, keepdims=True), 1e-12)
    # Centre on the box; the longest side is 1.
    lo, hi = v2.min(0), v2.max(0)
    v2 = (v2 - (lo + hi) / 2) / max((hi - lo).max(), 1e-9)
    p, n, c, ff = crease_split(v2, f2, colour)
    return p, n, c, ff, {"gain": round(gain, 2), "in_faces": int(len(f)), "faces": int(len(ff)), "vertices": int(len(p))}


def pack(meshes):
    out = bytearray(b"AIM1")
    out += struct.pack("<I", len(meshes))
    for name, (p, n, c, f) in meshes:
        assert len(p) < 65536, name
        nb = name.encode()
        out += struct.pack("<B", len(nb)) + nb
        out += struct.pack("<II", len(p), len(f))
        q = np.clip(np.round(p * 2 * 32767), -32767, 32767).astype("<i2")  # +-0.5 -> full range
        nq = np.clip(np.round(n * 127), -127, 127).astype("i1")
        cq = np.clip(np.round(c), 0, 255).astype("u1")
        for i in range(len(p)):
            out += q[i].tobytes() + nq[i].tobytes() + b"\0" + bytes(cq[i]) + b"\xff"
        out += f.astype("<u2").tobytes()
    return bytes(out)


def preview(p, n, c, f, size=256, yaw=0.5, pitch=0.35):
    """A quick look: lit by one light from the upper left, painter's order."""
    cy, sy, cp, sp = np.cos(yaw), np.sin(yaw), np.cos(pitch), np.sin(pitch)
    r = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]]) @ np.eye(3)
    r = np.array([[1, 0, 0], [0, cp, -sp], [0, sp, cp]]) @ r
    q = p @ r.T
    nn = n @ r.T
    light = np.array([-0.4, 0.6, 0.7])
    light /= np.linalg.norm(light)
    im = Image.new("RGB", (size, size), (18, 17, 24))
    dr = ImageDraw.Draw(im)
    order = np.argsort(q[f].mean(1)[:, 2])
    for fi in order:
        tri = f[fi]
        shade = 0.35 + 0.65 * max(0.0, float(nn[tri].mean(0) @ light))
        col = tuple(int(min(255, x * shade)) for x in c[tri].mean(0))
        pts = [((q[i, 0] * 0.9 + 0.5) * size, (0.5 - q[i, 1] * 0.9) * size) for i in tri]
        dr.polygon(pts, fill=col)
    return im


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("glb_dir")
    ap.add_argument("out")
    ap.add_argument("--faces", type=int, default=600)
    ap.add_argument("--preview", default=None)
    ap.add_argument("--only", default=None)
    a = ap.parse_args()
    names = sorted(x[:-4] for x in os.listdir(a.glb_dir) if x.endswith(".glb"))
    if a.only:
        names = [x for x in names if x in a.only.split(",")]
    meshes, report = [], {}
    for name in names:
        faces = int(a.faces * 1.4) if name in THIN else a.faces
        p, n, c, f, info = convert(os.path.join(a.glb_dir, name + ".glb"), faces, name)
        meshes.append((name, (p, n, c, f)))
        report[name] = info
        if a.preview:
            os.makedirs(a.preview, exist_ok=True)
            row = Image.new("RGB", (768, 256))
            for i, yaw in enumerate((0.0, 0.6, 1.57)):
                row.paste(preview(p, n, c, f, yaw=yaw), (256 * i, 0))
            row.save(os.path.join(a.preview, name + ".png"))
    data = pack(meshes)
    open(a.out, "wb").write(data)
    print(json.dumps({"bytes": len(data), "meshes": report}, indent=1))


if __name__ == "__main__":
    sys.exit(main())
