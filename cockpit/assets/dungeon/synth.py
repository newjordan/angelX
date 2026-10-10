"""Synthesized Delve sounds: Fortune's wheel spinning down, and its landing bell."""
import numpy as np, wave, sys
SR = 44100
rng = np.random.default_rng(7)

def click(f0, dur=0.03, bright=1.0):
    t = np.arange(int(SR * dur)) / SR
    env = np.exp(-t * 180)
    tone = np.sin(2 * np.pi * f0 * t) * 0.6 + np.sin(2 * np.pi * f0 * 2.7 * t) * 0.25 * bright
    noise = rng.standard_normal(len(t)) * np.exp(-t * 600) * 0.5
    return (tone + noise) * env

def spin(T=4.0, crossings=28):
    out = np.zeros(int(SR * (T + 0.3)))
    for k in range(1, crossings + 1):
        tk = T * (1 - (1 - k / crossings) ** (1 / 3))
        c = click(1700 + rng.uniform(-60, 60), bright=0.8 + 0.4 * (1 - k / crossings))
        i = int(tk * SR)
        out[i:i + len(c)] += c[: len(out) - i]
    # The hub's low rumble, slowing with the wheel.
    t = np.arange(len(out)) / SR
    speed = np.clip(3 * (1 - t / T) ** 2, 0, None)
    rumble = np.sin(2 * np.pi * np.cumsum(40 + 30 * speed) / SR) * 0.08 * speed / 3
    return out + rumble

def bell(dur=1.4):
    t = np.arange(int(SR * dur)) / SR
    parts = [(880, 1.0, 3.0), (880 * 2.0, 0.5, 4.0), (880 * 2.76, 0.35, 5.5), (880 * 5.4, 0.2, 8.0), (1318.5, 0.6, 3.2)]
    out = sum(a * np.sin(2 * np.pi * f * t) * np.exp(-t * d) for f, a, d in parts)
    out[: int(SR * 0.004)] *= np.linspace(0, 1, int(SR * 0.004))
    return out

def level(x, rms_db=-20.0, peak_db=-2.0):
    rms = np.sqrt(np.mean(x[np.abs(x) > 1e-4] ** 2))
    x = x * (10 ** (rms_db / 20) / rms)
    peak = 10 ** (peak_db / 20)
    return np.tanh(x / peak) * peak

def save(name, x):
    x = level(x)
    with wave.open(name + ".wav", "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes((x * 32767).astype(np.int16).tobytes())

save("wheel_spin", spin())
save("wheel_land", bell())
