#!/usr/bin/env python3
"""Join a hosted Delve seat over its WebSocket the way the browser page does,
and report what a friend would see: frames per second, the gaps between
frames, ping, and how old frames are when they land.

    delve-socket-probe.py <invitation link> [seconds] [--ip <address>] [--ack-delay <ms>]

The link is the one the host hands out (http(s)://host:port/#<seat code>).
--ip connects to that address while keeping the link's name for TLS, to test
one route (e.g. a public Funnel ingress) on purpose. --ack-delay holds each
frame receipt back, as a far route does, so the host's frame window skips
frames. Only the standard library is used, so it runs on a bare Mac or
Linux box.
"""

import base64, json, os, socket, ssl, struct, sys, threading, time
from urllib.parse import urlsplit


def main():
    args = sys.argv[1:]
    ip = None
    if "--ip" in args:
        at = args.index("--ip")
        ip = args[at + 1]
        del args[at : at + 2]
    ack_delay = 0.0
    if "--ack-delay" in args:
        at = args.index("--ack-delay")
        ack_delay = float(args[at + 1]) / 1000
        del args[at : at + 2]
    link = args[0]
    seconds = float(args[1]) if len(args) > 1 else 10.0
    url = urlsplit(link)
    token = url.fragment
    secure = url.scheme == "https"
    host = url.hostname
    port = url.port or (443 if secure else 80)

    def connect():
        raw = socket.create_connection((ip or host, port), timeout=10)
        raw.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        return ssl.create_default_context().wrap_socket(raw, server_hostname=host) if secure else raw

    # Take the seat as the page does on load.
    hello = connect()
    hello.sendall(
        (
            f"POST /hello HTTP/1.1\r\nHost: {url.netloc}\r\nAuthorization: Bearer {token}\r\n"
            f"Content-Type: text/plain\r\nContent-Length: 5\r\nConnection: close\r\n\r\nprobe"
        ).encode()
    )
    print("hello     ", hello.recv(4096).split(b"\r\n")[0].decode())
    hello.close()
    sock = connect()
    key = base64.b64encode(os.urandom(16)).decode()
    sock.sendall(
        (
            f"GET /play HTTP/1.1\r\nHost: {url.netloc}\r\nUpgrade: websocket\r\n"
            f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n"
            f"Sec-WebSocket-Protocol: delve.v1, {token}\r\n\r\n"
        ).encode()
    )
    head = b""
    while b"\r\n\r\n" not in head:
        chunk = sock.recv(4096)
        if not chunk:
            sys.exit("closed during handshake")
        head += chunk
    head, rest = head.split(b"\r\n\r\n", 1)
    status = head.split(b"\r\n")[0].decode()
    if " 101 " not in status:
        sys.exit(f"no socket: {status}")
    lock = threading.Lock()

    def send(obj):
        payload = json.dumps(obj, separators=(",", ":")).encode()
        mask = os.urandom(4)
        n = len(payload)
        header = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n))
        with lock:
            sock.sendall(header + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))

    def late_ack(seq):
        try:
            send({"ack": seq})
        except OSError:
            pass

    buf = bytearray(rest)

    def read_exact(n):
        while len(buf) < n:
            chunk = sock.recv(65536)
            if not chunk:
                raise EOFError
            buf.extend(chunk)
        out = bytes(buf[:n])
        del buf[:n]
        return out

    def read_message():
        b0, b1 = read_exact(2)
        n = b1 & 0x7F
        if n == 126:
            n = struct.unpack(">H", read_exact(2))[0]
        elif n == 127:
            n = struct.unpack(">Q", read_exact(8))[0]
        return b0 & 0x0F, read_exact(n)

    state = {"raid": None, "next": 1}
    pings, ages, gaps, arrivals, sizes = [], [], [], [], []
    seqs = []
    errors = []
    kinds = {1: 0, 4: 0}
    # Each patch must name the picture received just before it as its base.
    held = {"seq": None, "bad": 0}
    t0 = time.time()
    stop = threading.Event()

    def keys():
        # Walk right then left, renewing every 80 ms like the page.
        sequence = 1
        while not stop.is_set():
            if state["raid"] is not None:
                sequence = max(sequence, state["next"])
                step = int((time.time() - t0) / 1.0) % 2
                send({"sequence": sequence, "raid_id": state["raid"],
                      "input": {"move_x": 1 if step == 0 else -1, "move_y": 0, "aim_x": 0, "aim_y": 0,
                                "fire": False, "dash": False, "bomb": False, "swing": False,
                                "play": 0, "cast": 0}})
                sequence += 1
            if int((time.time() - t0) * 1000) % 500 < 80:
                send({"ping": time.perf_counter() * 1000})
            time.sleep(0.08)

    threading.Thread(target=keys, daemon=True).start()
    try:
        while time.time() - t0 < seconds:
            opcode, payload = read_message()
            if opcode == 0x8:
                break
            if opcode != 0x2 or not payload:
                continue
            kind = payload[0]
            now = time.time()
            if kind in (1, 4):
                kinds[kind] = kinds.get(kind, 0) + 1
                seq, painted = struct.unpack(">QQ", payload[1:17])
                if ack_delay:
                    threading.Timer(ack_delay, late_ack, (seq,)).start()
                else:
                    send({"ack": seq})
                if kind == 4 and struct.unpack(">Q", payload[25:33])[0] != held["seq"]:
                    held["bad"] += 1
                held["seq"] = seq
                if arrivals:
                    gaps.append((now - arrivals[-1]) * 1000)
                arrivals.append(now)
                sizes.append(len(payload))
                seqs.append(seq)
                ages.append(now * 1000 - painted)
            elif kind == 2:
                s = json.loads(payload[1:])
                state["raid"], state["next"] = s.get("raid_id"), s.get("next_sequence", 1)
            elif kind == 3:
                note = json.loads(payload[1:])
                if "pong" in note:
                    pings.append(time.perf_counter() * 1000 - note["pong"])
                elif "input_error" in note:
                    errors.append(note["input_error"])
    finally:
        stop.set()

    def pct(values, p):
        values = sorted(values)
        return values[min(len(values) - 1, int(len(values) * p))] if values else float("nan")

    span = arrivals[-1] - arrivals[0] if len(arrivals) > 1 else float("nan")
    print(f"route      {ip or host}:{port} ({'wss' if secure else 'ws'})")
    print(f"frames     {len(arrivals)} in {span:.1f}s = {len(arrivals) / span:.1f} fps, "
          f"{sum(sizes) / span / 1024:.0f} KB/s, {sum(sizes) / max(1, len(sizes)) / 1024:.1f} KB each")
    print(f"gap ms     p50 {pct(gaps, .5):.0f}  p90 {pct(gaps, .9):.0f}  p99 {pct(gaps, .99):.0f}  max {max(gaps or [0]):.0f}")
    print(f"pictures   png {kinds.get(1, 0)}  tiles {kinds.get(4, 0)}  patches off their base {held['bad']}")
    if os.environ.get("DELVE_PROBE_HIST") == "1" and gaps:
        buckets = {}
        for gap in gaps:
            key = int(gap // 4) * 4
            buckets[key] = buckets.get(key, 0) + 1
        hist = " ".join(f"{k}-{k+4}:{buckets[k]}" for k in sorted(buckets))
        print(f"gap hist   {hist}")
        long = sorted(gaps, reverse=True)[:8]
        print("gap long   " + " ".join(f"{g:.0f}" for g in long))
        jumps = [seqs[i] - seqs[i - 1] for i in range(1, len(seqs))]
        jumped = [(gaps[i - 1], jumps[i - 1]) for i in range(1, len(seqs)) if jumps[i - 1] != 1]
        print(f"seq jumps  {len(jumped)} of {len(jumps)} steps not consecutive")
        if jumped:
            show = " ".join(f"{g:.0f}ms/Δ{d}" for g, d in sorted(jumped, reverse=True)[:8])
            print(f"seq long   {show}")
    print(f"ping ms    p50 {pct(pings, .5):.0f}  p90 {pct(pings, .9):.0f}  ({len(pings)} pings)")
    print(f"frame age  p50 {pct(ages, .5):.0f}  p90 {pct(ages, .9):.0f} ms (host clock vs this one)")
    print(f"controls   {len(errors)} refused{': ' + errors[0] if errors else ''}")


if __name__ == "__main__":
    main()
