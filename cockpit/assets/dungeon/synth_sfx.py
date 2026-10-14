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


EFFECTS = {fn.__name__: fn for fn in [
    shot_arcane, shot_spark,
    knight_hit, knight_hit_big, hit_blade, hit_axe, hit_arrow, hit_bolt, hit_blast, hit_magic,
    step_stone, step_gravel, step_ash, step_wet, step_moss, step_wood, door_pass,
    gold, potion, holy_water, bomb_toss, shield_up, axe_spin, helmet_pop, armour_fall,
    engine_whistle, boss_roar, boss_rage, boss_death, knight_fall, bones_clatter,
    crack_open, hand_rise, hand_grab, hand_drag, ground_close,
    hell_gate_rise, hell_gate_enter, circle_toll,
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
    target = STEP_DB if name.startswith("step_") else BODY_DB
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
        trim = 0.0
        while p is not None and p > -2.0 and trim > -6.0:
            trim -= 0.7
            path = render(name, trim)
            s, p, r = stats(path)
        print(f"{name:<18} {s:5.2f} s  peak {p:6.1f} dB  rms {r:6.1f} dB")


if __name__ == "__main__":
    main(sys.argv[1:])
