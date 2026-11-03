"""Motion library: procedural curves and keyframe clips. No mocap data, no models.

Every motion is f(t, h) -> pose, t in [0,1), h = sprite height in px.
Pose keys: bone angles in radians (image coords, y down: positive = clockwise on screen),
'root' rotates the whole body about the hip, 'root_off' = (dx, dy) px.
Limb keys use the image side: *_r = image-right limb, *_l = image-left limb. Motions are
authored for a character facing image-right ("forward" = +x); rig.make_pose_mapper mirrors
them for left-facing sprites.
Conventions for a limb hanging down: negative angle swings it forward (+x), positive back.
Lower leg positive = knee bends (foot goes back). Lower arm negative = elbow bends forward.
"""
import numpy as np

TAU = 2 * np.pi
D = np.radians


def ease(x):
    x = np.clip(x, 0, 1)
    return x * x * (3 - 2 * x)


# ---------------------------------------------------------------- keyframe engine
def clip(keys, loop=False):
    """keys: [(t, {bone: degrees, 'dx': frac_of_h, 'dy': frac_of_h}), ...] sorted by t.
    Missing bones are 0. Loops use Catmull-Rom (smooth through keys); one-shots use
    smoothstep between keys and hold the last key."""
    ts = np.array([k[0] for k in keys], float)
    names = sorted({n for _, k in keys for n in k})
    V = np.array([[k.get(n, 0.0) for n in names] for _, k in keys], float)

    def f(t, h):
        if loop:
            n = len(ts)
            t = t % 1.0
            i = np.searchsorted(ts, t, side="right") - 1
            i = int(np.clip(i, 0, n - 1))
            j = (i + 1) % n
            t0, t1 = ts[i], ts[j] if j > i else ts[j] + 1.0
            u = (t - t0) / max(t1 - t0, 1e-9)
            p0, p1, p2, p3 = V[(i - 1) % n], V[i], V[j], V[(j + 1) % n]
            v = 0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u * u
                       + (-p0 + 3 * p1 - 3 * p2 + p3) * u ** 3)
        else:
            if t <= ts[0]:
                v = V[0]
            elif t >= ts[-1]:
                v = V[-1]
            else:
                i = np.searchsorted(ts, t, side="right") - 1
                u = ease((t - ts[i]) / (ts[i + 1] - ts[i]))
                v = V[i] * (1 - u) + V[i + 1] * u
        pose = {}
        dx = dy = 0.0
        for n, x in zip(names, v):
            if n == "dx":
                dx = x * h
            elif n == "dy":
                dy = x * h
            else:
                pose[n] = D(x)
        pose["root_off"] = (dx, dy)
        return pose
    f.loop = loop
    return f


# ---------------------------------------------------------------- procedural loops
def idle(t, h):
    s = np.sin(TAU * t)
    return {"root_off": (0, 0.012 * h * (1 - np.cos(TAU * t)) / 2), "torso": D(1.5) * s,
            "head": D(3) * np.sin(TAU * t + 0.6),
            "upper_arm_l": D(4) * s, "upper_arm_r": -D(4) * s,
            "lower_arm_l": D(4) * s, "lower_arm_r": -D(6) * s,
            "upper_leg_l": -D(1.5) * s, "upper_leg_r": -D(1.5) * s}


def _gait(t, h, swing, knee, lean, bob, elbow, arm_k):
    s, c = np.sin(TAU * t), np.cos(TAU * t)
    ll, lr = -swing * s, swing * s
    kl = knee * max(0, np.sin(TAU * t + np.pi / 2))
    kr = knee * max(0, np.sin(TAU * t - np.pi / 2))
    return {"root_off": (0, -bob * h * abs(c)), "torso": lean,
            "head": -D(2) * np.sin(2 * TAU * t) - lean * 0.4,
            "upper_leg_l": ll, "upper_leg_r": lr, "lower_leg_l": kl, "lower_leg_r": kr,
            "upper_arm_l": -ll * arm_k, "upper_arm_r": -lr * arm_k,
            "lower_arm_l": -elbow, "lower_arm_r": -elbow}


def walk(t, h):
    return _gait(t, h, D(28), D(35), D(4), 0.025, D(15), 0.8)


def run(t, h):
    return _gait(t, h, D(42), D(75), D(12), 0.06, D(75), 1.1)


def dance(t, h):
    s = np.sin(TAU * t); s2 = np.sin(2 * TAU * t)
    return {"root_off": (0.03 * h * s, -0.03 * h * abs(s2)), "root": D(4) * s,
            "torso": -D(8) * s, "head": D(10) * s2,
            "upper_arm_l": D(150) * ease(0.5 + 0.5 * s) + D(10), "lower_arm_l": D(40) * s2,
            "upper_arm_r": -D(150) * ease(0.5 - 0.5 * s) - D(10), "lower_arm_r": -D(40) * s2,
            "upper_leg_l": -D(10) * max(0, s2), "lower_leg_l": D(25) * max(0, s2),
            "upper_leg_r": -D(10) * max(0, -s2), "lower_leg_r": D(25) * max(0, -s2)}


