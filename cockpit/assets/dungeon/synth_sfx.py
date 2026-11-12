#!/usr/bin/env python3
"""Synthesize the Delve's spectacle sound effects, offline and from nothing.

No API, no sound pack: every effect here is built from noise bursts, filtered
sweeps, decaying sine modes and envelopes with numpy and scipy, then encoded
with ffmpeg to the format the other effects use (mono, 44.1 kHz, 96 kbit/s
mp3) into sfx/<name>.mp3.

  python3 synth_sfx.py              make every effect
  python3 synth_sfx.py knight_hit   make just these
  python3 synth_sfx.py --stats      print each file's length, peak and RMS

Levels follow sfx/README.md: the body of an effect sits at RMS -20 dB with
peaks limited to -2 dBFS, as `level.py` leaves the rest. Footsteps, which
come many a second, sit 6 dB under that so a walking party stays a texture
and not a drumbeat. Run it with numpy and scipy installed (a venv is fine).
Output is deterministic: every noise source is seeded.
"""
import pathlib, subprocess, sys, tempfile, wave
import numpy as np
from scipy import signal

SR = 44100
HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "sfx"
PEAK = 0.72  # a hair under -2 dBFS, so the mp3 encode never overshoots it
BODY_DB = -20.0
STEP_DB = -26.0
# A hi-hat rides on each note of the Bard's that lands, a few a second: it
# sits under a single effect's body.
HAT_DB = -26.0
# The blades, the magic and the wishes sit by loudness, not body RMS: a
# whoosh is all treble and a boom all bass, so the same RMS would play the
# one far louder than the other. Each is set (dB off `BODY_DB`) so that,
# looped and measured with ffmpeg's ebur128, a swing sits near the old
# `swing` (-21 LUFS) and under its contact, the contacts climb light to
# heavy (-20.5, -18.5, -17.5 LUFS), a cast and its break sit with the
# shots (-19 to -20), and a wish landing rings out a little grander at
# each tier (-17, -16.5, -16).
LEVEL = {
    "swing_spear": -4.9, "swing_sword": -4.1, "swing_axe": -3.3, "swing_cleaver": -2.0, "swing_maul": 0.7,
    "hit_flesh_light": -0.5, "hit_flesh_medium": 2.5, "hit_flesh_heavy": 3.5,
    "gore_behead": 2.6, "gore_head_thud": 0.6, "gore_bisect": 1.5, "gore_slump": 1.3,
    "parry": -1.4, "parry_whizz": -1.8,
    "cast_fire": 0.7, "impact_fire": 0.6, "cast_frost": -3.2, "impact_frost": -2.6,
    "cast_holy": 0.5, "impact_holy": -1.2, "cast_storm": 1.0, "impact_storm": 2.7,
    "cast_poison": 0.6, "impact_poison": 0.4, "cast_arcane": 0.8, "impact_arcane": -1.4,
    "cast_earth": 1.1, "impact_earth": 1.4,
    "lamp_rub": -0.5, "genie_appear": 1.7, "wish_land": 2.9, "wish_land_grand": 3.7, "wish_land_mythic": 4.1,
    "fairy_wish": 0.0,
    # The steps up (`rules::rewards`): each a short fanfare of its own, a
    # little under a wish landing, so a step up never drowns the fight.
    "level_up": 2.5, "rung_up": 1.0, "relic_find": 2.5, "deepest_yet": 2.5, "sky_lit": 1.5,
}


# ─── primitives ──────────────────────────────────────────────────────────────

def rng_for(name):
    return np.random.default_rng(sum(ord(c) * (i + 7) for i, c in enumerate(name)))


class Bench:
    """One effect's workspace: a seeded noise source and the shared tools."""

    def __init__(self, name):
        self.r = rng_for(name)

    def noise(self, dur):
        return self.r.standard_normal(int(SR * dur))


def t_of(dur):
    return np.arange(int(SR * dur)) / SR


def exp_env(dur, tau, attack=0.002):
    t = t_of(dur)
    e = np.exp(-t / tau)
    a = int(SR * attack)
    if a > 0:
        e[:a] *= np.linspace(0, 1, a)
    return e


def shape(n, points):
    """A piecewise-linear envelope through (time fraction, level) points."""
    xs, ys = zip(*points)
    return np.interp(np.linspace(0, 1, n), xs, ys)


def band(x, lo, hi, order=3):
    hi = min(hi, SR / 2 - 100)
    sos = signal.butter(order, [lo, hi], btype="band", fs=SR, output="sos")
    return signal.sosfilt(sos, x)


def low(x, hz, order=3):
    return signal.sosfilt(signal.butter(order, hz, btype="low", fs=SR, output="sos"), x)


def high(x, hz, order=3):
    return signal.sosfilt(signal.butter(order, hz, btype="high", fs=SR, output="sos"), x)


def swept_band(x, f0, f1, width=0.5, steps=24):
    """A band-pass whose centre glides from f0 to f1 over the signal."""
    n = len(x)
    out = np.zeros(n)
    edges = np.linspace(0, n, steps + 1).astype(int)
    for i in range(steps):
        k = (i + 0.5) / steps
        c = f0 * (f1 / f0) ** k
        seg = band(x, c * (1 - width / 2), c * (1 + width / 2), 2)
        out[edges[i]:edges[i + 1]] = seg[edges[i]:edges[i + 1]]
    return out


def sweep(f0, f1, dur, expo=True):
    t = t_of(dur)
    k = t / dur
    f = f0 * (f1 / f0) ** k if expo else f0 + (f1 - f0) * k
    return np.sin(2 * np.pi * np.cumsum(f) / SR)


def modes(freqs, decays, dur, amps=None):
    """A struck thing: decaying sine modes (metal, bone, glass, bells)."""
    t = t_of(dur)
    amps = amps or [1.0] * len(freqs)
    out = np.zeros_like(t)
    for f, d, a in zip(freqs, decays, amps):
        out += a * np.sin(2 * np.pi * f * t) * np.exp(-t * d)
    n = int(SR * 0.001)
    out[:n] *= np.linspace(0, 1, n)
    return out


def thud(f0, f1, dur, tau):
    return sweep(f0, f1, dur) * exp_env(dur, tau, 0.001)


def place(canvas, x, at, gain=1.0):
    i = int(SR * at)
    if i >= len(canvas):
        return
    n = min(len(x), len(canvas) - i)
    canvas[i:i + n] += gain * x[:n]


def canvas_of(dur):
    return np.zeros(int(SR * dur))


def drive(x, amount):
    return np.tanh(x * amount) / np.tanh(amount)


def tail(x, rt, wet=0.3, b=None):
    """A short room: a decaying noise impulse response."""
    r = b.r if b else np.random.default_rng(3)
    n = int(SR * rt)
    ir = r.standard_normal(n) * np.exp(-np.arange(n) / (SR * rt / 5))
    ir = low(ir, 3500)
    wetted = signal.fftconvolve(x, ir)[: len(x)]
    wetted *= np.max(np.abs(x)) / (np.max(np.abs(wetted)) + 1e-9)
    return x + wet * wetted


def fade_out(x, dur=0.03):
    n = min(len(x), int(SR * dur))
    x[-n:] *= np.linspace(1, 0, n)
    return x


# ─── the effects ─────────────────────────────────────────────────────────────

def knight_hit(b):
    d = 0.4
    x = canvas_of(d)
    place(x, low(b.noise(0.1), 1800) * exp_env(0.1, 0.02), 0, 0.8)
    place(x, thud(130, 62, 0.25, 0.07), 0, 1.0)
    place(x, modes([880, 1430, 2260, 3100], [38, 45, 60, 80], 0.3, [0.5, 0.35, 0.25, 0.15]), 0.004, 0.45)
    return fade_out(x)


def knight_hit_big(b):
    d = 0.7
    x = canvas_of(d)
    place(x, band(b.noise(0.2), 250, 3500) * exp_env(0.2, 0.05), 0, 0.9)
    place(x, thud(95, 38, 0.5, 0.16), 0, 1.2)
    place(x, modes([620, 980, 1530, 2400, 3300], [20, 24, 30, 42, 60], 0.6, [0.6, 0.5, 0.35, 0.25, 0.15]), 0.003, 0.6)
    place(x, modes([420, 700], [30, 36], 0.4), 0.09, 0.25)
    return tail(fade_out(x), 0.2, 0.18, b)


def hit_blade(b):
    d = 0.3
    x = canvas_of(d)
    place(x, high(b.noise(0.08), 1400) * exp_env(0.08, 0.015), 0, 0.8)
    place(x, modes([2200, 3400, 5100], [55, 70, 90], 0.25), 0.0, 0.5)
    place(x, thud(210, 110, 0.12, 0.03), 0.0, 0.7)
    return fade_out(x)


def hit_axe(b):
    d = 0.38
    x = canvas_of(d)
    place(x, band(b.noise(0.14), 350, 2600) * exp_env(0.14, 0.04), 0, 0.9)
    place(x, thud(120, 55, 0.3, 0.09), 0, 1.0)
    place(x, modes([1600, 2500], [60, 80], 0.2), 0.002, 0.25)
    return fade_out(x)


def hit_arrow(b):
    d = 0.3
    x = canvas_of(d)
    place(x, high(b.noise(0.03), 2500) * exp_env(0.03, 0.006), 0, 0.7)
    place(x, thud(190, 120, 0.1, 0.03), 0.0, 0.9)
    shaft = np.sin(2 * np.pi * (640 + 25 * np.sin(2 * np.pi * 38 * t_of(0.24))) * t_of(0.24)) * np.exp(-t_of(0.24) * 22)
    place(x, shaft, 0.01, 0.35)
    return fade_out(x)


def hit_bolt(b):
    d = 0.3
    x = canvas_of(d)
    place(x, band(b.noise(0.05), 800, 4500) * exp_env(0.05, 0.01), 0, 0.8)
    place(x, thud(150, 70, 0.2, 0.06), 0, 1.1)
    place(x, modes([420, 910], [28, 40], 0.2), 0.004, 0.3)
    return fade_out(x)


def hit_blast(b):
    d = 0.5
    x = canvas_of(d)
    burst = swept_band(b.noise(0.35), 3200, 260, 1.2) * exp_env(0.35, 0.09)
    place(x, burst, 0, 1.0)
    place(x, thud(85, 40, 0.4, 0.12), 0, 1.0)
    crackle = (b.r.random(int(SR * 0.3)) > 0.995) * b.r.standard_normal(int(SR * 0.3)) * exp_env(0.3, 0.1)
    place(x, high(crackle, 1500) * 4, 0.03, 0.6)
    return fade_out(x)


def hit_magic(b):
    d = 0.4
    t = t_of(d)
    f = 1700 * (0.38) ** (t / d)
    mod = np.sin(2 * np.pi * 240 * t) * 180
    zap = np.sin(2 * np.pi * np.cumsum(f + mod) / SR) * exp_env(d, 0.09)
    glit = band(b.noise(d), 3500, 8000) * exp_env(d, 0.05) * (0.5 + 0.5 * np.sin(2 * np.pi * 45 * t))
    x = zap * 0.8 + glit * 0.35
    return fade_out(x)


def shot_arcane(b):
    """The Mage's bolt leaving the staff: a rising glassy zap with a
    wavering shimmer and a breath of air behind it."""
    d = 0.32
    t = t_of(d)
    f = 520 * (3.1) ** (t / d) + 60 * np.sin(2 * np.pi * 31 * t)
    zap = np.sin(2 * np.pi * np.cumsum(f) / SR) * shape(len(t), [(0, 0), (0.06, 1), (0.5, 0.5), (1, 0)])
    over = np.sin(2 * np.pi * np.cumsum(f * 1.5) / SR) * shape(len(t), [(0, 0), (0.1, 0.5), (1, 0)])
    air = swept_band(b.noise(d), 900, 4200, 0.8) * shape(len(t), [(0, 0), (0.1, 1), (1, 0)])
    glint = band(b.noise(d), 5000, 9000) * exp_env(d, 0.05) * (0.5 + 0.5 * np.sin(2 * np.pi * 52 * t))
    x = zap * 0.7 + over * 0.25 + air * 0.35 + glint * 0.2
    return fade_out(tail(x, 0.2, 0.15, b))


