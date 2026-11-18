#!/usr/bin/env python3
import argparse
import base64
import json
import os
import select
import struct
import sys
import termios
import time
import tty
import zlib

WARMUP = 30

BLAST4 = [
    (11, 320, 200, -1, 40, 12, 2, 2),
    (12, 192, 160, 0, 24, 10, 4, 20),
    (13, 160, 160, 1, 20, 10, 6, 40),
    (14, 480, 140, 2, 60, 8, 12, 5),
]


def frame_bytes(w, h, n, seed):
    buf = bytearray(w * h * 4)
    for y in range(h):
        base = y * w * 4
        g = (y + n * 5) & 255
        for x in range(w):
            i = base + x * 4
            buf[i] = (x + n * 3 + seed) & 255
            buf[i + 1] = g
            buf[i + 2] = (x ^ y ^ n) & 255
            buf[i + 3] = 255
    return bytes(buf)


def png_bytes(w, h, rgba, level=1):
    stride = w * 4
    raw = bytearray((stride + 1) * h)
    for y in range(h):
        raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)] = rgba[y * stride:(y + 1) * stride]

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
            + chunk(b"IDAT", zlib.compress(bytes(raw), level)) + chunk(b"IEND", b""))


def percentiles(times):
    s = sorted(times)
    def at(p):
        return s[min(len(s) - 1, int(len(s) * p))] * 1000.0
    return {"p50_ms": round(at(0.50), 3), "p95_ms": round(at(0.95), 3),
            "worst_ms": round(s[-1] * 1000.0, 3), "frames": len(s)}


def apc(payload):
    return b"\x1b_G" + payload + b"\x1b\\"


class Term:
    def __init__(self):
        self.fd = sys.stdin.fileno()
        self.old = termios.tcgetattr(self.fd)
        tty.setraw(self.fd)
        self.out = sys.stdout.buffer

    def close(self):
        termios.tcsetattr(self.fd, termios.TCSADRAIN, self.old)

    def write(self, data):
        self.out.write(data)
        self.out.flush()

    def ack(self, marker=b"\x1b\\", timeout=5.0):
        buf = b""
        deadline = time.perf_counter() + timeout
        while marker not in buf:
            left = deadline - time.perf_counter()
            if left <= 0:
                raise TimeoutError("no ack from kitty")
            r, _, _ = select.select([self.fd], [], [], left)
            if not r:
                raise TimeoutError("no ack from kitty")
            buf += os.read(self.fd, 65536)
        return buf


def upload_b64(img_id, w, h, z, cols, rows, data, fmt=32):
    b64 = base64.b64encode(data)
    chunks = [b64[i:i + 4096] for i in range(0, len(b64), 4096)] or [b""]
    out = []
    for idx, chunk in enumerate(chunks):
        more = 1 if idx < len(chunks) - 1 else 0
        if idx == 0:
            head = f"a=T,f={fmt},s={w},v={h},i={img_id},c={cols},r={rows},z={z},q=2,m={more};".encode()
        else:
            head = f"m={more};".encode()
        out.append(apc(head + chunk))
    return b"".join(out)


class ShmStream:
    def __init__(self, data):
        from multiprocessing import shared_memory
        self.shm = shared_memory.SharedMemory(create=True, size=len(data), track=False)
        self.payload = base64.b64encode(self.shm.name.encode())

    def put(self, data):
        self.shm.buf[:len(data)] = data

    def close(self):
        self.shm.close()
        try:
            self.shm.unlink()
        except FileNotFoundError:
            pass

    def upload(self, img_id, w, h, z, cols, rows):
        head = f"a=T,f=32,t=s,s={w},v={h},i={img_id},c={cols},r={rows},z={z},q=2;".encode()
        return apc(head + self.payload)


