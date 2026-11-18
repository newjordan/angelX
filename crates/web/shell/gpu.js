'use strict';

async function loadRenderer(build) {
  const mod = await import(build.js);
  await mod.default({ module_or_path: build.wasm });
  return mod;
}

async function makeGpuPresenter(canvas, mod, backend) {
  const renderer = await mod.Renderer.create(canvas, backend);
  const RESET_EVERY_MS = 1000;

  let dead = false, onreset = null, onlost = null, lastReset = -Infinity;

  const observer = new ResizeObserver(entries => {
    if (dead) return;
    const e = entries[0];
    const box = e.devicePixelContentBoxSize && e.devicePixelContentBoxSize[0];
    let w, h;
    if (box) {
      w = box.inlineSize;
      h = box.blockSize;
    } else {
      const css = e.contentBoxSize ? e.contentBoxSize[0] : null;
      const dpr = window.devicePixelRatio || 1;
      w = Math.round((css ? css.inlineSize : canvas.clientWidth) * dpr);
      h = Math.round((css ? css.blockSize : canvas.clientHeight) * dpr);
    }
    guard(() => renderer.resize(Math.max(1, w | 0), Math.max(1, h | 0)));
  });
  try {
    observer.observe(canvas, { box: 'device-pixel-content-box' });
  } catch {
    observer.observe(canvas);
  }

  function fail(why) {
    if (dead) return;
    dead = true;
    observer.disconnect();
    try { renderer.free(); } catch { }
    queueMicrotask(() => { if (onlost) onlost(why); });
  }
  function guard(f) {
    if (dead) return undefined;
    try {
      const r = f();
      if (renderer.lost()) fail('the GPU device was lost');
      return r;
    } catch (e) {
      fail(e);
      return undefined;
    }
  }
  function takePlates() {
    for (let id = guard(() => renderer.plate_wanted()) >>> 0; id; id = guard(() => renderer.plate_wanted()) >>> 0) {
      const png = guard(() => renderer.take_plate());
      if (!png || !png.length) return;
      decodePlate(id, png);
    }
  }
  async function decodePlate(id, png) {
    try {
      const bitmap = await createImageBitmap(new Blob([png], { type: 'image/png' }),
        { premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
      const w = bitmap.width, h = bitmap.height;
      const pad = typeof OffscreenCanvas === 'function'
        ? new OffscreenCanvas(w, h)
        : Object.assign(document.createElement('canvas'), { width: w, height: h });
      const g = pad.getContext('2d', { willReadFrequently: true });
      g.drawImage(bitmap, 0, 0);
      bitmap.close();
      const rgba = g.getImageData(0, 0, w, h).data;
      guard(() => renderer.put_picture(id, w, h, new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength)));
    } catch (e) {
      console.warn('a plate did not decode', e);
      guard(() => renderer.plate_failed(id));
    }
  }

  canvas.addEventListener('webglcontextlost', () => fail('the WebGL context was lost'));

  return {
    kind: 'gpu-' + renderer.kind(),
    adapter: renderer.adapter(),
    rgba: true,
    props: true,
    draw(heap, ptr, w, h) {
      guard(() => renderer.present_frame(w, h, heap.subarray(ptr, ptr + w * h * 4)));
    },
    clear() {
      guard(() => renderer.clear());
    },
    drawStage(heap, ptr, len, t) {
      const drawn = guard(() => renderer.present_stage(heap.subarray(ptr, ptr + len), t));
      takePlates();
      if (drawn === 2 && onreset && t - lastReset >= RESET_EVERY_MS) {
        lastReset = t;
        onreset();
      }
      return drawn > 0;
    },
    setScale: m => guard(() => renderer.set_scale(m)) ?? 0,
    setPost: on => guard(() => renderer.set_post(!!on)) ? 1 : 0,
    set onreset(f) { onreset = f; },
    set onlost(f) { onlost = f; },
    dispose() {
      if (dead) return;
      dead = true;
      observer.disconnect();
      try { renderer.free(); } catch { }
    },
  };
}
