"""Minimal FBX *binary* reader (pure python + numpy + zlib), enough for Mixamo:
the node tree, Model (LimbNode) rest transforms, AnimationCurveNode/AnimationCurve
keys, the connections between them, and (for skinned files) Geometry vertices,
polygon indices and Cluster skin weights. Format notes follow the public
Blender `io_scene_fbx/parse_fbx.py` layout description; no code copied."""
import struct, zlib
import numpy as np

_ARR = {b"f": ("<f", 4), b"d": ("<d", 8), b"l": ("<q", 8), b"i": ("<i", 4), b"b": ("<?", 1)}


class Node:
    __slots__ = ("name", "props", "children")

    def __init__(self, name, props, children):
        self.name, self.props, self.children = name, props, children

    def find(self, name):
        return next((c for c in self.children if c.name == name), None)

    def all(self, name):
        return [c for c in self.children if c.name == name]

    def __repr__(self):
        return f"Node({self.name}, {self.props[:3]}, {len(self.children)} kids)"


def _prop(b, o):
    t = b[o:o + 1]; o += 1
    if t == b"Y": return struct.unpack_from("<h", b, o)[0], o + 2
    if t == b"C": return bool(b[o]), o + 1
    if t == b"I": return struct.unpack_from("<i", b, o)[0], o + 4
    if t == b"F": return struct.unpack_from("<f", b, o)[0], o + 4
    if t == b"D": return struct.unpack_from("<d", b, o)[0], o + 8
    if t == b"L": return struct.unpack_from("<q", b, o)[0], o + 8
    if t in (b"S", b"R"):
        n = struct.unpack_from("<I", b, o)[0]; o += 4
        v = b[o:o + n]
        return (v.decode("utf8", "replace") if t == b"S" else v), o + n
    if t in _ARR:
        n, enc, clen = struct.unpack_from("<III", b, o); o += 12
        raw = b[o:o + clen]; o += clen
        if enc == 1:
            raw = zlib.decompress(raw)
        fmt, sz = _ARR[t]
        return np.frombuffer(raw, fmt[1:] if fmt[1] != "?" else np.bool_, n).astype(
            {b"f": np.float32, b"d": np.float64, b"l": np.int64, b"i": np.int32, b"b": bool}[t]), o
    raise ValueError(f"FBX: unknown property type {t!r} at {o}")


def _node(b, o, v64):
    if v64:
        end, nprop, plen = struct.unpack_from("<QQQ", b, o); o += 24
    else:
        end, nprop, plen = struct.unpack_from("<III", b, o); o += 12
    nlen = b[o]; o += 1
    if end == 0:
        return None, o + nlen
    name = b[o:o + nlen].decode("ascii", "replace"); o += nlen
    props = []
    for _ in range(nprop):
        v, o = _prop(b, o)
        props.append(v)
    kids = []
    sentinel = 25 if v64 else 13
    while end - o > sentinel:
        c, o = _node(b, o, v64)
        if c is None:
            break
        kids.append(c)
    return Node(name, props, kids), end


def parse(path):
    b = open(path, "rb").read()
    if not b.startswith(b"Kaydara FBX Binary"):
        raise ValueError(f"{path}: not a binary FBX (export 'FBX Binary' from Mixamo)")
    ver = struct.unpack_from("<I", b, 23)[0]
    v64 = ver >= 7500
    o, top = 27, []
    while o < len(b):
        n, o = _node(b, o, v64)
        if n is None:
            break
        top.append(n)
    return Node("root", [ver], top)


def props70(n):
    out = {}
    p = n.find("Properties70")
    for c in (p.children if p else []):
        out[c.props[0]] = c.props[4:] if len(c.props) > 4 else []
    return out


def euler_fbx(deg, order=0):
    """FBX eEulerXYZ (order 0): rotate about X, then Y, then Z -> M = Rz Ry Rx."""
    from .skeleton import rx, ry, rz
    a = np.radians(deg)
    R = {"X": rx(a[0]), "Y": ry(a[1]), "Z": rz(a[2])}
    seq = ["XYZ", "XZY", "YZX", "YXZ", "ZXY", "ZYX"][order]
    m = np.eye(3)
    for ax in seq:
        m = R[ax] @ m
    return m


TICKS = 46186158000.0