def shot_spark(b):
    """The Healer's spark off the censer: a soft swung whoosh, a tiny bell
    and a few embers crackling after it."""
    d = 0.45
    x = canvas_of(d)
    place(x, swept_band(b.noise(0.2), 500, 1800, 0.9) * shape(int(SR * 0.2), [(0, 0), (0.4, 1), (1, 0)]), 0, 0.6)
    place(x, modes([2093, 3136, 4186], [14, 20, 28], 0.4, [1.0, 0.5, 0.25]), 0.03, 0.4)
    n = int(SR * 0.3)
    crackle = (b.r.random(n) > 0.997) * b.r.standard_normal(n) * exp_env(0.3, 0.09)
    place(x, high(crackle, 2000) * 3, 0.05, 0.5)
    return fade_out(x)


def step_stone(b):
    d = 0.2
    x = canvas_of(d)
    place(x, band(b.noise(0.05), 1200, 3800) * exp_env(0.05, 0.008), 0, 0.7)
    place(x, thud(240, 130, 0.12, 0.025), 0, 1.0)
    place(x, modes([1900, 3300], [90, 120], 0.1), 0, 0.12)
    return fade_out(x)


def step_gravel(b):
    d = 0.22
    x = canvas_of(d)
    for i in range(7):
        at = i * 0.011 + b.r.random() * 0.006
        g = band(b.noise(0.012), 1800 + 500 * i % 3000, 7000) * exp_env(0.012, 0.003)
        place(x, g, at, 0.7 - i * 0.07)
    place(x, thud(180, 100, 0.1, 0.02), 0, 0.5)
    return fade_out(x)


def step_ash(b):
    d = 0.25
    x = canvas_of(d)
    place(x, low(b.noise(0.14), 1400) * exp_env(0.14, 0.04, 0.006), 0, 0.8)
    place(x, thud(150, 85, 0.14, 0.04), 0, 0.9)
    pops = (b.r.random(int(SR * 0.1)) > 0.998) * b.r.standard_normal(int(SR * 0.1))
    place(x, high(pops, 2500) * 3, 0.04, 0.35)
    return fade_out(x)


def step_wet(b):
    d = 0.28
    x = canvas_of(d)
    place(x, band(b.noise(0.12), 400, 2200) * exp_env(0.12, 0.035), 0, 0.8)
    place(x, thud(160, 90, 0.1, 0.025), 0, 0.7)
    place(x, sweep(300, 950, 0.07) * exp_env(0.07, 0.03), 0.05, 0.4)
    place(x, sweep(500, 1300, 0.05) * exp_env(0.05, 0.02), 0.11, 0.25)
    return fade_out(x)


def step_moss(b):
    d = 0.26
    x = canvas_of(d)
    place(x, low(b.noise(0.16), 900) * exp_env(0.16, 0.05, 0.008), 0, 0.9)
    place(x, thud(120, 70, 0.14, 0.04), 0, 0.8)
    place(x, sweep(200, 420, 0.06) * exp_env(0.06, 0.03), 0.06, 0.3)
    return fade_out(x)


def step_wood(b):
    d = 0.24
    x = canvas_of(d)
    place(x, thud(210, 120, 0.12, 0.03), 0, 1.0)
    place(x, modes([620, 990, 1500], [48, 60, 90], 0.2, [0.5, 0.35, 0.2]), 0, 0.45)
    place(x, band(b.noise(0.03), 1000, 3000) * exp_env(0.03, 0.006), 0, 0.4)
    return fade_out(x)


def door_pass(b):
    d = 0.85
    x = canvas_of(d)
    scrape = swept_band(b.noise(0.55), 700, 260, 0.9) * shape(int(SR * 0.55), [(0, 0), (0.15, 1), (0.8, 0.8), (1, 0)])
    place(x, scrape, 0, 0.7)
    place(x, low(b.noise(0.6), 220) * shape(int(SR * 0.6), [(0, 0), (0.2, 1), (1, 0)]), 0, 0.9)
    place(x, thud(95, 48, 0.3, 0.09), 0.5, 1.0)
    place(x, modes([260, 420], [14, 20], 0.3), 0.5, 0.15)
    return fade_out(tail(x, 0.25, 0.2, b))


def gold(b):
    d = 0.75
    x = canvas_of(d)
    at = 0.0
    for i in range(9):
        f0 = b.r.uniform(2800, 4200)
        coin = modes([f0, f0 * 1.51, f0 * 2.13], [26, 34, 48], 0.3, [1.0, 0.5, 0.3])
        place(x, coin, at, b.r.uniform(0.45, 0.8))
        at += b.r.uniform(0.012, 0.085) * (1 + i * 0.15)
    return fade_out(x)


def potion(b):
    d = 0.8
    x = canvas_of(d)
    place(x, modes([2400, 3700], [60, 80], 0.15), 0, 0.3)
    for i, at in enumerate([0.08, 0.26, 0.44]):
        glug = sweep(230 + 40 * i, 640 + 60 * i, 0.12) * shape(int(SR * 0.12), [(0, 0), (0.25, 1), (1, 0)])
        place(x, glug, at, 0.8)
        place(x, band(b.noise(0.12), 300, 1400) * shape(int(SR * 0.12), [(0, 0), (0.3, 1), (1, 0)]), at, 0.25)
    place(x, sweep(900, 400, 0.15) * exp_env(0.15, 0.06), 0.62, 0.25)
    return fade_out(x)


def holy_water(b):
    d = 1.0
    x = canvas_of(d)
    place(x, band(b.noise(0.2), 600, 5000) * exp_env(0.2, 0.05), 0, 0.5)
    for i, f in enumerate([1760, 2217, 2637, 3520]):
        place(x, modes([f, f * 2.01], [5, 9], 0.8, [1.0, 0.25]), 0.03 + 0.07 * i, 0.32)
    shimmer = band(b.noise(0.7), 4000, 9000) * exp_env(0.7, 0.25) * (0.5 + 0.5 * np.sin(2 * np.pi * 11 * t_of(0.7)))
    place(x, shimmer, 0.1, 0.18)
    return fade_out(x)


def bomb_toss(b):
    d = 0.55
    x = canvas_of(d)
    place(x, swept_band(b.noise(0.22), 350, 1600, 0.9) * shape(int(SR * 0.22), [(0, 0), (0.4, 1), (1, 0)]), 0, 0.9)
    fuse = high(b.noise(0.3), 3000) * (0.4 + 0.6 * (b.r.random(int(SR * 0.3)) > 0.6)) * shape(int(SR * 0.3), [(0, 0), (0.1, 1), (1, 0.3)])
    place(x, fuse, 0.22, 0.28)
    place(x, thud(160, 90, 0.08, 0.02), 0.0, 0.3)
    return fade_out(x)


def shield_up(b):
    d = 0.4
    x = canvas_of(d)
    place(x, swept_band(b.noise(0.15), 2500, 900, 0.8) * exp_env(0.15, 0.05), 0, 0.5)
    place(x, modes([540, 1010, 1620, 2400], [20, 26, 36, 50], 0.35, [0.7, 0.5, 0.35, 0.2]), 0.07, 0.8)
    place(x, thud(180, 100, 0.1, 0.03), 0.07, 0.5)
    return fade_out(x)


def axe_spin(b):
    d = 0.8
    t = t_of(d)
    n = b.noise(d)
    centre = 900 + 500 * np.sin(2 * np.pi * 3.2 * t)
    out = np.zeros_like(t)
    chunks = 40
    edges = np.linspace(0, len(t), chunks + 1).astype(int)
    for i in range(chunks):
        c = centre[edges[i]]
        out[edges[i]:edges[i + 1]] = band(n, c * 0.7, c * 1.4, 2)[edges[i]:edges[i + 1]]
    am = 0.35 + 0.65 * (0.5 + 0.5 * np.sin(2 * np.pi * 7 * t - 1.2)) ** 2
    x = out * am * shape(len(t), [(0, 0), (0.12, 1), (0.8, 0.9), (1, 0)])
    x += low(n, 240) * am * 0.5 * shape(len(t), [(0, 0), (0.2, 1), (1, 0)])
    return fade_out(x)


def helmet_pop(b):
    d = 0.7
    x = canvas_of(d)
    place(x, sweep(380, 980, 0.06) * exp_env(0.06, 0.02), 0, 0.8)
    place(x, modes([1500, 2420, 3650, 5100], [14, 18, 25, 36], 0.5, [0.8, 0.6, 0.4, 0.2]), 0.005, 0.7)
    for at, g in [(0.2, 0.5), (0.31, 0.3), (0.38, 0.18)]:
        place(x, modes([1200, 2000, 3100], [28, 34, 50], 0.25), at, g)
        place(x, thud(220, 140, 0.06, 0.015), at, g)
    return fade_out(x)


def armour_fall(b):
    d = 1.0
    x = canvas_of(d)
    at, gap, g = 0.0, 0.2, 1.0
    for _ in range(8):
        f = b.r.uniform(420, 900)
        place(x, modes([f, f * 1.7, f * 2.6, f * 3.9], [16, 22, 30, 44], 0.35, [0.7, 0.5, 0.3, 0.2]), at, 0.7 * g)
        place(x, thud(b.r.uniform(120, 200), 70, 0.1, 0.03), at, 0.6 * g)
        place(x, band(b.noise(0.06), 900, 5000) * exp_env(0.06, 0.012), at, 0.4 * g)
        at += gap
        gap *= 0.62
        g *= 0.8
    chain = band(b.noise(0.5), 2500, 7000) * (b.r.random(int(SR * 0.5)) > 0.8) * exp_env(0.5, 0.2)
    place(x, chain, 0.05, 0.25)
    return fade_out(x)


def _growl(b, dur, f0, f1, formant, rough):
    t = t_of(dur)
    f = f0 * (f1 / f0) ** (t / dur) * (1 + 0.025 * np.sin(2 * np.pi * 6.5 * t))
    phase = np.cumsum(f) / SR
    pulses = signal.sawtooth(2 * np.pi * phase, 0.2)
    pulses *= 1 + rough * np.sin(2 * np.pi * 31 * t)
    voiced = swept_band(pulses, formant[0], formant[1], 0.9, 20)
    breath = band(b.noise(dur), 200, 1800) * 0.35
    return drive(voiced * 1.4 + breath, 2.4)


def engine_whistle(b):
    d = 0.85
    t = t_of(d)
    vibrato = 1 + 0.025 * np.sin(2 * np.pi * 19 * t)
    pitch = (530 + 280 * np.sin(np.pi * t / d)) * vibrato
    phase = 2 * np.pi * np.cumsum(pitch) / SR
    voice = np.sin(phase) + 0.45 * np.sin(phase * 1.97) + 0.18 * np.sin(phase * 3.08)
    hiss = band(b.noise(d), 1600, 6000) * 0.15
    return fade_out((voice + hiss) * shape(len(t), [(0, 0), (0.06, 1), (0.8, 0.65), (1, 0)]), 0.05)


def boss_roar(b):
    d = 1.5
    g = _growl(b, d, 95, 52, (420, 950), 0.5)
    x = g * shape(len(g), [(0, 0), (0.12, 0.8), (0.4, 1), (0.8, 0.8), (1, 0)])
    x += thud(60, 38, d, 0.5) * 0.4 * shape(len(g), [(0, 0), (0.1, 1), (1, 0.2)])
    return fade_out(tail(x, 0.35, 0.2, b), 0.08)


def boss_rage(b):
    d = 1.05
    g = _growl(b, d, 150, 84, (600, 1500), 0.8)
    x = g * shape(len(g), [(0, 0), (0.08, 1), (0.6, 0.9), (1, 0)])
    x += high(b.noise(d), 3000) * 0.05 * shape(len(g), [(0, 0), (0.2, 1), (1, 0)])
    return fade_out(x, 0.06)


def boss_death(b):
    d = 2.0
    x = canvas_of(d)
    place(x, thud(70, 30, 1.6, 0.4), 0, 1.4)
    rumble = low(b.noise(d), 180) * shape(int(SR * d), [(0, 0), (0.05, 1), (0.5, 0.7), (1, 0)])
    place(x, rumble, 0, 1.5)
    place(x, _growl(b, 1.0, 80, 36, (300, 700), 0.4) * shape(int(SR * 1.0), [(0, 0), (0.1, 1), (1, 0)]), 0, 0.7)
    for _ in range(18):
        at = b.r.uniform(0.05, 1.5)
        place(x, band(b.noise(0.05), 600, 4000) * exp_env(0.05, 0.01), at, b.r.uniform(0.1, 0.4) * (1.6 - at / 1.5))
    place(x, modes([55, 110, 164], [4, 5, 6], 1.5), 0.0, 0.5)
    return fade_out(tail(x, 0.5, 0.25, b), 0.12)


