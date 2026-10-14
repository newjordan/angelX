'use strict';
// Test only. With `?offline`, a test build (crates/web/testgame) plays
// without a host: this fake socket talks to the loopback server compiled
// into the same wasm (`loopback_*` exports). `?offline=80` delays every
// message 80 ms each way. A real game has no loopback and the page says so.

/* exported makeLoopback */
function makeLoopback(x, delayMs) {
  x.loopback_open(delayMs);
  const sock = {
    readyState: 0,
    binaryType: 'arraybuffer',
    onopen: null,
    onmessage: null,
    onclose: null,
    send(bytes) {
      const p = x.angel_alloc(bytes.length);
      new Uint8Array(x.memory.buffer, p, bytes.length).set(bytes);
      x.loopback_send(p, bytes.length, performance.now());
      x.angel_free(p, bytes.length);
    },
    close() {
      sock.readyState = 3;
    },
    // Called each frame: run the server to now, deliver what has arrived.
    pump(now) {
      if (sock.readyState !== 1) return;
      const n = x.loopback_take(now);
      if (!n) return;
      const packed = new Uint8Array(x.memory.buffer, x.loopback_ptr(), n).slice();
      for (let i = 0; i < n;) {
        const len = packed[i] | packed[i + 1] << 8 | packed[i + 2] << 16 | packed[i + 3] << 24;
        i += 4;
        sock.onmessage({ data: packed.buffer.slice(i, i + len) });
        i += len;
      }
    },
  };
  setTimeout(() => {
    sock.readyState = 1;
    if (sock.onopen) sock.onopen();
  }, 0);
  return sock;
}
