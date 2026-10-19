'use strict';
// Present: put the game's frame on the screen. A game draws at its own
// native size (the muster 320x180, the Delve 384x224 or 768x448); here it
// is scaled by the largest whole number that fits, nearest-sampled, black
// around it. Where a whole number would leave too much of the window black
// (a large frame on a small screen: the Delve's second art at 768 wide in
// a 1280 window is only 1x), or not even 1x fits, it is fitted instead,
// still nearest, rather than shown small or cropped.
//
// Three ways, best first: WebGL2, WebGL1, a 2D canvas. The frame's words
// are 0xAARRGGBB little-endian, so its bytes are B G R A: WebGL uploads
// them untouched and swizzles in the shader. The 2D canvas has no shader,
// so it asks the module for an RGBA copy instead (`rgba` below); no pixel
// is ever touched in JS.

/* exported makePresenter */
/* global makeStage */
// `want` forces a fallback for testing: 'webgl1' or '2d' (`?present=`).
function makePresenter(canvas, want) {
  // The backing store in device pixels, kept equal to what the canvas
  // covers on screen, so whole-number scales stay whole on the glass.
  const screen = { w: 1, h: 1 };
  function setSize(w, h) {
    w = Math.max(1, w | 0);
    h = Math.max(1, h | 0);
    if (w === screen.w && h === screen.h) return;
    screen.w = canvas.width = w;
    screen.h = canvas.height = h;
  }
  const observer = new ResizeObserver(entries => {
    const e = entries[0];
    const box = e.devicePixelContentBoxSize && e.devicePixelContentBoxSize[0];
    if (box) {
      setSize(box.inlineSize, box.blockSize);
    } else {
      // Safari has no device-pixel box: round the CSS box times the ratio.
      const css = e.contentBoxSize ? e.contentBoxSize[0] : null;
      const dpr = window.devicePixelRatio || 1;
      setSize(Math.round((css ? css.inlineSize : canvas.clientWidth) * dpr),
              Math.round((css ? css.blockSize : canvas.clientHeight) * dpr));
    }
  });
  try {
    observer.observe(canvas, { box: 'device-pixel-content-box' });
  } catch {
    observer.observe(canvas);
  }

  // Where a w x h frame goes in the backing store: x, y (from the top),
  // width, height, written into one reused box.
  const box = new Int32Array(4);
  // A whole-number scale is kept while it fills at least this share of
  // what fitting the frame would.
  const MIN_FILL = 0.8;
  // The dev panel's `scale` knob: 0 as above, 1 whole numbers whenever 1x
  // fits, 2 always fitted.
  let mode = 0;
  function fit(w, h) {
    const cw = screen.w, ch = screen.h;
    const k = Math.min(Math.floor(cw / w), Math.floor(ch / h));
    let dw, dh;
    const whole = mode === 1 ? k >= 1 : mode === 2 ? false : k >= 1 && k >= MIN_FILL * Math.min(cw / w, ch / h);
    if (whole) {
      dw = w * k;
      dh = h * k;
    } else {
      const s = Math.min(cw / w, ch / h);
      dw = Math.max(1, Math.floor(w * s));
      dh = Math.max(1, Math.floor(h * s));
    }
    box[0] = (cw - dw) >> 1;
    box[1] = (ch - dh) >> 1;
    box[2] = dw;
    box[3] = dh;
    return box;
  }

  // Alpha and premultiplied, like the glass window's surface: a stage
  // frame (stage.js) is premultiplied colour over transparency, and
  // the page shows through it. A finished frame stays opaque — its
  // shader forces alpha — so this costs that path nothing.
  const opts = { alpha: true, antialias: false, depth: false, stencil: false,
    premultipliedAlpha: true, preserveDrawingBuffer: false };
  let gl = want === 'webgl1' || want === '2d' ? null : canvas.getContext('webgl2', opts);
  const two = !!gl;
  if (!gl && want !== '2d') gl = canvas.getContext('webgl', opts);
  const presenter = gl ? glPresenter(canvas, gl, two, fit, screen) : canvasPresenter(canvas, fit, screen);
  presenter.setScale = m => {
    mode = m === 1 || m === 2 ? m : 0;
    return mode;
  };
  return presenter;
}