def run_streams(term, streams, frames, transport):
    ring = {s[0]: [frame_bytes(s[1], s[2], v, s[0]) for v in range(4)] for s in streams}
    if transport == "png":
        ring = {s[0]: [png_bytes(s[1], s[2], f) for f in ring[s[0]]] for s in streams}
    payload = sum(len(ring[s[0]][0]) for s in streams)
    times = []
    stamp_log = None
    if os.environ.get("BENCH_STAMP_LOG"):
        stamp_log = open(os.environ["BENCH_STAMP_LOG"], "w", buffering=1)
    pace_fps = float(os.environ.get("BENCH_FPS", "0") or 0)
    pace_t0 = time.perf_counter()
    term.write(b"\x1b[2J")
    for n in range(frames + WARMUP):
        if pace_fps > 0:
            target = pace_t0 + n / pace_fps
            delay = target - time.perf_counter()
            if delay > 0:
                time.sleep(delay)
        if stamp_log is not None:
            stamp_log.write(f"{n} {time.time() * 1000.0:.1f}\n")
        t0 = time.perf_counter()
        shms = {}
        if transport == "shm":
            shms = {s[0]: ShmStream(ring[s[0]][n % len(ring[s[0]])]) for s in streams}
            for (img_id, *_rest) in streams:
                shms[img_id].put(ring[img_id][n % len(ring[img_id])])
            if stamp_log is not None and os.environ.get("BENCH_SOLID"):
                rgb = (n & 0xFFFFFF).to_bytes(3, "little")
                for s in streams:
                    size = s[1] * s[2] * 4
                    shms[s[0]].put(bytes([rgb[0], rgb[1], rgb[2], 255]) * (size // 4))
        parts = []
        for (img_id, w, h, z, cols, rows, row, col) in streams:
            parts.append(f"\x1b[{row};{col}H".encode())
            data = ring[img_id][n % len(ring[img_id])]
            if stamp_log is not None and transport != "png":
                rgb = (n & 0xFFFFFF).to_bytes(3, "little")
                if os.environ.get("BENCH_SOLID"):
                    data = bytes([rgb[0], rgb[1], rgb[2], 255]) * (w * h)
                else:
                    data = bytearray(data)
                    for y in range(min(16, h)):
                        for x in range(min(16, w)):
                            i = (y * w + x) * 4
                            data[i:i + 3] = rgb
                            data[i + 3] = 255
                    data = bytes(data)
            if transport == "shm":
                parts.append(shms[img_id].upload(img_id, w, h, z, cols, rows))
            elif transport == "png":
                parts.append(upload_b64(img_id, w, h, z, cols, rows, data, fmt=100))
            else:
                parts.append(upload_b64(img_id, w, h, z, cols, rows, data))
        parts.append(apc(b"a=q,i=99;"))
        term.write(b"".join(parts))
        if not os.environ.get("BENCH_NOACK") or n % 30 == 29:
            term.ack()
        dt = time.perf_counter() - t0
        for s in shms.values():
            s.close()
        if n >= WARMUP:
            times.append(dt)
    return times, payload


def run_placements(term, count, frames, cols=200, rows=50):
    img = frame_bytes(64, 64, 0, 21)
    term.write(upload_b64(21, 64, 64, 0, 4, 2, img) + apc(b"a=q,i=99;"))
    term.ack()
    times = []
    for n in range(frames + WARMUP):
        t0 = time.perf_counter()
        parts = []
        for i in range(count):
            col = 1 + (i * 4 + n) % max(1, cols - 4)
            row = 1 + ((i * 4 + n) // max(1, cols - 4)) % max(1, rows - 2)
            parts.append(f"\x1b[{row};{col}H".encode())
            parts.append(apc(f"a=p,i=21,p={i + 1},c=4,r=2,q=2;".encode()))
        parts.append(apc(b"a=q,i=99;"))
        term.write(b"".join(parts))
        term.ack()
        if n >= WARMUP:
            times.append(time.perf_counter() - t0)
    return times


POOL = 4096


def run_churn(term, per_frame, frames):
    img = frame_bytes(16, 16, 1, 7)
    times = []
    n = 0
    for frame in range(frames + WARMUP):
        t0 = time.perf_counter()
        parts = []
        for k in range(per_frame):
            img_id = 1000 + (n % POOL)
            n += 1
            col = 1 + (k % 50) * 2
            row = 1 + (k // 50)
            parts.append(f"\x1b[{row};{col}H".encode())
            parts.append(upload_b64(img_id, 16, 16, 0, 2, 1, img))
        parts.append(apc(b"a=q,i=99;"))
        term.write(b"".join(parts))
        term.ack()
        if frame >= WARMUP:
            times.append(time.perf_counter() - t0)
    return times, per_frame * 16 * 16 * 4


def run_text(term, cols, rows, frames):
    alphabet = "abcdefghij0123456789"
    times = []
    for n in range(frames + WARMUP):
        lines = []
        for r in range(rows):
            rot = (n + r) % len(alphabet)
            line = (alphabet[rot:] + alphabet[:rot]) * (cols // len(alphabet) + 1)
            lines.append(line[:cols])
        frame = "\x1b[H" + "\r\n".join(lines) + "\x1b[c"
        t0 = time.perf_counter()
        term.write(frame.encode())
        term.ack(marker=b"c")
        if n >= WARMUP:
            times.append(time.perf_counter() - t0)
    return times


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["blast4", "big", "placements", "text", "churn", "stamp"])
    ap.add_argument("--transport", choices=["b64", "png", "shm"], default="b64")
    ap.add_argument("--frames", type=int, default=300)
    ap.add_argument("--w", type=int, default=1920)
    ap.add_argument("--h", type=int, default=1080)
    ap.add_argument("--n", type=int, default=1000)
    ap.add_argument("--cols", type=int, default=0, help="0 = the window's real size")
    ap.add_argument("--rows", type=int, default=0)
    args = ap.parse_args()
    if args.cols <= 0 or args.rows <= 0:
        size = os.get_terminal_size()
        args.cols, args.rows = size.columns, size.lines

    term = Term()
    try:
        if args.mode == "blast4":
            times, payload = run_streams(term, BLAST4, args.frames, args.transport)
            result = {"mode": "blast4", "transport": args.transport,
                      "payload_bytes": payload, **percentiles(times)}
        elif args.mode == "big":
            streams = [(31, args.w, args.h, 0, 100, 30, 2, 2)]
            times, payload = run_streams(term, streams, args.frames, args.transport)
            result = {"mode": "big", "w": args.w, "h": args.h,
                      "transport": args.transport, "payload_bytes": payload,
                      **percentiles(times)}
        elif args.mode == "stamp":
            streams = [(41, 320, 200, 0, 40, 12, 2, 2)]
            times, payload = run_streams(term, streams, args.frames, args.transport)
            result = {"mode": "stamp", "transport": args.transport,
                      "payload_bytes": payload, **percentiles(times)}
        elif args.mode == "placements":
            times = run_placements(term, args.n, args.frames, args.cols, args.rows)
            result = {"mode": "placements", "n": args.n, **percentiles(times)}
        elif args.mode == "churn":
            times, payload = run_churn(term, args.n, args.frames)
            result = {"mode": "churn", "n": args.n,
                      "payload_bytes": payload, **percentiles(times)}
        else:
            times = run_text(term, args.cols, args.rows, args.frames)
            result = {"mode": "text", "cols": args.cols, "rows": args.rows,
                      "cells": args.cols * args.rows, **percentiles(times)}
        if "cells" in result and result["p50_ms"] > 0:
            result["cells_per_s_p50"] = round(result["cells"] / (result["p50_ms"] / 1000.0))
        if "payload_bytes" in result and result["p50_ms"] > 0:
            result["mib_s_p50"] = round(result["payload_bytes"] / 1048576
                                         / (result["p50_ms"] / 1000.0), 1)
        result["host"] = "kitty"
        result["times_ms"] = [round(t * 1000.0, 3) for t in times]
    finally:
        term.close()
    out = os.environ.get("BENCH_OUT", "/tmp/kitty-gfx-bench.json")
    with open(out, "w") as f:
        json.dump(result, f, indent=1)
    print(f"KITTY_BENCH {json.dumps(result)}", file=sys.stderr)


if __name__ == "__main__":
    main()