def knight_fall(b):
    d = 0.95
    x = canvas_of(d)
    for at, g in [(0.0, 1.0), (0.14, 0.55), (0.24, 0.3)]:
        place(x, thud(140 - at * 150, 60, 0.3, 0.08), at, g)
        place(x, band(b.noise(0.12), 300, 3200) * exp_env(0.12, 0.03), at, g * 0.7)
        place(x, modes([540, 880, 1400, 2300], [16, 22, 30, 42], 0.6, [0.6, 0.45, 0.3, 0.2]), at, g * 0.55)
    scrape = swept_band(b.noise(0.4), 1800, 700, 0.9) * shape(int(SR * 0.4), [(0, 0), (0.2, 1), (1, 0)])
    place(x, scrape, 0.3, 0.2)
    return fade_out(tail(x, 0.25, 0.2, b))


def bones_clatter(b):
    d = 0.8
    x = canvas_of(d)
    at, gap = 0.0, 0.035
    for _ in range(40):
        if at >= 0.6:
            break
        f = b.r.uniform(700, 3200)
        place(x, modes([f, f * 2.4], [b.r.uniform(60, 130), 160], 0.12), at, b.r.uniform(0.3, 0.9) * (1.1 - at))
        if b.r.random() < 0.5:
            place(x, thud(b.r.uniform(260, 420), 150, 0.04, 0.01), at, 0.25)
        at += gap * b.r.uniform(0.6, 1.6)
        gap *= 1.07
    return fade_out(x)


def crack_open(b):
    d = 1.15
    x = canvas_of(d)
    place(x, _growl(b, d, 52, 40, (140, 320), 0.3) * shape(int(SR * d), [(0, 0), (0.3, 0.8), (1, 0)]), 0, 0.5)
    at = 0.0
    for _ in range(60):
        if at >= 0.95:
            break
        place(x, band(b.noise(0.04), 700, 7000) * exp_env(0.04, 0.007), at, b.r.uniform(0.25, 0.9))
        place(x, thud(b.r.uniform(80, 150), 50, 0.08, 0.02), at, 0.3)
        at += b.r.exponential(0.07) + 0.012
    fire = swept_band(b.noise(0.9), 400, 1800, 1.0) * shape(int(SR * 0.9), [(0, 0), (0.6, 0.6), (1, 0)])
    place(x, fire, 0.25, 0.3)
    return fade_out(x, 0.08)


def hand_rise(b):
    d = 1.3
    x = canvas_of(d)
    n = int(SR * d)
    place(x, low(b.noise(d), 260) * shape(n, [(0, 0), (0.7, 1), (1, 0.2)]), 0, 1.0)
    place(x, sweep(42, 88, d) * shape(n, [(0, 0), (0.6, 1), (1, 0.3)]), 0, 0.7)
    place(x, swept_band(b.noise(d), 300, 1100, 0.8) * shape(n, [(0, 0), (0.5, 0.7), (1, 0.2)]), 0, 0.35)
    for _ in range(14):
        at = b.r.uniform(0.1, 1.1)
        place(x, band(b.noise(0.05), 500, 3500) * exp_env(0.05, 0.01), at, b.r.uniform(0.1, 0.35))
    return fade_out(x, 0.1)


def hand_grab(b):
    d = 0.6
    x = canvas_of(d)
    place(x, thud(110, 48, 0.4, 0.1), 0, 1.0)
    for at in [0.0, 0.05, 0.09, 0.14, 0.2]:
        place(x, band(b.noise(0.03), 900, 6000) * exp_env(0.03, 0.006), at, b.r.uniform(0.4, 0.8))
        place(x, modes([b.r.uniform(1100, 2400)], [90], 0.08), at, 0.3)
    place(x, high(b.noise(0.4), 2500) * exp_env(0.4, 0.13), 0.04, 0.12)
    place(x, _growl(b, 0.4, 70, 55, (250, 600), 0.4) * shape(int(SR * 0.4), [(0, 0), (0.2, 1), (1, 0)]), 0.05, 0.4)
    return fade_out(x)


def hand_drag(b):
    d = 2.1
    x = canvas_of(d)
    n = int(SR * d)
    t = t_of(d)
    place(x, low(b.noise(d), 150) * (0.65 + 0.35 * np.sin(2 * np.pi * 5.5 * t)) * shape(n, [(0, 0), (0.08, 1), (0.8, 0.9), (1, 0)]), 0, 1.4)
    place(x, sweep(120, 36, d) * shape(n, [(0, 0), (0.1, 1), (0.85, 0.8), (1, 0)]), 0, 0.8)
    place(x, _growl(b, d, 60, 34, (200, 500), 0.5) * shape(n, [(0, 0), (0.2, 0.7), (1, 0)]), 0, 0.5)
    at = 0.1
    for _ in range(40):
        if at >= 1.9:
            break
        place(x, band(b.noise(0.07), 300, 3000) * exp_env(0.07, 0.015), at, b.r.uniform(0.1, 0.4))
        at += b.r.exponential(0.12) + 0.02
    return fade_out(x, 0.15)


def ground_close(b):
    d = 1.0
    x = canvas_of(d)
    place(x, thud(75, 30, 0.8, 0.25), 0, 1.5)
    place(x, band(b.noise(0.4), 120, 2500) * exp_env(0.4, 0.1), 0, 1.0)
    place(x, high(b.noise(0.7), 3000) * exp_env(0.7, 0.25), 0.05, 0.12)
    for at in [0.15, 0.27, 0.4, 0.52]:
        place(x, band(b.noise(0.05), 400, 4000) * exp_env(0.05, 0.01), at, 0.35)
    return fade_out(tail(x, 0.3, 0.2, b))


def hell_gate_rise(b):
    d = 2.4
    n = int(SR * d)
    x = canvas_of(d)
    fire = swept_band(b.noise(d), 300, 2600, 1.1) * shape(n, [(0, 0), (0.7, 1), (0.9, 0.9), (1, 0)])
    place(x, fire, 0, 0.9)
    place(x, low(b.noise(d), 200) * shape(n, [(0, 0), (0.4, 1), (1, 0.2)]), 0, 0.9)
    horn = signal.sawtooth(2 * np.pi * np.cumsum(55 * (1 + 0.01 * np.sin(2 * np.pi * 4 * t_of(d)))) / SR)
    place(x, low(horn, 420) * shape(n, [(0, 0), (0.5, 0.8), (1, 0)]), 0, 0.6)
    crack = (b.r.random(n) > 0.9992) * b.r.standard_normal(n)
    place(x, high(crack, 1200) * 5, 0, 0.5)
    place(x, thud(70, 34, 0.8, 0.25), 0, 0.8)
    return fade_out(x, 0.2)


def hell_gate_enter(b):
    d = 1.4
    n = int(SR * d)
    x = canvas_of(d)
    whoosh = swept_band(b.noise(d), 3200, 160, 1.2) * shape(n, [(0, 0), (0.1, 1), (1, 0)])
    place(x, whoosh, 0, 0.9)
    place(x, sweep(520, 46, d) * shape(n, [(0, 0), (0.15, 1), (1, 0)]), 0, 0.6)
    place(x, thud(90, 36, 0.9, 0.3), 0.05, 0.9)
    place(x, swept_band(b.noise(0.5), 800, 2200, 1.0) * exp_env(0.5, 0.2), 0, 0.35)
    return fade_out(tail(x, 0.35, 0.2, b), 0.15)


def circle_toll(b):
    d = 2.3
    x = canvas_of(d)
    base = 98.0
    ratios = [1.0, 2.0, 2.42, 2.97, 4.2, 5.4]
    decays = [1.7, 2.3, 3.1, 3.9, 5.5, 8.0]
    amps = [1.0, 0.7, 0.45, 0.4, 0.25, 0.15]
    place(x, modes([base * r for r in ratios], decays, d, amps), 0, 1.0)
    place(x, modes([base * 1.5, base * 3.0], [2.4, 3.4], d), 0.0, 0.35)
    place(x, thud(220, 110, 0.15, 0.04), 0, 0.6)
    place(x, band(b.noise(0.1), 500, 3000) * exp_env(0.1, 0.02), 0, 0.2)
    return fade_out(tail(x, 0.45, 0.2, b), 0.25)


# ─── the finishers (rules::gore) ─────────────────────────────────────────────

def gore_kick(b):
    """A boot into a monster's ribs: a breath of leather, a dull heavy thump,
    a slap, a rattle of mail."""
    d = 0.34
    x = canvas_of(d)
    n = int(SR * 0.1)
    place(x, swept_band(b.noise(0.1), 500, 1800, 0.9) * shape(n, [(0, 0), (0.7, 1), (1, 0.2)]), 0, 0.35)
    place(x, thud(150, 52, 0.28, 0.07), 0.07, 1.2)
    place(x, low(b.noise(0.06), 1400) * exp_env(0.06, 0.012), 0.07, 0.8)
    place(x, modes([260, 410, 880], [38, 52, 70], 0.2, [1.0, 0.6, 0.3]), 0.075, 0.25)
    return fade_out(tail(x, 0.15, 0.12, b))


def gore_thunk(b):
    """A shaft and the body on it into wood and stone: a crack, a knock, the
    weight arriving, and the shaft's short twang."""
    d = 0.3
    x = canvas_of(d)
    place(x, high(b.noise(0.025), 1800) * exp_env(0.025, 0.005), 0, 0.9)
    place(x, thud(210, 95, 0.14, 0.04), 0, 1.1)
    place(x, thud(90, 45, 0.22, 0.07), 0.005, 0.7)
    t = t_of(0.24)
    twang = np.sin(2 * np.pi * (690 + 30 * np.sin(2 * np.pi * 34 * t)) * t) * np.exp(-t * 26)
    place(x, twang, 0.012, 0.3)
    place(x, modes([1240, 2100], [48, 70], 0.2, [0.5, 0.25]), 0.004, 0.25)
    return fade_out(x)


def gore_chop(b):
    """A blade through a neck or a waist: the whistle of the swing, the wet
    cut, the weight behind it, a bright ring of steel."""
    d = 0.4
    x = canvas_of(d)
    n = int(SR * 0.12)
    place(x, swept_band(b.noise(0.12), 4200, 900, 0.9) * shape(n, [(0, 0), (0.35, 1), (1, 0)]), 0, 0.5)
    place(x, band(b.noise(0.09), 300, 2800) * exp_env(0.09, 0.02), 0.09, 1.0)
    place(x, thud(180, 70, 0.2, 0.05), 0.095, 0.9)
    place(x, modes([1900, 3100], [70, 95], 0.2), 0.095, 0.22)
    t = t_of(0.2)
    squelch = band(b.noise(0.2), 200, 1200) * (0.55 + 0.45 * np.sin(2 * np.pi * 26 * t)) * exp_env(0.2, 0.07)
    place(x, squelch, 0.11, 0.55)
    return fade_out(tail(x, 0.12, 0.1, b))


def gore_splat(b):
    """A wet slap on stone: the heavy smack, a low thud, and a few drops
    falling after it."""
    d = 0.36
    x = canvas_of(d)
    place(x, low(b.noise(0.12), 2200) * exp_env(0.12, 0.03), 0, 1.0)
    place(x, band(b.noise(0.06), 600, 3500) * exp_env(0.06, 0.012), 0, 0.6)
    place(x, thud(120, 48, 0.26, 0.08), 0, 1.0)
    for at, f in [(0.1, 1250), (0.15, 1020), (0.2, 880), (0.27, 1500)]:
        t = t_of(0.05)
        blip = np.sin(2 * np.pi * np.cumsum(f * (0.5 ** (t / 0.05))) / SR) * np.exp(-t * 70)
        place(x, blip, at, 0.22)
    return fade_out(x)


def gore_bones(b):
    """Bones coming apart and settling: a hollow crack, then dry clicks and
    ticks across the floor, thinning out."""
    d = 0.6
    x = canvas_of(d)
    place(x, high(b.noise(0.03), 1500) * exp_env(0.03, 0.006), 0, 0.7)
    place(x, thud(240, 110, 0.14, 0.04), 0, 0.7)
    place(x, modes([420, 780, 1330], [45, 60, 80], 0.25, [1.0, 0.5, 0.3]), 0.004, 0.35)
    at = 0.04
    while at < 0.52:
        f = 1300 + 3600 * b.r.random()
        click = modes([f, f * 1.9], [140 + 120 * b.r.random(), 220], 0.07)
        place(x, click, at, 0.35 * (1.0 - at / 0.7))
        at += 0.012 + 0.07 * b.r.random() * (1 + at * 2.5)
    return fade_out(tail(x, 0.15, 0.1, b))