def wave(t, h):
    s = np.sin(2 * TAU * t)
    return {**idle(t, h), "upper_arm_r": -D(150), "lower_arm_r": D(25) * s - D(10),
            "head": D(5) * s * 0.5}


# ---------------------------------------------------------------- keyframe clips
def _attack(side):
    o, s = ("r", "l") if side == "r" else ("l", "r")
    return clip([
        (0.0, {}),
        (0.35, {f"upper_arm_{o}": -150, f"lower_arm_{o}": -40, "torso": -8, "head": 4,
                f"upper_arm_{s}": -25, f"lower_arm_{s}": -30}),
        (0.55, {f"upper_arm_{o}": 50, f"lower_arm_{o}": 0, "torso": 14, "head": -7, "dx": 0.04,
                f"upper_arm_{s}": -35, f"lower_arm_{s}": -30, "upper_leg_r": -18,
                "upper_leg_l": 12, "lower_leg_r": 10}),
        (0.7, {f"upper_arm_{o}": 40, "torso": 12, "head": -6, "dx": 0.035,
               f"upper_arm_{s}": -30, f"lower_arm_{s}": -30, "upper_leg_r": -16,
               "upper_leg_l": 10, "lower_leg_r": 9}),
        (1.0, {}),
    ])


attack_right = _attack("r")
attack_left = _attack("l")

jump = clip([
    (0.0, {}),
    (0.18, {"dy": 0.09, "upper_leg_l": -45, "upper_leg_r": -45, "lower_leg_l": 85,
            "lower_leg_r": 85, "torso": 18, "head": -10, "upper_arm_l": 35, "upper_arm_r": 35}),
    (0.32, {"dy": -0.08, "upper_leg_l": 8, "upper_leg_r": 8, "lower_leg_l": 5, "lower_leg_r": 5,
            "torso": -4, "upper_arm_l": -150, "upper_arm_r": -150}),
    (0.52, {"dy": -0.32, "upper_leg_l": -50, "upper_leg_r": -40, "lower_leg_l": 90,
            "lower_leg_r": 80, "torso": 4, "upper_arm_l": -130, "upper_arm_r": -130,
            "lower_arm_l": -30, "lower_arm_r": -30}),
    (0.74, {"dy": -0.02, "upper_leg_l": -10, "upper_leg_r": -10, "lower_leg_l": 15,
            "lower_leg_r": 15, "upper_arm_l": -60, "upper_arm_r": -60}),
    (0.86, {"dy": 0.08, "upper_leg_l": -40, "upper_leg_r": -40, "lower_leg_l": 75,
            "lower_leg_r": 75, "torso": 14, "head": -8, "upper_arm_l": 20, "upper_arm_r": 20}),
    (1.0, {}),
])

cast = clip([
    (0.0, {}),
    (0.3, {"upper_arm_l": -100, "upper_arm_r": -100, "lower_arm_l": -40, "lower_arm_r": -40,
           "torso": -6, "head": -6, "dy": 0.01}),
    (0.5, {"upper_arm_l": -170, "upper_arm_r": -165, "lower_arm_l": 0, "lower_arm_r": 0,
           "torso": -10, "head": -12, "dy": -0.03}),
    (0.62, {"upper_arm_l": -85, "upper_arm_r": -80, "lower_arm_l": 0, "lower_arm_r": 0,
            "torso": 8, "head": 4, "dx": 0.03}),
    (0.8, {"upper_arm_l": -80, "upper_arm_r": -75, "lower_arm_l": -5, "lower_arm_r": -5,
           "torso": 6, "head": 3, "dx": 0.025}),
    (1.0, {}),
])

hurt = clip([
    (0.0, {}),
    (0.15, {"torso": -16, "head": -22, "dx": -0.05, "upper_arm_l": 40, "upper_arm_r": 30,
            "lower_arm_l": -30, "lower_arm_r": -30, "upper_leg_r": -12, "lower_leg_r": 10}),
    (0.4, {"torso": -10, "head": 8, "dx": -0.04, "upper_arm_l": 20, "upper_arm_r": 15,
           "lower_arm_l": -40, "lower_arm_r": -40, "upper_leg_r": -8, "lower_leg_r": 15}),
    (1.0, {}),
])

death = clip([
    (0.0, {}),
    (0.15, {"torso": -14, "head": -20, "dx": -0.03, "upper_arm_l": 35, "upper_arm_r": 30}),
    (0.35, {"torso": 10, "head": 15, "dy": 0.08, "upper_leg_l": -30, "upper_leg_r": -20,
            "lower_leg_l": 60, "lower_leg_r": 50, "upper_arm_l": 15, "upper_arm_r": 10}),
    (0.65, {"root": -55, "torso": -5, "head": -10, "dy": 0.22, "dx": -0.12,
            "upper_leg_l": -25, "upper_leg_r": -35, "lower_leg_l": 30, "lower_leg_r": 40,
            "upper_arm_l": 70, "upper_arm_r": 50}),
    (0.85, {"root": -88, "head": -8, "dy": 0.37, "dx": -0.2, "upper_leg_l": -8,
            "upper_leg_r": -14, "lower_leg_l": 10, "lower_leg_r": 15, "upper_arm_l": 95,
            "upper_arm_r": 80, "lower_arm_l": -10}),
    (1.0, {"root": -90, "head": -4, "dy": 0.38, "dx": -0.2, "upper_leg_l": -6,
           "upper_leg_r": -10, "lower_leg_l": 6, "lower_leg_r": 10, "upper_arm_l": 92,
           "upper_arm_r": 78, "lower_arm_l": -6}),
])

