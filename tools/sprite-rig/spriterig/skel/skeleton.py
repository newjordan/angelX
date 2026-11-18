import numpy as np

P = "mixamorig:"
_SPEC = [
    ("Hips", None, (0, 0.98, 0)),
    ("Spine", "Hips", (0, 0.10, 0.0)),
    ("Spine1", "Spine", (0, 0.12, 0)),
    ("Spine2", "Spine1", (0, 0.12, 0)),
    ("Neck", "Spine2", (0, 0.14, 0)),
    ("Head", "Neck", (0, 0.09, 0.01)),
    ("HeadTop_End", "Head", (0, 0.19, 0)),
]
for s, x in (("Left", 1), ("Right", -1)):
    _SPEC += [
        (s + "Shoulder", "Spine2", (0.06 * x, 0.10, 0)),
        (s + "Arm", s + "Shoulder", (0.12 * x, -0.02, 0)),
        (s + "ForeArm", s + "Arm", (0, -0.27, 0)),
        (s + "Hand", s + "ForeArm", (0, -0.25, 0)),
        (s + "HandIndex1", s + "Hand", (0, -0.09, 0.01)),
        (s + "UpLeg", "Hips", (0.09 * x, -0.06, 0)),
        (s + "Leg", s + "UpLeg", (0, -0.42, 0)),
        (s + "Foot", s + "Leg", (0, -0.40, -0.02)),
        (s + "ToeBase", s + "Foot", (0, -0.07, 0.12)),
        (s + "Toe_End", s + "ToeBase", (0, 0, 0.07)),
    ]

NAMES = [n for n, _, _ in _SPEC]
INDEX = {n: i for i, n in enumerate(NAMES)}
PARENT = np.array([INDEX[p] if p else -1 for _, p, _ in _SPEC])
OFFSET = np.array([o for _, _, o in _SPEC], float)
CHILD = {}
for i, p in enumerate(PARENT):
    if p >= 0:
        CHILD.setdefault(int(p), i)
CHILD[INDEX["Hips"]] = INDEX["Spine"]
CHILD[INDEX["Spine2"]] = INDEX["Neck"]


def bare(name):
    return name.split(":")[-1]


def rx(a):
    c, s = np.cos(a), np.sin(a)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])


def ry(a):
    c, s = np.cos(a), np.sin(a)
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])


def rz(a):
    c, s = np.cos(a), np.sin(a)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def euler(order, degs):
    m = np.eye(3)
    for ax, d in zip(order.upper(), degs):
        m = m @ {"X": rx, "Y": ry, "Z": rz}[ax](np.radians(d))
    return m


def fk(local, hips=(0, 0, 0), offset=OFFSET):
    B = len(NAMES)
    R = np.empty((B, 3, 3))
    X = np.empty((B, 3))
    for i in range(B):
        p = PARENT[i]
        if p < 0:
            R[i] = local[i]
            X[i] = offset[i] + np.asarray(hips, float)
        else:
            R[i] = R[p] @ local[i]
            X[i] = X[p] + R[p] @ offset[i]
    return R, X


def rest():
    return fk(np.tile(np.eye(3), (len(NAMES), 1, 1)))


def height(offset=OFFSET):
    _, X = fk(np.tile(np.eye(3), (len(NAMES), 1, 1)), offset=offset)
    return X[:, 1].max() - X[:, 1].min()