def gore_burst(b):
    """Something bursting apart: a wet detonation, the low thud under it, and
    chunks pattering down after."""
    d = 0.7
    x = canvas_of(d)
    n = int(SR * 0.3)
    place(x, swept_band(b.noise(0.3), 3000, 220, 1.2) * exp_env(0.3, 0.07), 0, 1.0)
    place(x, thud(95, 38, 0.45, 0.13), 0, 1.3)
    place(x, low(b.noise(0.1), 1500) * exp_env(0.1, 0.02), 0, 0.8)
    for at in [0.09, 0.14, 0.2, 0.26, 0.33, 0.41, 0.5]:
        w = band(b.noise(0.05), 300 + 500 * b.r.random(), 1800) * exp_env(0.05, 0.012)
        place(x, w, at, 0.45 * (1.0 - at / 0.8))
    place(x, thud(70, 36, 0.4, 0.14), 0.03, 0.5)
    return fade_out(tail(x, 0.18, 0.14, b))


# ─── the Bard (`rules::bard`) ────────────────────────────────────────────────
# The Bard's songs are slices of Knight Shift Radio's trance (`bard_tracks/`),
# not synthesised here; what stays is the note that lands: an open hi-hat on
# top of the groove, so hitting fills it in.

def hit_note(b):
    """A note lands: an open hi-hat, six square waves out of tune with each
    other through a high band (the old drum machines' metal), with a hiss."""
    d = 0.22
    t = t_of(d)
    metal = sum(np.sign(np.sin(2 * np.pi * f * t)) for f in [205.3, 304.4, 369.6, 522.7, 540.0, 800.0])
    x = band(metal, 7000, 12500, 3) * exp_env(d, 0.055, 0.0005)
    x = x / (np.max(np.abs(x)) + 1e-9)
    x += high(b.noise(d), 8000) * exp_env(d, 0.04, 0.0005) * 0.6
    return fade_out(tail(x, 0.08, 0.1, b), 0.04)


# ─── moving filters, blades through air, sparkles ────────────────────────────

def svf(x, fc, q=1.0, mode="band"):
    """A state-variable filter whose corner `fc` (Hz: a number, or a curve
    a sample) may move as it plays without a click: a whoosh's band riding
    the blade's speed, a breath's formants opening. The band output has a
    gain of one at its centre."""
    n = len(x)
    fc = np.broadcast_to(np.asarray(fc, dtype=float), (n,))
    g = np.tan(np.pi * np.clip(fc, 20.0, SR * 0.45) / SR)
    k = 1.0 / q
    a1 = 1.0 / (1.0 + g * (g + k))
    a2 = g * a1
    a3 = g * a2
    xs, a1s, a2s, a3s = x.tolist(), a1.tolist(), a2.tolist(), a3.tolist()
    out = [0.0] * n
    ic1 = ic2 = 0.0
    pick = {"band": 0, "low": 1, "high": 2}[mode]
    for i in range(n):
        v3 = xs[i] - ic2
        v1 = a1s[i] * ic1 + a2s[i] * v3
        v2 = ic2 + a2s[i] * ic1 + a3s[i] * v3
        ic1 = 2.0 * v1 - ic1
        ic2 = 2.0 * v2 - ic2
        out[i] = k * v1 if pick == 0 else v2 if pick == 1 else xs[i] - k * v1 - v2
    return np.array(out)


def speed_of(dur, peak, rise, fall):
    """A stroke's speed through the air, 0 to 1: it swells to its fastest
    at `peak` (a fraction of `dur`) and falls away, `rise` and `fall` how
    broad either side is."""
    u = t_of(dur) / dur
    return np.where(u < peak, np.exp(-((u - peak) / rise) ** 2), np.exp(-((u - peak) / fall) ** 2))


def whoosh(b, dur, lo, hi, peak=0.5, rise=0.3, fall=0.16, q=1.2, whistle=0.0, air=0.0, flutter=0.0, rate=0.0):
    """A blade through the air: noise through a band that climbs from `lo`
    to `hi` Hz with the blade's speed, loud as the speed's square. Its
    edge sings a little (`whistle`, Hz at full speed, a narrow band); a
    heavy one shoves a low push of air before it (`air`); a flat or bearded
    one flutters as it turns (`flutter` deep at `rate` Hz)."""
    v = speed_of(dur, peak, rise, fall)
    t = t_of(dur)
    out = svf(b.noise(dur), lo + (hi - lo) * v, q) * v ** 2
    if whistle:
        out += svf(b.noise(dur), whistle * (0.55 + 0.45 * v), 14.0) * v ** 3 * 0.9
    if air:
        out += svf(b.noise(dur), 70 + 160 * v, 0.8, "low") * v ** 2 * air * 2.2
    if flutter:
        out *= 1.0 - flutter * (0.5 + 0.5 * np.sin(2 * np.pi * rate * t + 1.3)) * v
    return out


def ping(f, tau, dur=None, partial=0.25):
    """One tiny bright ping: a sine and its octave, gone in `tau`."""
    dur = dur or min(0.4, tau * 7)
    t = t_of(dur)
    x = (np.sin(2 * np.pi * f * t) + partial * np.sin(2 * np.pi * 2.01 * f * t)) * np.exp(-t / tau)
    a = int(SR * 0.0015)
    x[:a] *= np.linspace(0, 1, a)
    return x


def sparkle(b, x, start, end, n, lo, hi, gain, tau=(0.012, 0.03), thin=1.0):
    """`n` pings scattered from `start` to `end` s, `lo` to `hi` Hz, each a
    random loudness; with `thin` above one they come thinner as they go."""
    for _ in range(n):
        k = b.r.random() ** thin
        at = start + (end - start) * k
        f = b.r.uniform(lo, hi)
        place(x, ping(f, b.r.uniform(*tau)), at, gain * b.r.uniform(0.35, 1.0) * (1.0 - 0.6 * k))


def crackles(b, dur, density, lo, hi, tau):
    """Sparse clicks through a band, thinning out over `tau`: fire, ice
    forming, static, grit."""
    n = int(SR * dur)
    clicks = (b.r.random(n) > 1.0 - density) * b.r.standard_normal(n)
    return band(clicks, lo, hi) * exp_env(dur, tau, 0.0005) * 6


def pad(freqs, dur, env, vib=4.8, depth=0.004, cents=5.0, phase=0.0):
    """A held chord: each note two voices `cents` apart, a slow vibrato,
    under the envelope points `env` (as `shape` takes them)."""
    t = t_of(dur)
    out = np.zeros_like(t)
    for i, f in enumerate(freqs):
        for c in (-cents, cents):
            fv = f * 2 ** (c / 1200) * (1 + depth * np.sin(2 * np.pi * vib * t + i + phase))
            out += np.sin(2 * np.pi * np.cumsum(fv) / SR) / (1 + 0.35 * i)
    return out * shape(len(t), env)


def chime(f, dur, decay, partials=((1.0, 1.0), (2.0, 0.4), (3.0, 0.18), (4.2, 0.08))):
    """A struck chime at `f`: its partials, the higher ones dying quicker."""
    return modes([f * r for r, _ in partials], [decay * (1 + 0.8 * i) for i in range(len(partials))], dur,
                 [a for _, a in partials])


# ─── the blades (`rules::melee`): swings by the arm, contacts by their weight ─
# A swing is the arm's own (`melee::Blade`): the spear a quick high flick,
# the sword a clean cut with its edge singing, the axe's bearded head
# chopping round, the cleaver's flat slab fluttering, the maul's block
# shoving the air before it. A contact is the blow's weight
# (`melee::Tier`), the steel and the meat together in one take so a
# stroke is two sounds, never four.

def swing_spear(b):
    """A spear thrust: light and quick, a tight "fwip" as the point goes
    out, the shaft's whistle on it, a tick of the grip."""
    d = 0.24
    x = canvas_of(d)
    place(x, band(b.noise(0.012), 1500, 5000) * exp_env(0.012, 0.003), 0, 0.18)
    place(x, whoosh(b, 0.2, 900, 4600, peak=0.2, rise=0.12, fall=0.3, q=1.7, whistle=3200), 0.004, 1.0)
    return fade_out(tail(x, 0.08, 0.08, b), 0.02)


def swing_sword(b):
    """A sword's cut: a clean whoosh climbing as the blade comes round, its
    edge singing, a faint shimmer of steel at the fastest."""
    d = 0.34
    x = canvas_of(d)
    place(x, band(b.noise(0.015), 900, 3000) * exp_env(0.015, 0.004), 0, 0.15)
    place(x, whoosh(b, 0.3, 550, 3600, peak=0.18, rise=0.11, fall=0.34, q=1.3, whistle=2100, air=0.15), 0.004, 1.0)
    ring = modes([3170, 4810, 6650], [26, 34, 44], 0.16, [0.5, 0.35, 0.2])
    place(x, ring * shape(len(ring), [(0, 0), (0.15, 1), (1, 1)]), 0.04, 0.05)
    return fade_out(tail(x, 0.1, 0.1, b), 0.03)


def swing_axe(b):
    """An axe coming round: lower and chunkier than a sword, the bearded
    head chopping the air as it turns, a push of air behind it."""
    d = 0.38
    x = canvas_of(d)
    place(x, low(b.noise(0.02), 1500) * exp_env(0.02, 0.005), 0, 0.2)
    place(x, whoosh(b, 0.34, 320, 2000, peak=0.19, rise=0.12, fall=0.36, q=1.0, air=0.45, flutter=0.45, rate=19), 0.004, 1.0)
    return fade_out(tail(x, 0.12, 0.1, b), 0.03)


def swing_cleaver(b):
    """A cleaver's slab: broad and flat, a heavy "whum" that flutters as
    the blade's face catches the air."""
    d = 0.4
    x = canvas_of(d)
    place(x, low(b.noise(0.02), 1200) * exp_env(0.02, 0.006), 0, 0.2)
    place(x, whoosh(b, 0.36, 260, 1500, peak=0.2, rise=0.13, fall=0.36, q=0.9, air=0.7, flutter=0.55, rate=31), 0.004, 1.0)
    return fade_out(tail(x, 0.12, 0.1, b), 0.03)


def swing_maul(b):
    """A maul's heave: slow and deep, the block shoving the air ahead of
    it in a "vwoom", the haft creaking at the top of the swing."""
    d = 0.5
    x = canvas_of(d)
    place(x, modes([410, 690, 1130], [40, 55, 70], 0.12, [0.6, 0.4, 0.2]), 0.0, 0.12)
    w = whoosh(b, 0.46, 170, 950, peak=0.24, rise=0.16, fall=0.36, q=0.8, air=1.2)
    v = speed_of(0.46, 0.24, 0.16, 0.36)
    w += sweep(52, 74, 0.46) * v ** 2 * 0.35
    place(x, w, 0.006, 1.0)
    return fade_out(tail(x, 0.15, 0.1, b), 0.04)


def hit_flesh_light(b):
    """A light blow landing: the edge's tick, a quick slice, a small wet
    slap of meat under it."""
    d = 0.26
    x = canvas_of(d)
    place(x, high(b.noise(0.006), 3200) * exp_env(0.006, 0.0012, 0.0002), 0, 1.0)
    place(x, band(b.noise(0.05), 1000, 5000) * exp_env(0.05, 0.008, 0.0005), 0.001, 0.8)
    place(x, low(b.noise(0.06), 1300) * exp_env(0.06, 0.016, 0.0006), 0.002, 0.9)
    place(x, thud(210, 110, 0.09, 0.02), 0.002, 0.6)
    t = t_of(0.08)
    squelch = band(b.noise(0.08), 300, 1500) * (0.55 + 0.45 * np.sin(2 * np.pi * 42 * t)) * exp_env(0.08, 0.02)
    place(x, squelch, 0.01, 0.4)
    place(x, modes([2650, 3900], [70, 95], 0.1, [0.6, 0.3]), 0.0, 0.12)
    return fade_out(tail(x, 0.08, 0.08, b), 0.02)


