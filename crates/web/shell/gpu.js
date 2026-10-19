'use strict';
// Gpu: the renderer module as a presenter (docs/GLASS_CORE.md, decision 6).
//
// The module (crates/web/render) is `angel-gpu`, the core the native window
// draws with, behind a canvas, in a wasm of its own beside the game's. The
// bundle carries it in two builds (`render-webgpu.js` + `render-webgpu_bg.wasm`,
// and `render-webgl.js` + `render-webgl_bg.wasm`, which adds wgpu's own WebGL2
// backend for a browser without `navigator.gpu`); boot.js picks one and this
// file wraps it in the interface present.js has, so the frame loop does not
// know which it is talking to:
//
//   kind        'gpu-webgpu' or 'gpu-webgl'
//   rgba        true: a finished frame comes from `angel_frame_rgba`, as RGBA bytes
//   props       true: a stage frame may carry 3D props (`STG3`), which the module draws
//   draw        a finished frame (the game's RGBA bytes), scaled to the canvas
//   drawStage   a stage frame (`angel_stage`'s bytes), decoded and drawn by the module;
//               a room in 3D's plate comes in it as a PNG, which is decoded
//               here with the browser's own decoder and handed back as pixels
//   clear, setScale, onreset: as present.js
//   setPost     the module's post pass (crates/gpu/src/post.rs) on or off
//   onlost      the module stopped (a lost device, a trap): called once, after
//               the frame in progress, for boot.js to go on with present.js
//
// Nothing else of the frame is read or drawn here; every call goes straight in.

/* exported loadRenderer, makeGpuPresenter */

// A renderer build's glue (an ES module) and its wasm, loaded. `build` is
// {js, wasm}: the two asset URLs the page's manifest names.
async function loadRenderer(build) {
  const mod = await import(build.js);
  await mod.default({ module_or_path: build.wasm });
  return mod;
}

// A presenter on `canvas`, which must not have a context yet, drawing through
// `mod` (what `loadRenderer` returns). `backend` is 'webgpu' or 'webgl'.
// Rejects when there is no adapter; the canvas is then not fit for another
// kind of context (boot.js puts a fresh one in its place).
async function makeGpuPresenter(canvas, mod, backend) {
  const renderer = await mod.Renderer.create(canvas, backend);
  // A picture the renderer dropped for its budget is asked for again, but
  // not more than this often: a scene too big for the budget would ask every frame.
  const RESET_EVERY_MS = 1000;

  let dead = false, onreset = null, onlost = null, lastReset = -Infinity;

  // The backing store in device pixels, kept equal to what the canvas covers
  // on screen, so whole-number scales stay whole on the glass. The renderer
  // sets the canvas's size and fits the frame in it.
  const observer = new ResizeObserver(entries => {
    if (dead) return;
    const e = entries[0];
    const box = e.devicePixelContentBoxSize && e.devicePixelContentBoxSize[0];
    let w, h;
    if (box) {
      w = box.inlineSize;
      h = box.blockSize;
    } else {
      // Safari has no device-pixel box: round the CSS box times the ratio.
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

  // The module stopped: say so once, after the frame in progress (a frame
  // that began as RGBA bytes must not reach another presenter).
  function fail(why) {
    if (dead) return;
    dead = true;
    observer.disconnect();
    try { renderer.free(); } catch { /* it is gone either way */ }
    queueMicrotask(() => { if (onlost) onlost(why); });
  }
  // Run a call into the module: a trap or a lost device ends it.
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
  // A room in 3D's plate came to the module as a PNG (each item's once): take
  // it, decode it off the frame with the browser's own decoder, and give the
  // module its pixels, the picture the room's blocks name. Until then the
  // blocks draw without it.
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

  // wgpu's WebGL2 backend has no device to lose; its context can go.
  canvas.addEventListener('webglcontextlost', () => fail('the WebGL context was lost'));

  return {
    kind: 'gpu-' + renderer.kind(),
    adapter: renderer.adapter(),
    rgba: true,
    props: true,
    // `heap` is a Uint8Array over the game's memory; the frame is w*h RGBA
    // pixels at byte offset `ptr`.
    draw(heap, ptr, w, h) {
      guard(() => renderer.present_frame(w, h, heap.subarray(ptr, ptr + w * h * 4)));
    },
    clear() {
      guard(() => renderer.clear());
    },
    // A frame from its parts: false when it was not one (the loop then draws
    // the finished frame). `t` is the page's clock in ms.
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
    // The post pass on (1) or off (0); returns what it is now.
    setPost: on => guard(() => renderer.set_post(!!on)) ? 1 : 0,
    set onreset(f) { onreset = f; },
    set onlost(f) { onlost = f; },
    // Let go of the GPU: the page is going on with another presenter.
    dispose() {
      if (dead) return;
      dead = true;
      observer.disconnect();
      try { renderer.free(); } catch { /* gone */ }
    },
  };
}
