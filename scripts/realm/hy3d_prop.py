"""A generated GLB (Hunyuan 3D) -> an angelX world3d prop OBJ.

    uv run --with numpy,pillow,pygltflib,fast-simplification,scipy \
        scripts/realm/hy3d_prop.py IN.glb OUT.obj --height H --faces N --mats stone,wood,...

Applies the node transform, welds the vertices split at texture seams and
decimates to N faces. Turns glTF Y-up into world3d Z-up (tile units, base on
z = 0, centred on the origin, x mirrored to match the raster's screen-right).
Each original face's base-colour texel is auto-levelled and snapped to the
nearest allowed material; each decimated face takes the material covering the
most area among the original faces nearest to it. Only bright, saturated warm
texels may become ember or fire. Writes the OBJ subset props.rs reads:
v / usemtl / f. Per-prop settings are in cockpit/assets/realm/props/README.md.
"""
import argparse, io, json, struct, sys
import numpy as np
from PIL import Image
import pygltflib, fast_simplification

# Representative albedo for matching (close to raster.rs material_base, with
# emissives at the colour they read as in a texture).
MATS = {
    "stone": (116, 122, 138), "stone_dark": (68, 74, 90), "roof": (76, 68, 84),
    "wood": (96, 70, 48), "trunk": (62, 48, 36), "door": (124, 86, 48),
    "ember": (235, 110, 40), "fire": (250, 200, 90), "grass": (68, 94, 58),
    "foliage": (40, 62, 44), "banner": (146, 62, 58), "brass": (190, 145, 75),
    "parchment": (226, 203, 139), "crystal": (100, 180, 210), "rock": (65, 81, 105),
    "armor": (157, 174, 195), "cloth_gold": (211, 165, 69), "cloth_blue": (66, 142, 212),
    "leather": (91, 43, 26), "water": (26, 42, 66), "path": (130, 120, 98),
}

def srgb_to_lab(rgb):
    c = np.asarray(rgb, dtype=np.float64) / 255.0
    c = np.where(c > 0.04045, ((c + 0.055) / 1.055) ** 2.4, c / 12.92)
    m = np.array([[0.4124, 0.3576, 0.1805], [0.2126, 0.7152, 0.0722], [0.0193, 0.1192, 0.9505]])
    xyz = c @ m.T / np.array([0.9505, 1.0, 1.089])
    f = np.where(xyz > 0.008856, np.cbrt(xyz), 7.787 * xyz + 16 / 116)
    return np.stack([116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]), 200 * (f[..., 1] - f[..., 2])], -1)