def hit_flesh_medium(b):
    """A solid blow: a crack, a meaty thwack with a knock of bone in it,
    the weight behind it, and the wet of the cut."""
    d = 0.38
    x = canvas_of(d)
    place(x, high(b.noise(0.006), 2500) * exp_env(0.006, 0.0018, 0.0002), 0, 1.0)
    place(x, band(b.noise(0.07), 400, 3000) * exp_env(0.07, 0.02, 0.0005), 0.001, 1.0)
    place(x, low(b.noise(0.1), 950) * exp_env(0.1, 0.035, 0.0008), 0.002, 1.0)
    place(x, modes([310, 520, 870], [50, 65, 85], 0.18, [1.0, 0.6, 0.35]), 0.001, 0.32)
    place(x, thud(165, 68, 0.2, 0.045), 0.002, 0.95)
    t = t_of(0.18)
    squelch = band(b.noise(0.18), 200, 1400) * (0.5 + 0.5 * np.sin(2 * np.pi * 27 * t)) * exp_env(0.18, 0.05, 0.004)
    place(x, squelch, 0.014, 0.55)
    place(x, crackles(b, 0.05, 0.004, 1500, 5000, 0.015), 0.004, 0.35)
    return fade_out(tail(x, 0.12, 0.1, b), 0.03)


def hit_flesh_heavy(b):
    """A heavy blow, a finisher's or a maul's: a crack, bone crunching
    under it, the meat giving with a deep thud driven hot under it all, a
    short room."""
    d = 0.6
    x = canvas_of(d)
    place(x, high(b.noise(0.008), 1800) * exp_env(0.008, 0.0025, 0.0002), 0, 1.0)
    at = 0.002
    for i in range(8):
        crunch = band(b.noise(0.02), b.r.uniform(700, 1400), b.r.uniform(3500, 6500)) * exp_env(0.02, 0.0035, 0.0003)
        place(x, crunch, at, 0.8 * (1 - i * 0.08))
        at += b.r.uniform(0.003, 0.008)
    place(x, modes([620, 1150, 1910], [60, 80, 110], 0.15, [1.0, 0.6, 0.35]), 0.003, 0.35)
    place(x, band(b.noise(0.16), 140, 700) * exp_env(0.16, 0.06, 0.001), 0.001, 1.3)
    place(x, low(b.noise(0.12), 1100) * exp_env(0.12, 0.04, 0.001), 0.001, 0.9)
    place(x, drive(thud(135, 46, 0.4, 0.09), 1.8), 0.002, 1.1)
    t = t_of(0.26)
    squelch = band(b.noise(0.26), 180, 1100) * (0.5 + 0.5 * np.sin(2 * np.pi * 21 * t)) * exp_env(0.26, 0.08, 0.006)
    place(x, squelch, 0.02, 0.65)
    return fade_out(tail(x, 0.22, 0.16, b), 0.05)


# ─── the finishers' big moments (`rules::gore`) ──────────────────────────────

def gore_behead(b):
    """A head taken: the swing's last whistle, a hard thwack through the
    neck with the vertebra's crack in it, then the spray, pulsing."""
    d = 0.72
    x = canvas_of(d)
    hit = 0.002
    place(x, high(b.noise(0.006), 2200) * exp_env(0.006, 0.0018, 0.0002), hit, 1.0)
    place(x, band(b.noise(0.06), 500, 3200) * exp_env(0.06, 0.017, 0.0005), hit, 1.0)
    place(x, thud(175, 68, 0.22, 0.05), hit, 1.15)
    place(x, modes([1450, 2380, 3620], [90, 120, 150], 0.08, [1.0, 0.6, 0.3]), hit + 0.004, 0.4)
    place(x, high(b.noise(0.004), 3000) * exp_env(0.004, 0.0008, 0.0001), hit + 0.006, 0.6)
    # The spray: a hiss of drops, three pulses of it, falling off.
    n = 0.62
    t = t_of(n)
    drops = (b.r.random(int(SR * n)) > 0.75).astype(float)
    hiss = low(svf(b.noise(n) * (0.4 + 0.6 * drops), 3200 - 1400 * (t / n), 0.9), 7500)
    pulse = 0.55 + 0.45 * np.cos(2 * np.pi * 6.5 * t) ** 2
    place(x, hiss * pulse * shape(len(t), [(0, 0), (0.04, 1), (0.35, 0.55), (1, 0)]), hit + 0.012, 0.32)
    gurgle = band(b.noise(0.25), 200, 750) * (0.5 + 0.5 * np.sin(2 * np.pi * 13 * t_of(0.25))) * exp_env(0.25, 0.08, 0.01)
    place(x, gurgle, hit + 0.02, 0.35)
    return fade_out(tail(x, 0.15, 0.1, b), 0.06)


def gore_head_thud(b):
    """The head lands: a dull knock of skull on stone, a wet slap, a
    smaller bounce, a short roll."""
    d = 0.46
    x = canvas_of(d)
    place(x, thud(185, 72, 0.18, 0.045), 0, 1.1)
    place(x, low(b.noise(0.03), 2600) * exp_env(0.03, 0.007, 0.0004), 0, 0.65)
    place(x, modes([240, 410, 690], [40, 55, 80], 0.2, [1.0, 0.5, 0.25]), 0.001, 0.28)
    place(x, thud(205, 92, 0.1, 0.028), 0.11, 0.5)
    place(x, low(b.noise(0.02), 2200) * exp_env(0.02, 0.005, 0.0004), 0.11, 0.3)
    t = t_of(0.22)
    roll = band(b.noise(0.22), 300, 1000) * (0.4 + 0.6 * np.abs(np.sin(2 * np.pi * 8.5 * t))) * shape(len(t), [(0, 0), (0.1, 1), (1, 0)])
    place(x, roll, 0.17, 0.13)
    return fade_out(tail(x, 0.12, 0.1, b), 0.04)


def gore_bisect(b):
    """A body cut through: a long wet rip as the blade goes across, the
    spine's crack, the weight of it parting, the spill after."""
    d = 0.72
    x = canvas_of(d)
    n = 0.17
    t = t_of(n)
    tear = np.abs(low(b.noise(n), 70)) * 4 + 0.3
    rip = svf(b.noise(n), 3600 * (0.15 ** (t / n)), 1.1) * tear * shape(len(t), [(0, 0), (0.08, 1), (0.6, 0.8), (1, 0)])
    place(x, rip, 0, 0.85)
    place(x, high(b.noise(0.006), 2000) * exp_env(0.006, 0.002, 0.0002), 0.025, 0.9)
    place(x, modes([880, 1580, 2700], [70, 90, 120], 0.12, [1.0, 0.5, 0.3]), 0.026, 0.3)
    place(x, low(b.noise(0.12), 1800) * exp_env(0.12, 0.05, 0.001), 0.02, 0.9)
    place(x, thud(135, 46, 0.4, 0.1), 0.025, 1.25)
    t = t_of(0.3)
    squelch = band(b.noise(0.3), 180, 1000) * (0.5 + 0.5 * np.sin(2 * np.pi * 17 * t)) * exp_env(0.3, 0.1, 0.008)
    place(x, squelch, 0.05, 0.6)
    for at, f in [(0.22, 1180), (0.29, 960), (0.37, 1420), (0.46, 830)]:
        tt = t_of(0.05)
        blip = np.sin(2 * np.pi * np.cumsum(f * (0.5 ** (tt / 0.05))) / SR) * np.exp(-tt * 70)
        place(x, blip, at, 0.16)
    place(x, band(b.noise(0.4), 900, 3500) * exp_env(0.4, 0.14, 0.02), 0.1, 0.12)
    return fade_out(tail(x, 0.18, 0.12, b), 0.06)


def gore_slump(b):
    """The halves fall: one heavy soft thud and a wet slap, the other a
    beat after, a squish as it settles."""
    d = 0.56
    x = canvas_of(d)
    place(x, thud(108, 42, 0.4, 0.11), 0, 1.25)
    place(x, low(b.noise(0.05), 2000) * exp_env(0.05, 0.014, 0.0006), 0, 0.75)
    place(x, band(b.noise(0.03), 500, 2600) * exp_env(0.03, 0.008, 0.0004), 0, 0.4)
    place(x, thud(128, 50, 0.3, 0.08), 0.095, 0.75)
    place(x, low(b.noise(0.04), 2200) * exp_env(0.04, 0.01, 0.0005), 0.095, 0.45)
    t = t_of(0.22)
    squish = band(b.noise(0.22), 200, 900) * (0.5 + 0.5 * np.sin(2 * np.pi * 12 * t)) * exp_env(0.22, 0.07, 0.01)
    place(x, squish, 0.15, 0.3)
    return fade_out(tail(x, 0.2, 0.14, b), 0.05)


# ─── the parry (`melee::parry`) ──────────────────────────────────────────────

def parry(b):
    """A bullet turned off the blade: a sharp tick, a bright scrape of
    edge on it, and a high clean ting that rings on and shimmers (two of
    its modes a hair apart), well above the shield's clang."""
    d = 1.1
    x = canvas_of(d)
    place(x, high(b.noise(0.004), 4500) * exp_env(0.004, 0.0008, 0.0001), 0, 1.0)
    place(x, svf(b.noise(0.04), np.linspace(4200, 9000, int(SR * 0.04)), 2.0) * exp_env(0.04, 0.012, 0.0005), 0.001, 0.45)
    ting = modes([2150, 2163, 3527, 5240, 7310, 1182], [3.0, 3.3, 5.2, 8.5, 13, 7],
                 d, [1.0, 0.65, 0.45, 0.3, 0.14, 0.22])
    place(x, ting, 0.001, 0.85)
    place(x, thud(330, 180, 0.05, 0.012), 0, 0.3)
    return fade_out(tail(x, 0.35, 0.16, b), 0.15)


def parry_whizz(b):
    """The turned bullet going back: a whine dropping as it recedes,
    fluttering as it spins, the air torn round it."""
    d = 0.44
    x = canvas_of(d)
    n = 0.38
    t = t_of(n)
    f = 1550 * (0.42 ** (t / n))
    env = shape(len(t), [(0, 0), (0.03, 1), (0.25, 0.6), (1, 0)])
    flutter = 1 - 0.25 * (0.5 + 0.5 * np.sin(2 * np.pi * 62 * t))
    phase = 2 * np.pi * np.cumsum(f) / SR
    tone = (np.sin(phase) + 0.3 * np.sin(2 * phase)) * env * flutter
    air = low(svf(b.noise(n), f * 1.4, 3.0), 7000) * env
    place(x, tone, 0.03, 0.55)
    place(x, air, 0.03, 0.9)
    return fade_out(x, 0.04)


# ─── magic by family (`rules::vfx::Voice`) ───────────────────────────────────
# Each family casts in two beats a breath apart, as the magic pass draws it:
# the wind-up while the rune opens (`magic::sigil`, a fifth of a second),
# the release at the hand; and each breaks on what it strikes its own way.

WIND = 0.14  # the release, after the wind-up


def cast_fire(b):
    """Fire: a roar drawn up, then a whoomph as it goes, the flames
    fluttering and crackling as it dies."""
    d = 0.8
    x = canvas_of(d)
    n = int(SR * WIND)
    ramp = np.linspace(0, 1, n) ** 2
    place(x, svf(b.noise(WIND), np.linspace(250, 1400, n), 0.9) * ramp, 0, 0.6)
    place(x, crackles(b, WIND, 0.002, 1500, 6000, 1.0) * ramp, 0, 0.3)
    place(x, drive(thud(140, 46, 0.42, 0.11), 1.6), WIND, 1.1)
    m = 0.6
    t = t_of(m)
    flutter = 1 + 0.35 * np.sin(2 * np.pi * 17 * t + 2 * np.sin(2 * np.pi * 5 * t))
    roar = svf(b.noise(m), 2600 * (0.2 ** (t / m)) + 400, 0.7) * exp_env(m, 0.2, 0.006) * flutter
    place(x, roar, WIND, 1.0)
    place(x, crackles(b, 0.6, 0.0018, 1500, 6500, 0.25), WIND + 0.02, 0.55)
    return fade_out(tail(x, 0.25, 0.15, b), 0.08)


def impact_fire(b):
    """Fire breaking: a pop, a boom with a burst of flame through it, a
    sizzle of embers after."""
    d = 0.75
    x = canvas_of(d)
    place(x, high(b.noise(0.005), 2500) * exp_env(0.005, 0.0015, 0.0002), 0, 1.0)
    place(x, band(b.noise(0.05), 300, 3000) * exp_env(0.05, 0.018, 0.0004), 0, 0.85)
    place(x, drive(thud(120, 44, 0.4, 0.09), 1.6), 0, 1.1)
    place(x, low(b.noise(0.2), 700) * exp_env(0.2, 0.07, 0.001), 0, 0.9)
    m = 0.4
    t = t_of(m)
    place(x, svf(b.noise(m), 3000 * (0.1 ** (t / m)) + 250, 0.8) * exp_env(m, 0.14, 0.003), 0, 0.95)
    sizzle = high(b.noise(0.6), 3500) * shape(int(SR * 0.6), [(0, 0), (0.08, 1), (1, 0)])
    place(x, sizzle, 0.04, 0.16)
    place(x, crackles(b, 0.6, 0.0022, 1800, 7000, 0.22), 0.03, 0.5)
    return fade_out(tail(x, 0.2, 0.15, b), 0.08)