celebrate = clip([
    (0.0, {"upper_arm_l": 150, "upper_arm_r": -150, "lower_arm_l": 10, "lower_arm_r": -10,
           "head": -6}),
    (0.25, {"dy": 0.05, "upper_leg_l": -25, "upper_leg_r": -25, "lower_leg_l": 50,
            "lower_leg_r": 50, "upper_arm_l": 130, "upper_arm_r": -130, "lower_arm_l": 40,
            "lower_arm_r": -40, "torso": 6}),
    (0.5, {"dy": -0.16, "upper_leg_l": -15, "upper_leg_r": -5, "lower_leg_l": 35,
           "lower_leg_r": 20, "upper_arm_l": 165, "upper_arm_r": -165, "head": -12}),
    (0.75, {"dy": 0.04, "upper_leg_l": -22, "upper_leg_r": -22, "lower_leg_l": 45,
            "lower_leg_r": 45, "upper_arm_l": 135, "upper_arm_r": -135, "lower_arm_l": 30,
            "lower_arm_r": -30, "torso": 5}),
], loop=True)

crouch = clip([
    (0.0, {}),
    (0.35, {"dy": 0.18, "upper_leg_l": -60, "upper_leg_r": -45, "lower_leg_l": 110,
            "lower_leg_r": 95, "torso": 22, "head": -16, "upper_arm_l": -30, "upper_arm_r": -40,
            "lower_arm_l": -40, "lower_arm_r": -50}),
    (0.7, {"dy": 0.185, "upper_leg_l": -61, "upper_leg_r": -46, "lower_leg_l": 112,
           "lower_leg_r": 97, "torso": 20, "head": -14, "upper_arm_l": -28, "upper_arm_r": -38,
           "lower_arm_l": -42, "lower_arm_r": -52}),
    (1.0, {"dy": 0.18, "upper_leg_l": -60, "upper_leg_r": -45, "lower_leg_l": 110,
           "lower_leg_r": 95, "torso": 22, "head": -16, "upper_arm_l": -30, "upper_arm_r": -40,
           "lower_arm_l": -40, "lower_arm_r": -50}),
])

block = clip([
    (0.0, {}),
    (0.3, {"upper_arm_l": -80, "lower_arm_l": -70, "upper_arm_r": -40, "lower_arm_r": -50,
           "torso": -6, "head": 6, "dx": -0.02, "upper_leg_r": -14, "upper_leg_l": 10}),
    (0.75, {"upper_arm_l": -84, "lower_arm_l": -72, "upper_arm_r": -42, "lower_arm_r": -52,
            "torso": -8, "head": 7, "dx": -0.025, "upper_leg_r": -14, "upper_leg_l": 10}),
    (1.0, {}),
])

kick = clip([
    (0.0, {}),
    (0.3, {"upper_leg_r": -70, "lower_leg_r": 90, "torso": -8, "head": 4, "upper_arm_l": -30,
           "upper_arm_r": 25, "dy": -0.01}),
    (0.48, {"upper_leg_r": -95, "lower_leg_r": 0, "torso": -16, "head": 8, "upper_arm_l": -45,
            "upper_arm_r": 35, "upper_leg_l": 6, "dy": -0.02}),
    (0.66, {"upper_leg_r": -60, "lower_leg_r": 60, "torso": -8, "upper_arm_l": -25,
            "upper_arm_r": 20}),
    (1.0, {}),
])

for _f in (idle, walk, run, dance, wave):
    _f.loop = True

# name -> (fn, default frames, fps)
ANIMS = {
    "idle": (idle, 12, 10), "walk": (walk, 12, 12), "run": (run, 10, 14),
    "jump": (jump, 14, 14), "attack_right": (attack_right, 10, 14),
    "attack_left": (attack_left, 10, 14), "cast": (cast, 14, 12), "hurt": (hurt, 8, 12),
    "death": (death, 16, 12), "celebrate": (celebrate, 12, 12), "dance": (dance, 16, 12),
    "crouch": (crouch, 10, 12), "block": (block, 10, 12), "wave": (wave, 12, 12),
    "kick": (kick, 10, 14),
}
ANIMS["attack"] = ANIMS["attack_right"]  # v1 name
DEFAULT_SET = "idle,walk,run,jump,attack_right,attack_left,cast,hurt,death,celebrate,dance,crouch,block,wave,kick"