def load_glb(path):
    g = pygltflib.GLTF2().load(path)
    blob = g.binary_blob()
    def acc(i):
        a = g.accessors[i]; bv = g.bufferViews[a.bufferView]
        comp = {5126: np.float32, 5125: np.uint32, 5123: np.uint16, 5121: np.uint8}[a.componentType]
        n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[a.type]
        start = (bv.byteOffset or 0) + (a.byteOffset or 0)
        stride = bv.byteStride or n * np.dtype(comp).itemsize
        arr = np.lib.stride_tricks.as_strided(np.frombuffer(blob[start:], dtype=comp), shape=(a.count, n), strides=(stride, np.dtype(comp).itemsize))
        return np.array(arr)
    def node_matrix(mesh_index):
        # The node that places this mesh (rotation, scale, translation).
        for n in g.nodes:
            if n.mesh == mesh_index:
                if n.matrix:
                    return np.array(n.matrix, dtype=np.float64).reshape(4, 4).T
                x, y, z, w = n.rotation or [0, 0, 0, 1]
                r = np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                              [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                              [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])
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
            verts.append(v); faces.append(f + base); uvs.append(uv); base += len(v)
            if tex is None and p.material is not None:
                m = g.materials[p.material]
                pbr = m.pbrMetallicRoughness
                if pbr and pbr.baseColorTexture is not None:
                    img = g.images[g.textures[pbr.baseColorTexture.index].source]
                    bv = g.bufferViews[img.bufferView]
                    data = blob[(bv.byteOffset or 0):(bv.byteOffset or 0) + bv.byteLength]
                    tex = np.asarray(Image.open(io.BytesIO(data)).convert("RGB"))
    return np.concatenate(verts), np.concatenate(faces), np.concatenate(uvs), tex

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("inp"); ap.add_argument("out")
    ap.add_argument("--height", type=float, required=True)
    ap.add_argument("--faces", type=int, default=1800)
    ap.add_argument("--mats", required=True)
    ap.add_argument("--yaw", type=float, default=0.0, help="degrees about z after import")
    ap.add_argument("--emissive-min", type=float, default=0.55, help="HSV value floor for ember/fire")
    ap.add_argument("--max-gain", type=float, default=2.5, help="ceiling on the auto-level lift")
    ap.add_argument("--light", default=None, help="material for the brightest share of the surface")
    ap.add_argument("--light-share", type=float, default=0.3, help="area share given to --light")
    a = ap.parse_args()
    v, f, uv, tex = load_glb(a.inp)
    # Face colours on the full mesh, then carry them through decimation by
    # sampling again from the nearest original face centroid.
    def face_rgb(fv, ff, fuv):
        if tex is None:
            return np.full((len(ff), 3), 120.0)
        c = fuv[ff].mean(1)
        h, w = tex.shape[:2]
        x = np.clip((c[:, 0] % 1.0) * (w - 1), 0, w - 1).astype(int)
        y = np.clip((c[:, 1] % 1.0) * (h - 1), 0, h - 1).astype(int)
        # average a 3x3 patch to tame texture noise
        acc = np.zeros((len(ff), 3))
        for dx in (-2, 0, 2):
            for dy in (-2, 0, 2):
                acc += tex[np.clip(y + dy, 0, h - 1), np.clip(x + dx, 0, w - 1)]
        return acc / 9.0
    # Weld vertices split at texture seams, or the simplifier keeps every seam.
    _, first, inv = np.unique(np.round(v, 5), axis=0, return_index=True, return_inverse=True)
    vw = v[first]
    fw = inv.reshape(-1)[f]
    keep = (fw[:, 0] != fw[:, 1]) & (fw[:, 1] != fw[:, 2]) & (fw[:, 0] != fw[:, 2])
    target = max(0.0, 1.0 - a.faces / keep.sum())
    v2, f2 = fast_simplification.simplify(vw.astype(np.float32), fw[keep].astype(np.int32), target_reduction=target, agg=7)
    # Classify every original face, then give each decimated face the material
    # that covers the most area among the original faces nearest to it.
    allowed = [m.strip() for m in a.mats.split(",")]
    lab_allowed = srgb_to_lab(np.array([MATS[m] for m in allowed]))
    raw = face_rgb(v, f, uv)
    tri0 = v[f]
    area0 = 0.5 * np.linalg.norm(np.cross(tri0[:, 1] - tri0[:, 0], tri0[:, 2] - tri0[:, 0]), axis=1)
    # Auto-level: PBR albedo is darker than the lit look the model is meant to
    # have. Lift the area-weighted 95th percentile of brightness to ~190 so a
    # light bone or stone reads light; hue and relative values are kept.
    lum = raw.mean(1)
    by_lum = np.argsort(lum)
    cum = np.cumsum(area0[by_lum]) / area0.sum()
    p95 = lum[by_lum][min(int(np.searchsorted(cum, 0.95)), len(lum) - 1)]
    gain = float(np.clip(190.0 / max(p95, 1.0), 1.0, a.max_gain))
    rgb = np.clip(raw * gain, 0, 255)
    lab = srgb_to_lab(rgb)
    d = ((lab[:, None, :] - lab_allowed[None, :, :]) ** 2).sum(-1)
    # Glow is judged on the texture as painted, never on the lifted colour.
    hsv_v = raw.max(1) / 255.0
    sat = (raw.max(1) - raw.min(1)) / np.maximum(raw.max(1), 1)
    for j, m in enumerate(allowed):
        if m in ("ember", "fire"):
            # only genuinely bright, saturated warm texels may glow
            warm = (raw[:, 0] > raw[:, 2] * 1.6)
            ok = (hsv_v >= a.emissive_min) & (sat > 0.45) & warm
            d[~ok, j] = 1e9
    opick = d.argmin(1)
    if a.light:
        # The brightest share of the surface by area takes the light material
        # (bone, pale stone), whatever its absolute colour came out as.
        cut = lum[by_lum][min(int(np.searchsorted(cum, 1.0 - a.light_share)), len(lum) - 1)]
        glow = np.isin(opick, [allowed.index(m) for m in ("ember", "fire") if m in allowed])
        opick = np.where((lum >= cut) & ~glow, allowed.index(a.light), opick)
    tri = v[f]
    area = 0.5 * np.linalg.norm(np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0]), axis=1)
    from scipy.spatial import cKDTree  # noqa
    tree = cKDTree(tri.mean(1))
    _, idx = tree.query(v2[f2].mean(1), k=min(24, len(f)))
    votes = np.zeros((len(f2), len(allowed)))
    np.add.at(votes, (np.repeat(np.arange(len(f2)), idx.shape[1]), opick[idx].ravel()), area[idx].ravel())
    pick = votes.argmax(1)
    # Orientation: glTF Y-up -> world3d Z-up; scale to height; base at z=0.
    # x is negated: the raster's screen-right is (-sin, cos), a mirror of glTF's.
    p = np.stack([-v2[:, 0], -v2[:, 2], v2[:, 1]], 1).astype(np.float64)
    yaw = np.radians(a.yaw)
    c, s = np.cos(yaw), np.sin(yaw)
    p[:, :2] = p[:, :2] @ np.array([[c, s], [-s, c]])
    p -= [(p[:, 0].max() + p[:, 0].min()) / 2, (p[:, 1].max() + p[:, 1].min()) / 2, p[:, 2].min()]
    p *= a.height / max(p[:, 2].max(), 1e-6)
    # Drop slivers the decimator left: no area, no facing to shade.
    e1 = p[f2[:, 1]] - p[f2[:, 0]]; e2 = p[f2[:, 2]] - p[f2[:, 0]]
    solid = np.linalg.norm(np.cross(e1, e2), axis=1) > 1e-6
    f2, pick = f2[solid], pick[solid]
    order = np.argsort(pick, kind="stable")
    lines = [f"# angelX world3d prop: {len(p)} vertices, {len(f2)} faces, height {a.height} tiles"]
    lines += [f"v {x:.3f} {y:.3f} {z:.3f}" for x, y, z in p]
    cur = None
    for i in order:
        if pick[i] != cur:
            cur = pick[i]; lines.append(f"usemtl {allowed[cur]}")
        a1, b1, c1 = f2[i] + 1
        lines.append(f"f {a1} {b1} {c1}")
    open(a.out, "w").write("\n".join(lines) + "\n")
    counts = {allowed[k]: int((pick == k).sum()) for k in range(len(allowed))}
    print(json.dumps({"gain": round(gain, 2), "in_faces": int(len(f)), "out_faces": int(len(f2)), "materials": counts, "size_kb": round(len("\n".join(lines)) / 1024, 1)}))

if __name__ == "__main__":
    main()