def cast_frost(b):
    """Frost: a glassy shimmer climbing, then a crystal chime as it goes,
    the ice crackling as it forms, a breath of cold."""
    d = 0.9
    x = canvas_of(d)
    t = t_of(WIND)
    ramp = (t / WIND) ** 1.5
    for f0, g in [(1250, 0.5), (1870, 0.35), (2640, 0.25)]:
        f = f0 * 2 ** (t / WIND)
        place(x, np.sin(2 * np.pi * np.cumsum(f) / SR) * ramp * (0.7 + 0.3 * np.sin(2 * np.pi * 26 * t)), 0, g)
    place(x, svf(b.noise(WIND), np.linspace(3000, 8000, len(t)), 1.5) * ramp, 0, 0.25)
    place(x, high(b.noise(0.004), 4000) * exp_env(0.004, 0.0008, 0.0001), WIND, 0.8)
    place(x, modes([2637, 3966, 5290, 6650, 7930], [6, 8, 10, 13, 17], 0.75, [1.0, 0.55, 0.45, 0.3, 0.2]), WIND, 0.7)
    place(x, modes([2960, 4441, 5925], [7, 10, 14], 0.6, [0.8, 0.45, 0.3]), WIND + 0.06, 0.45)
    place(x, crackles(b, 0.6, 0.006, 3000, 9500, 0.2), WIND, 0.45)
    place(x, svf(b.noise(0.6), 7000, 0.8, "high") * exp_env(0.6, 0.25, 0.02), WIND, 0.12)
    place(x, thud(300, 160, 0.08, 0.02), WIND, 0.35)
    return fade_out(tail(x, 0.3, 0.2, b), 0.12)


def impact_frost(b):
    """Frost breaking: a crack, a knock of ice, and it shatters, shards
    tinkling down and a crunch under them."""
    d = 0.72
    x = canvas_of(d)
    place(x, low(high(b.noise(0.004), 3000), 9000) * exp_env(0.004, 0.0012, 0.0001), 0, 0.55)
    place(x, modes([1830, 3100, 4570], [55, 70, 90], 0.15, [1.0, 0.6, 0.35]), 0, 0.5)
    place(x, thud(270, 120, 0.12, 0.025), 0, 0.75)
    at = 0.004
    for i in range(22):
        f = b.r.uniform(2400, 8200)
        place(x, modes([f, f * 2.31, f * 3.7], [b.r.uniform(30, 70), 90, 130], 0.25, [1.0, 0.35, 0.15]), at, 0.42 * (1 - i / 26))
        at += b.r.exponential(0.014) + 0.002
    t = t_of(0.12)
    grit = band(b.noise(0.12), 1500, 6500) * (b.r.random(len(t)) > 0.6) * exp_env(0.12, 0.035, 0.0005)
    place(x, grit, 0.002, 0.5)
    return fade_out(tail(x, 0.22, 0.15, b), 0.08)


def cast_holy(b):
    """Holy light: a soft choir swelling, then a bell, and the light
    blooming on after it, a few motes chiming in it."""
    d = 1.15
    x = canvas_of(d)
    choir = pad([440.0, 554.37, 659.25, 880.0], 0.95, [(0, 0), (0.16, 0.55), (0.3, 0.7), (0.65, 0.45), (1, 0)])
    place(x, low(choir, 3000), 0, 0.3)
    place(x, svf(b.noise(0.95), np.linspace(900, 2200, int(SR * 0.95)), 3.0) * shape(int(SR * 0.95), [(0, 0), (0.16, 0.6), (1, 0)]), 0, 0.12)
    prime = 660.0
    ratios = [0.5, 1.0, 1.2, 1.5, 2.0, 3.0, 4.0]
    place(x, modes([prime * r for r in ratios], [2.2, 2.7, 3.2, 3.6, 4.2, 6.0, 8.0], 1.0, [0.5, 0.7, 0.45, 0.35, 0.8, 0.3, 0.18]), WIND, 0.6)
    place(x, high(b.noise(0.005), 3000) * exp_env(0.005, 0.0012, 0.0001), WIND, 0.35)
    bloom = pad([880.0, 1108.73, 1318.5, 1760.0], 0.85, [(0, 0), (0.28, 1), (0.6, 0.6), (1, 0)], vib=5.5)
    place(x, bloom, WIND + 0.02, 0.18)
    sparkle(b, x, WIND + 0.08, d - 0.25, 9, 2600, 6200, 0.18, (0.03, 0.08), 1.4)
    return fade_out(tail(x, 0.45, 0.25, b), 0.18)


def impact_holy(b):
    """Holy light breaking: a radiant shwing, a bright chime, a soft
    thump, the light lingering."""
    d = 0.78
    x = canvas_of(d)
    place(x, svf(b.noise(0.06), np.linspace(2000, 8000, int(SR * 0.06)), 1.8) * exp_env(0.06, 0.02, 0.003), 0, 0.5)
    place(x, modes([1320, 1980, 2640, 3960, 5280], [6, 8, 11, 15, 20], 0.7, [1.0, 0.6, 0.5, 0.3, 0.15]), 0.002, 0.75)
    place(x, thud(210, 90, 0.16, 0.04), 0, 0.75)
    place(x, pad([1760.0, 2637.0], 0.5, [(0, 0), (0.15, 1), (1, 0)], vib=6), 0.02, 0.15)
    sparkle(b, x, 0.05, 0.6, 8, 3000, 7000, 0.18, (0.02, 0.06), 1.3)
    return fade_out(tail(x, 0.3, 0.2, b), 0.12)


def cast_storm(b):
    """Lightning: a charge buzzing up, the snap as it goes, a zap and a
    burst of static, the rumble rolling after."""
    d = 0.9
    x = canvas_of(d)
    t = t_of(WIND)
    f = 90 * 2 ** (t / WIND)
    buzz = np.sign(np.sin(2 * np.pi * np.cumsum(f) / SR)) * (0.6 + 0.4 * (b.r.random(len(t)) > 0.5))
    place(x, low(buzz, 3200) * (t / WIND) ** 2, 0, 0.35)
    place(x, crackles(b, WIND, 0.004, 2000, 8000, 1.0) * (t / WIND), 0, 0.35)
    place(x, low(b.noise(0.003), 9000) * exp_env(0.003, 0.0006, 0.00005), WIND, 0.9)
    place(x, high(b.noise(0.02), 2000) * exp_env(0.02, 0.005, 0.0002), WIND, 0.8)
    place(x, sweep(3600, 280, 0.09) * exp_env(0.09, 0.03, 0.0005), WIND, 0.45)
    place(x, crackles(b, 0.25, 0.012, 2000, 9000, 0.06), WIND, 0.55)
    m = 0.72
    tt = t_of(m)
    rumble = low(b.noise(m), 170) * (0.65 + 0.35 * np.sin(2 * np.pi * 4.2 * tt)) * shape(len(tt), [(0, 0), (0.12, 1), (0.45, 0.6), (1, 0)])
    place(x, rumble, WIND + 0.01, 1.6)
    place(x, thud(82, 34, 0.6, 0.2), WIND, 0.55)
    return fade_out(tail(x, 0.3, 0.15, b), 0.12)


def impact_storm(b):
    """Lightning striking (and a Thunder Scroll's thunder): a crack like a
    whip, static spitting, then the thunder rolling away."""
    d = 1.25
    x = canvas_of(d)
    place(x, low(b.noise(0.004), 9000) * exp_env(0.004, 0.0009, 0.00005), 0, 0.9)
    place(x, low(high(b.noise(0.03), 1500), 9000) * exp_env(0.03, 0.009, 0.0002), 0, 0.85)
    place(x, crackles(b, 0.16, 0.02, 1800, 9000, 0.05), 0.002, 0.6)
    place(x, sweep(2600, 200, 0.1) * exp_env(0.1, 0.035, 0.0005), 0, 0.3)
    m = 1.15
    t = t_of(m)
    roll = 0.6 + 0.4 * np.sin(2 * np.pi * 3.3 * t + 0.6 * np.sin(2 * np.pi * 1.1 * t))
    place(x, low(b.noise(m), 140) * roll * shape(len(t), [(0, 0), (0.035, 1), (0.35, 0.7), (1, 0)]), 0.01, 1.9)
    place(x, band(b.noise(0.9), 90, 800) * roll[:int(SR * 0.9)] * exp_env(0.9, 0.3, 0.004), 0.005, 0.8)
    place(x, drive(thud(76, 30, 0.85, 0.26), 1.5), 0.002, 0.9)
    return fade_out(tail(x, 0.4, 0.2, b), 0.2)


def cast_poison(b):
    """Poison and acid: a hiss drawn up, a wet glorp as it goes, bubbles
    rising and popping in a sizzle."""
    d = 0.8
    x = canvas_of(d)
    n = int(SR * WIND)
    ramp = np.linspace(0, 1, n) ** 2
    place(x, band(b.noise(WIND), 2500, 7500) * ramp, 0, 0.3)
    place(x, svf(b.noise(WIND), np.linspace(200, 600, n), 2.5) * ramp, 0, 0.3)
    g = 0.1
    t = t_of(g)
    glorp = svf(b.noise(g) * 0.4 + np.sin(2 * np.pi * np.cumsum(180 * 3 ** (t / g)) / SR), 180 * 3 ** (t / g) * 1.2, 2.0)
    place(x, glorp * shape(len(t), [(0, 0), (0.2, 1), (1, 0)]), WIND, 0.85)
    place(x, low(b.noise(0.08), 800) * exp_env(0.08, 0.03, 0.002), WIND, 0.55)
    for _ in range(15):
        at = WIND + 0.03 + b.r.random() ** 1.3 * 0.5
        f0 = b.r.uniform(320, 1150)
        length = b.r.uniform(0.035, 0.07)
        tb = t_of(length)
        blip = np.sin(2 * np.pi * np.cumsum(f0 * (1 + 2.5 * tb / length)) / SR) * np.exp(-tb / (length * 0.35))
        place(x, blip, at, b.r.uniform(0.25, 0.55))
    hiss = band(b.noise(0.6), 3000, 9000) * (b.r.random(int(SR * 0.6)) > 0.5) * shape(int(SR * 0.6), [(0, 0), (0.15, 1), (1, 0)])
    place(x, hiss, WIND, 0.2)
    return fade_out(tail(x, 0.15, 0.1, b), 0.08)


def impact_poison(b):
    """Poison breaking: a wet splat, the acid sizzling where it lands,
    bubbles popping in it."""
    d = 0.7
    x = canvas_of(d)
    place(x, low(b.noise(0.08), 2200) * exp_env(0.08, 0.025, 0.0005), 0, 1.0)
    place(x, band(b.noise(0.04), 600, 3500) * exp_env(0.04, 0.01, 0.0003), 0, 0.6)
    place(x, thud(135, 55, 0.22, 0.06), 0, 0.95)
    sizzle = band(b.noise(0.6), 3000, 9500) * (b.r.random(int(SR * 0.6)) > 0.45) * shape(int(SR * 0.6), [(0, 0), (0.06, 1), (1, 0)])
    place(x, sizzle, 0.02, 0.3)
    for _ in range(10):
        at = 0.05 + b.r.random() ** 1.4 * 0.48
        f0 = b.r.uniform(350, 1200)
        length = b.r.uniform(0.03, 0.06)
        tb = t_of(length)
        blip = np.sin(2 * np.pi * np.cumsum(f0 * (1 + 2.5 * tb / length)) / SR) * np.exp(-tb / (length * 0.35))
        place(x, blip, at, b.r.uniform(0.2, 0.45))
    return fade_out(tail(x, 0.15, 0.1, b), 0.08)