function glPresenter(canvas, gl, two, fit, screen) {
  const VS2 = '#version 300 es\nin vec2 p;out vec2 uv;' +
    'void main(){uv=vec2(p.x+1.0,1.0-p.y)*0.5;gl_Position=vec4(p,0.0,1.0);}';
  const FS2 = '#version 300 es\nprecision highp float;uniform sampler2D t;in vec2 uv;out vec4 o;' +
    'void main(){o=vec4(texture(t,uv).bgr,1.0);}';
  const VS1 = 'attribute vec2 p;varying vec2 uv;' +
    'void main(){uv=vec2(p.x+1.0,1.0-p.y)*0.5;gl_Position=vec4(p,0.0,1.0);}';
  const FS1 = '#ifdef GL_FRAGMENT_PRECISION_HIGH\nprecision highp float;\n#else\nprecision mediump float;\n#endif\n' +
    'uniform sampler2D t;varying vec2 uv;void main(){gl_FragColor=vec4(texture2D(t,uv).bgr,1.0);}';

  let tex = null, tw = 0, th = 0, lost = false, prog = null, quad = null;
  // WebGL2 can draw a frame from its parts (stage.js); `onreset` hears when
  // a restored context needs every picture again.
  const stage = two && typeof makeStage === 'function' ? makeStage(gl, fit, screen) : null;
  let onreset = null;
  // WebGL1 cannot upload from an offset into a view, so it keeps a view
  // that starts at the frame; remade only when the frame or memory moves.
  let view = null, viewBuf = null, viewPtr = -1, viewLen = 0;

  function init() {
    const shader = (type, src) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      return s;
    };
    prog = gl.createProgram();
    gl.attachShader(prog, shader(gl.VERTEX_SHADER, two ? VS2 : VS1));
    gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, two ? FS2 : FS1));
    gl.bindAttribLocation(prog, 0, 'p');
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error('present: ' + gl.getProgramInfoLog(prog));
    gl.useProgram(prog);
    quad = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    gl.clearColor(0, 0, 0, 0);
    tw = th = 0;
  }
  init();

  // A lost context (a GPU reset, a phone backgrounding the page) comes back
  // only if the loss is prevented; then everything is made again.
  canvas.addEventListener('webglcontextlost', e => { e.preventDefault(); lost = true; });
  canvas.addEventListener('webglcontextrestored', () => {
    lost = false;
    init();
    if (stage) {
      stage.init();
      if (onreset) onreset();
    }
  });

  // The frame's own program and quad, again: the stage shares the context.
  function rebind() {
    gl.useProgram(prog);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    if (two) gl.vertexAttribDivisor(0, 0);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, tex);
  }

  return {
    kind: two ? 'webgl2' : 'webgl1',
    rgba: false,
    // `heap` is a Uint8Array over the module's memory; the frame is w*h
    // words at byte offset `ptr`.
    draw(heap, ptr, w, h) {
      if (lost) return;
      if (stage) rebind();
      if (w !== tw || h !== th) {
        tw = w;
        th = h;
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      }
      if (two) {
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, heap, ptr);
      } else {
        const len = w * h * 4;
        if (heap.buffer !== viewBuf || ptr !== viewPtr || len !== viewLen) {
          view = heap.subarray(ptr, ptr + len);
          viewBuf = heap.buffer;
          viewPtr = ptr;
          viewLen = len;
        }
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, view);
      }
      const b = fit(w, h);
      gl.viewport(0, 0, screen.w, screen.h);
      gl.clear(gl.COLOR_BUFFER_BIT);
      // GL counts y from the bottom.
      gl.viewport(b[0], screen.h - b[1] - b[3], b[2], b[3]);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    },
    clear() {
      if (lost) return;
      gl.viewport(0, 0, screen.w, screen.h);
      gl.clear(gl.COLOR_BUFFER_BIT);
    },
    // A frame from its parts (`angel_stage`): false when it was not one.
    drawStage: stage ? (heap, ptr, len, t) => !lost && stage.draw(heap, ptr, len, t) : null,
    set onreset(f) {
      onreset = f;
    },
  };
}

function canvasPresenter(canvas, fit, screen) {
  const ctx = canvas.getContext('2d', { alpha: true });
  const off = document.createElement('canvas');
  const offCtx = off.getContext('2d');
  let image = null, imageBuf = null, imagePtr = -1, iw = 0, ih = 0;
  return {
    kind: 'canvas2d',
    // Draw with `angel_frame_rgba`: the module swizzles, not JS.
    rgba: true,
    draw(heap, ptr, w, h) {
      if (heap.buffer !== imageBuf || ptr !== imagePtr || w !== iw || h !== ih) {
        image = new ImageData(new Uint8ClampedArray(heap.buffer, ptr, w * h * 4), w, h);
        imageBuf = heap.buffer;
        imagePtr = ptr;
        if (w !== iw || h !== ih) {
          off.width = iw = w;
          off.height = ih = h;
        }
      }
      offCtx.putImageData(image, 0, 0);
      const b = fit(w, h);
      ctx.clearRect(0, 0, screen.w, screen.h);
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(off, b[0], b[1], b[2], b[3]);
    },
    clear() {
      ctx.clearRect(0, 0, screen.w, screen.h);
    },
  };
}