class Scene:
    def __init__(self, path):
        self.root = parse(path)
        objs = self.root.find("Objects")
        self.objs = {c.props[0]: c for c in objs.children}
        self.parent, self.links = {}, {}
        for c in self.root.find("Connections").children:
            kind, src, dst = c.props[0], c.props[1], c.props[2]
            prop = c.props[3] if len(c.props) > 3 else None
            self.links.setdefault(dst, []).append((src, prop))
            if kind == "OO" and src in self.objs and self.objs[src].name == "Model" and \
                    dst in self.objs and self.objs[dst].name == "Model":
                self.parent[src] = dst
        gs = props70(self.root.find("GlobalSettings")) if self.root.find("GlobalSettings") else {}
        self.unit = float(gs.get("UnitScaleFactor", [1.0])[0]) / 100.0  # -> metres

    def name(self, oid):
        return self.objs[oid].props[1].split("\x00")[0].split("::")[-1]

    def bones(self):
        ids = [k for k, n in self.objs.items() if n.name == "Model" and n.props[2] in ("LimbNode", "Root", "Null")]
        order, seen = [], set()

        def visit(i):
            if i in seen:
                return
            p = self.parent.get(i)
            if p in ids:
                visit(p)
            seen.add(i); order.append(i)
        for i in ids:
            visit(i)
        return order

    def rest(self, oid):
        p = props70(self.objs[oid])
        t = np.array(p.get("Lcl Translation", [0, 0, 0]), float) * self.unit
        ro = int(p.get("RotationOrder", [0])[0]) if p.get("RotationOrder") else 0
        pre = euler_fbx(np.array(p.get("PreRotation", [0, 0, 0]), float))
        post = euler_fbx(np.array(p.get("PostRotation", [0, 0, 0]), float))
        lcl = np.array(p.get("Lcl Rotation", [0, 0, 0]), float)
        return t, pre, post, lcl, ro

    def curves(self, oid):
        """{'Lcl Rotation': {'X': (times_s, values)}, 'Lcl Translation': {...}}"""
        out = {}
        for src, prop in self.links.get(oid, []):
            n = self.objs.get(src)
            if n is None or n.name != "AnimationCurveNode" or prop not in ("Lcl Rotation", "Lcl Translation"):
                continue
            ax = {}
            for c, cp in self.links.get(src, []):
                cn = self.objs.get(c)
                if cn is not None and cn.name == "AnimationCurve":
                    kt = cn.find("KeyTime").props[0] / TICKS
                    kv = cn.find("KeyValueFloat").props[0].astype(float)
                    ax[cp.split("|")[-1]] = (kt, kv)
            out[prop] = ax
        return out


def _at(tr, t, default):
    if tr is None:
        return default
    kt, kv = tr
    return float(np.interp(t, kt, kv))


def load_clip(path, name=None, fps=30):
    from . import clips as C
    import os
    sc = Scene(path)
    ids = sc.bones()
    pos = {i: k for k, i in enumerate(ids)}
    names = [sc.name(i) for i in ids]
    par = [pos.get(sc.parent.get(i), -1) for i in ids]
    rest = [sc.rest(i) for i in ids]
    cur = [sc.curves(i) for i in ids]
    tmax = max([kt[-1] for c in cur for ax in c.values() for kt, _ in ax.values()] + [0])
    tmin = min([kt[0] for c in cur for ax in c.values() for kt, _ in ax.values()] + [0])
    T = max(int(round((tmax - tmin) * fps)) + 1, 1)

    def local_r(k, t):
        tr, pre, post, lcl, ro = rest[k]
        rc = cur[k].get("Lcl Rotation", {})
        e = np.array([_at(rc.get(a), t, lcl[j]) for j, a in enumerate("XYZ")])
        return pre @ euler_fbx(e, ro) @ post.T

    restR = [local_r(k, -1e9) if not cur[k] else pre @ euler_fbx(lcl, ro) @ post.T
             for k, (tr, pre, post, lcl, ro) in enumerate(rest)]
    off = np.array([r[0] for r in rest])
    world, hp = [], []
    hk = next((k for k, n in enumerate(names) if n.split(":")[-1] == "Hips"), 0)
    for f in range(T):
        t = tmin + f / fps
        G = [None] * len(ids)
        for k in range(len(ids)):
            r = local_r(k, t)
            G[k] = r if par[k] < 0 else G[par[k]] @ r
        tc = cur[hk].get("Lcl Translation", {})
        hp.append(np.array([_at(tc.get(a), t, off[hk][j] / sc.unit) for j, a in enumerate("XYZ")]) * sc.unit)
        world.append(G)
    # Hips may sit under an armature node: carry the parent's world rotation into its rest
    return C.retarget(names, par, off, restR, world, hp, fps,
                      name or os.path.splitext(os.path.basename(path))[0], "fbx:" + os.path.basename(path))