def cast_arcane(b):
    """Arcane: a glassy sweep climbing, then a deep "vwom" as it goes and
    a shimmering chord with sparkles off it."""
    d = 0.8
    x = canvas_of(d)
    t = t_of(WIND)
    fc = 420 * 4 ** (t / WIND)
    mod = np.sin(2 * np.pi * np.cumsum(fc * 1.41) / SR) * (1.5 * t / WIND)
    place(x, np.sin(2 * np.pi * np.cumsum(fc) / SR + mod) * (t / WIND) ** 2, 0, 0.45)
    place(x, svf(b.noise(WIND), np.linspace(600, 3600, len(t)), 1.4) * (t / WIND) ** 2, 0, 0.3)
    place(x, sweep(115, 52, 0.45) * exp_env(0.45, 0.13, 0.004), WIND, 0.85)
    m = 0.62
    tm = t_of(m)
    shimmer = pad([523.25, 783.99, 1046.5, 1567.98], m, [(0, 0), (0.015, 1), (0.3, 0.5), (1, 0)], vib=6.5, depth=0.003, cents=7)
    shimmer *= 0.6 + 0.4 * np.sin(2 * np.pi * 14 * tm)
    place(x, shimmer, WIND, 0.35)
    tg = t_of(0.2)
    glint = np.sin(2 * np.pi * 2093 * tg + 1.2 * np.sin(2 * np.pi * 2093 * 1.41 * tg)) * np.exp(-tg / 0.05)
    place(x, glint, WIND, 0.4)
    sparkle(b, x, WIND + 0.03, d - 0.2, 10, 2500, 7500, 0.2, (0.012, 0.04), 1.3)
    return fade_out(tail(x, 0.3, 0.2, b), 0.1)


def impact_arcane(b):
    """Arcane breaking: a hit, a phasing zap falling away, a glassy knock,
    sparkles left in the air."""
    d = 0.66
    x = canvas_of(d)
    place(x, high(b.noise(0.004), 3000) * exp_env(0.004, 0.0012, 0.0001), 0, 0.8)
    m = 0.26
    t = t_of(m)
    f = 1800 * (0.13 ** (t / m))
    zap = np.sin(2 * np.pi * np.cumsum(f) / SR + 1.4 * np.sin(2 * np.pi * np.cumsum(f * 0.5) / SR)) * exp_env(m, 0.08, 0.0005)
    place(x, zap, 0, 0.75)
    place(x, modes([1567, 2350, 3135], [14, 18, 24], 0.35, [1.0, 0.6, 0.35]), 0.002, 0.45)
    place(x, thud(165, 70, 0.2, 0.05), 0, 0.8)
    sparkle(b, x, 0.04, 0.5, 8, 2500, 7500, 0.2, (0.012, 0.035), 1.3)
    return fade_out(tail(x, 0.25, 0.18, b), 0.08)


def cast_earth(b):
    """Earth: stone grinding up out of the floor, then the crunch of rock
    as it goes, the thud under it, pebbles skittering."""
    d = 0.82
    x = canvas_of(d)
    n = int(SR * WIND)
    ramp = np.linspace(0, 1, n) ** 1.5
    grain = np.repeat(b.r.random(n // 220 + 1) > 0.45, 220)[:n].astype(float)
    place(x, low(b.noise(WIND), 260) * (0.5 + 0.5 * grain) * ramp, 0, 1.0)
    place(x, sweep(40, 72, WIND) * ramp, 0, 0.35)
    place(x, crackles(b, WIND, 0.003, 800, 4000, 1.0) * ramp, 0, 0.45)
    at = WIND
    for i in range(10):
        crack = band(b.noise(0.03), b.r.uniform(300, 900), b.r.uniform(2500, 5000)) * exp_env(0.03, b.r.uniform(0.004, 0.012), 0.0003)
        place(x, crack, at, b.r.uniform(0.5, 0.9) * (1 - i * 0.05))
        at += b.r.uniform(0.004, 0.012)
    place(x, drive(thud(130, 46, 0.4, 0.09), 1.8), WIND, 1.1)
    place(x, band(b.noise(0.16), 150, 900) * exp_env(0.16, 0.06, 0.002), WIND, 1.2)
    at = WIND + 0.1
    while at < d - 0.15:
        f = b.r.uniform(900, 2600)
        place(x, modes([f, f * 1.8], [120, 160], 0.06), at, 0.3 * (1 - (at - WIND) / (d - WIND)))
        at += b.r.exponential(0.05) + 0.012
    return fade_out(tail(x, 0.25, 0.15, b), 0.08)


def impact_earth(b):
    """Earth breaking: rock cracking on rock, a deep boom, the crunch, and
    the debris pattering down in dust."""
    d = 0.8
    x = canvas_of(d)
    place(x, high(b.noise(0.006), 2000) * exp_env(0.006, 0.0018, 0.0002), 0, 1.0)
    place(x, band(b.noise(0.06), 200, 3500) * exp_env(0.06, 0.02, 0.0004), 0, 1.0)
    place(x, drive(thud(110, 38, 0.5, 0.12), 1.8), 0, 1.2)
    place(x, band(b.noise(0.2), 140, 900) * exp_env(0.2, 0.07, 0.001), 0, 1.2)
    at = 0.002
    for i in range(8):
        crack = band(b.noise(0.03), b.r.uniform(400, 1000), b.r.uniform(3000, 5500)) * exp_env(0.03, b.r.uniform(0.004, 0.01), 0.0003)
        place(x, crack, at, 0.7 * (1 - i * 0.07))
        at += b.r.uniform(0.004, 0.01)
    at = 0.09
    while at < 0.68:
        f = b.r.uniform(800, 2800)
        place(x, modes([f, f * 1.9], [110, 150], 0.06), at, 0.32 * (1 - at / 0.75))
        if b.r.random() < 0.4:
            place(x, thud(b.r.uniform(240, 400), 150, 0.04, 0.01), at, 0.2)
        at += b.r.exponential(0.035) + 0.01
    place(x, band(b.noise(0.6), 300, 2000) * exp_env(0.6, 0.22, 0.01), 0.02, 0.16)
    return fade_out(tail(x, 0.25, 0.15, b), 0.08)


# ─── the wish moments (`altar`, `overclass::grant`, `nooks::gnome`) ──────────

def lamp_rub(b):
    """The lamp rubbed: three strokes of a palm on brass, a breath drawn
    in rising through it, and a magic climbing to meet the genie (who
    comes half a second on, `genie_appear`)."""
    d = 0.78
    x = canvas_of(d)
    for i, at in enumerate([0.0, 0.15, 0.3]):
        s = 0.15
        t = t_of(s)
        stroke = band(b.noise(s), 900, 3600) * np.sin(np.pi * t / s) ** 2
        squeak = np.sin(2 * np.pi * np.cumsum(1900 + 260 * np.sin(np.pi * t / s) + 40 * i) / SR) * np.sin(np.pi * t / s) ** 4
        place(x, stroke, at, 0.4 - 0.05 * i)
        place(x, squeak, at, 0.07)
    n = 0.68
    t = t_of(n)
    env = shape(len(t), [(0, 0), (0.45, 0.55), (0.75, 1), (1, 0)])
    breath = svf(b.noise(n), 400 * 4.5 ** (t / n), 2.2) + 0.7 * svf(b.noise(n), 1100 * 2.8 ** (t / n), 3.0)
    place(x, breath * env, 0.06, 0.7)
    rise = sum(np.sin(2 * np.pi * np.cumsum(f * 2 ** (t / n)) / SR) / (1 + i) for i, f in enumerate([440.0, 659.25, 880.0]))
    place(x, rise * env * (0.75 + 0.25 * np.sin(2 * np.pi * 11 * t)), 0.06, 0.25)
    sparkle(b, x, 0.25, 0.7, 10, 3000, 7500, 0.16, (0.01, 0.03), 0.6)
    return fade_out(tail(x, 0.2, 0.15, b), 0.1)


def genie_appear(b):
    """The genie gathers out of the smoke: a soft poof and a whoom, a
    shimmering chord, sparkles running up it, the smoke swirling."""
    d = 1.3
    x = canvas_of(d)
    place(x, low(b.noise(0.4), 1200) * shape(int(SR * 0.4), [(0, 0), (0.08, 1), (1, 0)]) ** 2, 0, 0.75)
    place(x, sweep(92, 52, 0.55) * shape(int(SR * 0.55), [(0, 0), (0.1, 1), (1, 0)]), 0, 0.6)
    t = t_of(1.1)
    chord = pad([587.33, 880.0, 1174.66, 1479.98, 1760.0], 1.1, [(0, 0), (0.11, 1), (0.45, 0.6), (1, 0)], vib=6, depth=0.003, cents=8)
    place(x, chord * (0.7 + 0.3 * np.sin(2 * np.pi * 9 * t)), 0.02, 0.32)
    for i, f in enumerate([1174.66, 1479.98, 1760.0, 2349.32, 2959.96, 3520.0]):
        place(x, chime(f, 0.5, 9), 0.1 + i * 0.06, 0.32 - i * 0.02)
    m = 1.0
    tm = t_of(m)
    swirl = svf(b.noise(m), 800 + 2200 * np.sin(np.pi * tm / m), 1.5) * shape(len(tm), [(0, 0), (0.2, 1), (1, 0)])
    place(x, swirl, 0.04, 0.2)
    sparkle(b, x, 0.12, 1.0, 14, 3500, 8000, 0.15, (0.01, 0.04), 1.2)
    return fade_out(tail(x, 0.45, 0.25, b), 0.2)


D_MAJOR = [587.33, 739.99, 880.0, 1174.66]


def _wish_land(b, d, chord, lows, boom, twinkles, toll=False, climb=False):
    """A wish landing on its knight: light drawn in for a fifth of a
    second (`magic::landed`'s wind), then the burst, a sparkling chord
    swelling and ringing on, the sparkles rising off them. A higher tier
    adds its lower voices, a boom, a bell, a climb."""
    x = canvas_of(d)
    w = 0.2
    t = t_of(w)
    ramp = (t / w) ** 2
    place(x, svf(b.noise(w), 500 * 8 ** (t / w), 1.2) * ramp, 0, 0.5)
    place(x, np.sin(2 * np.pi * np.cumsum(440 * 2 ** (t / w)) / SR) * ramp, 0, 0.15)
    place(x, high(b.noise(0.005), 3000) * exp_env(0.005, 0.0012, 0.0001), w, 0.5)
    for i, f in enumerate(chord):
        place(x, chime(f, d - w, 4.5 + i * 0.6), w + i * 0.012, 0.32)
    body = d - w
    place(x, pad(chord + lows, body, [(0, 0), (0.08, 1), (0.5, 0.55), (1, 0)], vib=5.5, cents=6), w, 0.2)
    tb = t_of(body)
    glitter = band(b.noise(body), 6000, 11000) * (0.5 + 0.5 * np.sin(2 * np.pi * 13 * tb)) * shape(len(tb), [(0, 0), (0.05, 1), (1, 0)])
    place(x, glitter, w, 0.07)
    if boom:
        place(x, drive(thud(95, 38, 0.7, 0.2), 1.4), w, boom)
    else:
        place(x, thud(150, 70, 0.25, 0.07), w, 0.5)
    if toll:
        place(x, modes([587.33 * r for r in (0.5, 1.0, 1.2, 1.5, 2.0, 3.0)], [1.8, 2.3, 2.8, 3.2, 4.0, 6.0], body,
                       [0.5, 0.7, 0.4, 0.3, 0.7, 0.25]), w, 0.4)
    if climb:
        for i, f in enumerate([1174.66, 1479.98, 1760.0, 2349.32, 2959.96, 3520.0, 4698.64]):
            place(x, chime(f, 0.5, 10), w + 0.18 + i * 0.05, 0.22)
    sparkle(b, x, w + 0.03, d - 0.25, twinkles, 2500, 8500, 0.17, (0.012, 0.05), 1.3)
    return fade_out(tail(x, 0.5, 0.25, b), 0.25)


def wish_land(b):
    """A wish lands, tiers I and II: a sparkling D major chord swells."""
    return _wish_land(b, 1.2, D_MAJOR, [293.66], 0.0, 12)


def wish_land_grand(b):
    """Tiers III and IV: the chord with its lower voices, a boom under it,
    sparkles climbing off it."""
    return _wish_land(b, 1.4, D_MAJOR + [1479.98], [146.83, 293.66, 440.0], 1.0, 20, climb=True)


def wish_land_mythic(b):
    """Tier V: all of it, a bell tolling on top and a deeper boom."""
    return _wish_land(b, 1.42, D_MAJOR + [1479.98, 1760.0], [73.42, 146.83, 293.66, 440.0], 1.4, 28, toll=True, climb=True)


def fairy_wish(b):
    """The fairies pay: a run up through fairy dust, then giggles in
    chimes (quick bursts of high notes, each bent down at its end, the way
    a laugh goes "hee-hee-hee"), tinkling dust all through."""
    d = 1.42
    x = canvas_of(d)
    penta = [2637.02, 2959.96, 3520.0, 3951.07, 4698.64]
    for i, f in enumerate([1318.51, 1567.98, 1760.0, 2093.0, 2349.32, 2637.02, 3135.96]):
        place(x, chime(f, 0.4, 12), i * 0.03, 0.22)
    for start, scale, notes in [(0.24, 1.0, [4, 3, 2, 3, 1]), (0.62, 0.84, [3, 2, 1, 2, 0]), (1.0, 1.12, [4, 2, 3, 1])]:
        for j, k in enumerate(notes):
            f = penta[k] * scale
            n = 0.16
            t = t_of(n)
            bend = f * (1 - 0.12 * np.clip((t - 0.035) / 0.05, 0, 1))
            note = np.sin(2 * np.pi * np.cumsum(bend) / SR) + 0.2 * np.sin(4 * np.pi * np.cumsum(bend) / SR)
            note *= np.exp(-t / 0.05) * shape(len(t), [(0, 0), (0.02, 1), (1, 1)])
            place(x, note, start + j * 0.068 + b.r.uniform(-0.006, 0.006), 0.3 * (1 - 0.08 * j))
    sparkle(b, x, 0.0, d - 0.2, 46, 4200, 9800, 0.14, (0.006, 0.02), 1.1)
    shimmer = band(b.noise(d), 6500, 11500) * (0.5 + 0.5 * np.sin(2 * np.pi * 17 * t_of(d))) * shape(int(SR * d), [(0, 0), (0.1, 1), (0.8, 0.6), (1, 0)])
    place(x, shimmer, 0, 0.05)
    return fade_out(tail(x, 0.4, 0.25, b), 0.2)


WEAPON_MAGIC = [
    swing_spear, swing_sword, swing_axe, swing_cleaver, swing_maul,
    hit_flesh_light, hit_flesh_medium, hit_flesh_heavy,
    gore_behead, gore_head_thud, gore_bisect, gore_slump, parry, parry_whizz,
    cast_fire, impact_fire, cast_frost, impact_frost, cast_holy, impact_holy, cast_storm, impact_storm,
    cast_poison, impact_poison, cast_arcane, impact_arcane, cast_earth, impact_earth,
    lamp_rub, genie_appear, wish_land, wish_land_grand, wish_land_mythic, fairy_wish,
]


# ─── the steps up (`rules::rewards`) ─────────────────────────────────────────
# A level, a rung on the resolution ladder, a relic, a floor deeper than the
# realm has stood on, a constellation of the realm's sky lit: each lands with
# its own short fanfare. A tier climbed has the wish's own landing.

def _brass(f, dur, bright=0.6):
    """A short brass note: its harmonics swelling in and held, driven warm."""
    t = t_of(dur)
    out = np.zeros_like(t)
    for k in range(1, 9):
        out += np.sin(2 * np.pi * f * k * t * (1 + 0.003 * np.sin(2 * np.pi * 5.2 * t))) * k ** -1.1 * np.exp(-(k - 1) * (1 - bright) * 0.6)
    return drive(out * shape(len(t), [(0, 0), (0.06, 1), (0.7, 0.8), (1, 0)]) * 0.5, 1.3)


def level_up(b):
    """A level reached: a bright fanfare climbing G major, G B D G, its last
    note held under a shimmer of sparkles."""
    d = 1.15
    x = canvas_of(d)
    for i, f in enumerate([392.0, 493.88, 587.33, 783.99]):
        dur = 0.16 if i < 3 else 0.75
        place(x, _brass(f, dur), i * 0.09, 0.3)
        place(x, chime(f * 2, dur + 0.2, 2.5), i * 0.09, 0.12)
    place(x, pad([392.0, 587.33, 783.99], 0.8, [(0, 0), (0.1, 1), (1, 0)]), 0.27, 0.08)
    sparkle(b, x, 0.3, d - 0.2, 18, 3000, 9000, 0.12, (0.01, 0.04), 1.2)
    return fade_out(tail(x, 0.45, 0.22, b), 0.2)


def rung_up(b):
    """A rung climbed: a lens finding its focus, a click and a glassy run
    upward, the last note ringing clear."""
    d = 1.0
    x = canvas_of(d)
    place(x, high(b.noise(0.004), 4000) * exp_env(0.004, 0.001, 0.0001), 0.0, 0.4)
    for i, f in enumerate([880.0, 1108.73, 1318.51, 1760.0, 2217.46, 2637.02]):
        place(x, chime(f, 0.5 if i < 5 else 0.9, 6.0 if i < 5 else 2.2), 0.05 + i * 0.055, 0.22)
    t = t_of(0.6)
    rise = np.sin(2 * np.pi * np.cumsum(1200 * 2 ** (t / 0.6 * 1.5)) / SR) * shape(len(t), [(0, 0), (0.2, 1), (1, 0)])
    place(x, rise, 0.05, 0.05)
    return fade_out(tail(x, 0.4, 0.25, b), 0.2)


def relic_find(b):
    """A relic taken: light coming down, a bell struck deep, a choir's chord
    opening under a fall of sparkles."""
    d = 1.4
    x = canvas_of(d)
    place(x, modes([196.0 * r for r in (1.0, 2.0, 2.4, 3.0, 4.2)], [2.0, 1.6, 1.2, 0.9, 0.6], d, [0.8, 0.5, 0.35, 0.25, 0.12]), 0.0, 0.35)
    place(x, pad([392.0, 493.88, 587.33, 783.99], d - 0.1, [(0, 0), (0.15, 1), (0.6, 0.6), (1, 0)], vib=4.0, cents=7), 0.05, 0.12)
    sparkle(b, x, 0.02, d - 0.3, 26, 2500, 9000, 0.13, (0.01, 0.05), 0.8)
    return fade_out(tail(x, 0.6, 0.3, b), 0.25)


def deepest_yet(b):
    """A floor deeper than the realm has stood on: a boom far down and a
    horn calling twice, a fourth apart, into the dark."""
    d = 1.4
    x = canvas_of(d)
    place(x, fade_out(drive(thud(90, 40, 1.2, 0.25), 1.3), 0.2), 0.0, 0.5)
    place(x, _brass(196.0, 0.35, 0.4), 0.08, 0.34)
    place(x, _brass(261.63, 0.9, 0.45), 0.45, 0.36)
    return fade_out(tail(x, 0.7, 0.3, b), 0.3)


def sky_lit(b):
    """A constellation lit: its stars ringing in one after another, high
    and quick, then all of it together as its lines draw."""
    d = 1.4
    x = canvas_of(d)
    for i, f in enumerate([1318.51, 1567.98, 1760.0, 1975.53, 2349.32, 2637.02, 3135.96, 3520.0]):
        place(x, chime(f, 0.5, 5.0), i * 0.05, 0.16)
    for i, f in enumerate([659.25, 987.77, 1318.51, 1975.53]):
        place(x, chime(f, d - 0.45, 1.8), 0.45 + i * 0.01, 0.2)
    place(x, pad([659.25, 987.77, 1318.51], d - 0.45, [(0, 0), (0.1, 1), (1, 0)], vib=3.0), 0.45, 0.06)
    sparkle(b, x, 0.45, d - 0.2, 18, 4000, 10000, 0.1, (0.01, 0.04), 1.4)
    return fade_out(tail(x, 0.5, 0.3, b), 0.25)


STEPS_UP = [level_up, rung_up, relic_find, deepest_yet, sky_lit]


EFFECTS = {fn.__name__: fn for fn in [
    shot_arcane, shot_spark,
    knight_hit, knight_hit_big, hit_blade, hit_axe, hit_arrow, hit_bolt, hit_blast, hit_magic,
    step_stone, step_gravel, step_ash, step_wet, step_moss, step_wood, door_pass,
    gold, potion, holy_water, bomb_toss, shield_up, axe_spin, helmet_pop, armour_fall,
    engine_whistle, boss_roar, boss_rage, boss_death, knight_fall, bones_clatter,
    crack_open, hand_rise, hand_grab, hand_drag, ground_close,
    hell_gate_rise, hell_gate_enter, circle_toll,
    gore_kick, gore_thunk, gore_chop, gore_splat, gore_bones, gore_burst,
    hit_note,
    *WEAPON_MAGIC,
    *STEPS_UP,
]}


# ─── levelling and encoding ──────────────────────────────────────────────────

def level(x, rms_db):
    """Body to `rms_db` over the whole file (as level.py measures it), then
    peaks limited to -2 dBFS with a soft knee."""
    x = x - np.mean(x)
    x = x / (np.sqrt(np.mean(x ** 2)) + 1e-12) * 10 ** (rms_db / 20)
    over = np.abs(x) > PEAK * 0.7
    if over.any():
        x = np.sign(x) * np.where(np.abs(x) > PEAK * 0.7,
                                  PEAK * 0.7 + (PEAK * 0.3) * np.tanh((np.abs(x) - PEAK * 0.7) / (PEAK * 0.3)),
                                  np.abs(x))
    # Limiting leaves the body a touch lower than asked: one correction pass.
    rms = np.sqrt(np.mean(x ** 2))
    x = x * 10 ** (rms_db / 20) / rms
    return np.clip(x, -PEAK, PEAK)


def render(name, trim_db=0.0):
    b = Bench(name)
    x = EFFECTS[name](b)
    x = np.nan_to_num(x)
    target = (STEP_DB if name.startswith("step_") else HAT_DB if name == "hit_note"
              else BODY_DB + LEVEL.get(name, 0.0))
    x = level(x, target + trim_db)
    path = OUT / f"{name}.mp3"
    with tempfile.TemporaryDirectory() as tmp:
        wav = pathlib.Path(tmp) / f"{name}.wav"
        with wave.open(str(wav), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes((x * 32767).astype(np.int16).tobytes())
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(wav), "-ac", "1", "-ar", str(SR),
                        "-b:a", "96k", str(path)], check=True)
    return path


def stats(path):
    out = subprocess.run(["ffmpeg", "-hide_banner", "-i", str(path), "-af", "astats", "-f", "null", "-"],
                         capture_output=True, text=True).stderr
    import re
    dur = re.search(r"Duration: (\d+):(\d+):([\d.]+)", out)
    seconds = int(dur.group(1)) * 3600 + int(dur.group(2)) * 60 + float(dur.group(3)) if dur else 0.0
    peak = re.findall(r"Peak level dB: (-?[\d.]+)", out)
    rms = re.findall(r"RMS level dB: (-?[\d.]+)", out)
    return seconds, float(peak[-1]) if peak else None, float(rms[-1]) if rms else None


def true_peak(path):
    """The decoded file's true peak (dBTP, ffmpeg ebur128): a sharp click
    can overshoot between the samples where the sample peak does not."""
    out = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", str(path), "-af", "ebur128=peak=true", "-f", "null", "-"],
                         capture_output=True, text=True).stderr
    import re
    tp = re.findall(r"Peak:\s+(-?[\d.]+) dBFS", out)
    return float(tp[-1]) if tp else None


def main(argv):
    OUT.mkdir(exist_ok=True)
    if "--stats" in argv:
        names = [a for a in argv if not a.startswith("--")]
        for path in sorted(OUT.glob("*.mp3")):
            if not names or path.stem in names:
                s, p, r = stats(path)
                print(f"{path.stem:<18} {s:5.2f} s  peak {p:6.1f} dB  rms {r:6.1f} dB")
        return
    names = [a for a in argv if not a.startswith("--")] or list(EFFECTS)
    for name in names:
        path = render(name)
        s, p, r = stats(path)
        # The mp3 encode can overshoot a sharp click: pull the level down
        # until the decoded peak sits under -2 dBFS like the others.
        # The blades', the magic's and the wishes' (`LEVEL`) keep their true
        # peak a decibel under full scale too.
        trim = 0.0
        tp = true_peak(path) if name in LEVEL else None
        while p is not None and (p > -2.0 or (tp is not None and tp > -1.0)) and trim > -6.0:
            trim -= 0.7
            path = render(name, trim)
            s, p, r = stats(path)
            tp = true_peak(path) if name in LEVEL else None
        print(f"{name:<18} {s:5.2f} s  peak {p:6.1f} dB  rms {r:6.1f} dB")


if __name__ == "__main__":
    main(sys.argv[1:])
