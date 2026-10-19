'use strict';
// Stage: draw a frame from its parts instead of a finished picture
// (docs/RENDER_FOUNDATION.md, "The game pipeline"). The module sends
// `angel-stage`'s wire format (crates/stage/src/wire.rs): the pictures that
// are new or changed, kept here as textures by id, and quads in the stage's
// own pixels, drawn back to front at the screen's density. A quad with no
// alpha adds to what is under it (light). The stage is drawn into a texture
// of its own, so a new scene can slide in over the last one.

/* exported makeStage */
function makeStage(gl, fit, screen) {
  // A quad turns by its angle about its middle, then stands on the stage's
  // tilted floor as angel-stage's Tilt says (crates/stage/src/lib.rs): the
  // floor projected (w carries depth, so pictures stay true), what stands
  // placed on its foot and scaled, the screen's marks untouched.
  const VS = '#version 300 es\n' +
    'in vec2 corner;in vec4 rect;in vec4 source;in vec4 color;in vec2 info;in vec4 turn;\n' +
    'uniform vec2 stage;uniform vec2 picture;uniform vec2 offset;uniform vec4 tilt;uniform float squash;\n' +
    'out vec2 uv;out vec2 local;out vec4 tint;flat out vec2 kind;\n' +
    'void main(){vec2 p=rect.xy+offset+corner*rect.zw;' +
    'vec2 c=rect.xy+offset+rect.zw*0.5;float sn=sin(turn.x),cs=cos(turn.x);vec2 d=p-c;' +
    'p=c+vec2(d.x*cs-d.y*sn,d.x*sn+d.y*cs);' +
    'float w=1.0;vec2 at=p;' +
    'if(tilt.w>0.0&&turn.y<1.5){' +
    'if(turn.y<0.5){vec2 e=p-tilt.xy;w=max(1.0-tilt.z*e.y,0.05);' +
    'at=vec2(tilt.x*w+tilt.w*e.x,tilt.y*w+tilt.w*squash*e.y);}' +
    'else{vec2 a=turn.zw-tilt.xy;float dp=max(1.0-tilt.z*a.y,0.05);' +
    'at=tilt.xy+vec2(tilt.w*a.x,tilt.w*squash*a.y)/dp+(p-turn.zw)*(tilt.w/dp);}}' +
    'gl_Position=vec4(at.x/stage.x*2.0-w,w-at.y/stage.y*2.0,0.0,w);' +
    'uv=(source.xy+corner*source.zw)/max(picture,vec2(1.0));' +
    'local=corner*2.0-1.0;tint=color;kind=info;}';
  const FS = '#version 300 es\nprecision highp float;uniform sampler2D t;' +
    'in vec2 uv;in vec2 local;in vec4 tint;flat in vec2 kind;out vec4 o;\n' +
    'void main(){' +
    'if(kind.y>0.5){float d=min(dot(local,local),1.0);o=tint*(1.0-d)*(1.0-d);return;}' +
    // kind.x 2 is the blit: the stage's own target, already premultiplied,
    // handed over as stored — an rgb*a here darkens translucent texels twice.
    'if(kind.x>1.5){o=texture(t,uv)*tint.a;return;}' +
    'if(kind.x>0.5){vec4 c=texture(t,uv);o=vec4(c.rgb*c.a,c.a)*tint.a;return;}' +
    'o=tint;}';
  // Floats an instance takes: rect, source, colour (4 each), textured,
  // shape, then its turn: angle, stand (0 floor, 1 upright, 2 screen), foot.
  const STRIDE = 18;
  // The wire (crates/stage/src/wire.rs): header bytes, bytes a quad takes.
  const HEADER = 56, QUAD = 72;
  // How long a new scene takes to slide in, in ms.
  const SLIDE_MS = 420;

  let prog, loc, quadBuf, instBuf, inst = new Float32Array(STRIDE * 256);
  let pictures = new Map();
  // The stage's two textures: the scene now, and the last one while the
  // new one slides in over it.
  const targets = [null, null];
  let tw = 0, th = 0, now = 0, lastScene = null, slide = null;

  function init() {
    const shader = (type, src) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      return s;
    };
    prog = gl.createProgram();
    gl.attachShader(prog, shader(gl.VERTEX_SHADER, VS));
    gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error('stage: ' + gl.getProgramInfoLog(prog));
    loc = {};
    for (const a of ['corner', 'rect', 'source', 'color', 'info', 'turn']) loc[a] = gl.getAttribLocation(prog, a);
    for (const u of ['stage', 'picture', 'offset', 't', 'tilt', 'squash']) loc[u] = gl.getUniformLocation(prog, u);
    quadBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 1, 1]), gl.STATIC_DRAW);
    instBuf = gl.createBuffer();
    pictures = new Map();
    targets[0] = targets[1] = null;
    tw = th = 0;
    lastScene = null;
    slide = null;
  }

  function texture(w, h) {
    const tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    return tex;
  }

  function target(i, w, h) {
    if (targets[i] && targets[i].w === w && targets[i].h === h) return targets[i];
    if (targets[i]) {
      gl.deleteTexture(targets[i].tex);
      gl.deleteFramebuffer(targets[i].fb);
    }
    const tex = texture(w, h);
    const fb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return (targets[i] = { tex, fb, w, h });
  }

  function upload(heap, at, id, w, h) {
    let p = pictures.get(id);
    if (!p || p.w !== w || p.h !== h) {
      if (p) gl.deleteTexture(p.tex);
      p = { tex: texture(w, h), w, h };
      pictures.set(id, p);
    }
    gl.bindTexture(gl.TEXTURE_2D, p.tex);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, heap, at);
  }

  // The instance attributes from `first` floats into the buffer.
  function point(first) {
    const bytes = STRIDE * 4;
    const at = (name, size, offset) => {
      gl.enableVertexAttribArray(loc[name]);
      gl.vertexAttribPointer(loc[name], size, gl.FLOAT, false, bytes, (first + offset) * 4);
      gl.vertexAttribDivisor(loc[name], 1);
    };
    gl.bindBuffer(gl.ARRAY_BUFFER, instBuf);
    at('rect', 4, 0);
    at('source', 4, 4);
    at('color', 4, 8);
    at('info', 2, 12);
    at('turn', 4, 14);
  }

  // `tilt`: the stage's floor [centre x, centre y, near, zoom, squash], or
  // none (the page, a flat stage).
  function begin(w, h, tilt) {
    gl.useProgram(prog);
    const tl = tilt || [0, 0, 0, 0, 1];
    gl.uniform4f(loc.tilt, tl[0], tl[1], tl[2], tl[3]);
    gl.uniform1f(loc.squash, tl[4]);
    gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
    gl.enableVertexAttribArray(loc.corner);
    gl.vertexAttribPointer(loc.corner, 2, gl.FLOAT, false, 0, 0);
    gl.vertexAttribDivisor(loc.corner, 0);
    gl.uniform2f(loc.stage, w, h);
    gl.uniform2f(loc.offset, 0, 0);
    gl.uniform1i(loc.t, 0);
    gl.activeTexture(gl.TEXTURE0);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  }

  // Draw `n` instances from `first`, the picture `id` bound (0: none).
  function batch(first, n, id) {
    const p = id ? pictures.get(id) : null;
    if (id && !p) return;
    if (p) gl.bindTexture(gl.TEXTURE_2D, p.tex);
    gl.uniform2f(loc.picture, p ? p.w : 1, p ? p.h : 1);
    point(first * STRIDE);
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, n);
  }

  // One texture over the box, flipped as a framebuffer is. Info x=2:
  // the passthrough kind, for the stage's own premultiplied targets.
  function blit(tex, w, h, x, y) {
    inst.set([x, y, w, h, 0, h, w, -h, 1, 1, 1, 1, 2, 0, 0, 2, 0, 0], 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, instBuf);
    gl.bufferData(gl.ARRAY_BUFFER, inst.subarray(0, STRIDE), gl.STREAM_DRAW);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.uniform2f(loc.picture, w, h);
    point(0);
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, 1);
  }

  function ease(k) {
    k = Math.min(1, Math.max(0, k));
    return k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2;
  }

  init();

  return {
    init,
    // `heap` is a Uint8Array over the module's memory, the frame `len`
    // bytes at `ptr`; `t` the page's clock in ms.
    draw(heap, ptr, len, t) {
      now = t;
      // The whole frame is validated before any GL work: a truncated or
      // lying buffer draws nothing instead of throwing halfway through
      // its uploads. Sizes cap at the texture limit every host keeps.
      if (len < HEADER) return false;
      const dv = new DataView(heap.buffer, heap.byteOffset + ptr, len);
      if (dv.getUint32(0, true) !== 0x32475453) return false; // "STG2"
      const w = dv.getUint32(4, true), h = dv.getUint32(8, true);
      const npics = dv.getUint32(12, true), nquads = dv.getUint32(16, true);
      const scene = dv.getUint32(20, true);
      const ex = dv.getFloat32(24, true), ey = dv.getFloat32(28, true);
      const tilt = [0, 1, 2, 3, 4].map((k) => dv.getFloat32(32 + k * 4, true));
      // A tilt that is not finite draws flat.
      const tilted = tilt.every(Number.isFinite) && tilt[3] > 0 ? tilt : null;
      const MAX_SIDE = 8192;
      if (!w || !h || w > MAX_SIDE || h > MAX_SIDE) return false;
      let need = HEADER;
      for (let i = 0; i < npics; i++) {
        if (need + 12 > len) return false;
        const pw = dv.getUint32(need + 4, true), ph = dv.getUint32(need + 8, true);
        if (!pw || !ph || pw > MAX_SIDE || ph > MAX_SIDE) return false;
        need += 12 + pw * ph * 4;
        if (need > len) return false;
      }
      if (need + nquads * QUAD > len) return false;
      let at = HEADER;
      for (let i = 0; i < npics; i++) {
        const id = dv.getUint32(at, true), pw = dv.getUint32(at + 4, true), ph = dv.getUint32(at + 8, true);
        at += 12;
        upload(heap, ptr + at, id, pw, ph);
        at += pw * ph * 4;
      }
      if (inst.length < nquads * STRIDE) inst = new Float32Array(nquads * STRIDE * 2);
      const pics = new Uint32Array(nquads);
      for (let i = 0; i < nquads; i++) {
        const o = i * STRIDE;
        for (let k = 0; k < 12; k++) inst[o + k] = dv.getFloat32(at + k * 4, true);
        for (let k = 0; k < 4; k++) inst[o + 14 + k] = dv.getFloat32(at + 48 + k * 4, true);
        pics[i] = dv.getUint32(at + 64, true);
        inst[o + 12] = pics[i] ? 1 : 0;
        inst[o + 13] = dv.getUint32(at + 68, true);
        at += QUAD;
      }
      const b = fit(w, h);
      const bw = b[2], bh = b[3];
      // A new scene through a door slides in over the last.
      if (lastScene !== null && scene !== lastScene && !Number.isNaN(ex) && targets[0]) {
        const kept = targets[0];
        targets[0] = targets[1];
        targets[1] = kept;
        slide = { start: t, ex, ey };
      }
      lastScene = scene;
      const cur = target(0, bw, bh);
      gl.bindFramebuffer(gl.FRAMEBUFFER, cur.fb);
      gl.viewport(0, 0, bw, bh);
      gl.disable(gl.SCISSOR_TEST);
      // Transparent, like the glass window's stage targets: a scene
      // that wants paper paints it, and the page shows through the rest.
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      begin(w, h, tilted);
      gl.bindBuffer(gl.ARRAY_BUFFER, instBuf);
      gl.bufferData(gl.ARRAY_BUFFER, inst.subarray(0, nquads * STRIDE), gl.STREAM_DRAW);
      let first = 0;
      for (let i = 1; i <= nquads; i++) {
        if (i === nquads || pics[i] !== pics[first]) {
          batch(first, i - first, pics[first]);
          first = i;
        }
      }
      // Onto the page: the stage's box, the page showing through around
      // and inside it wherever the scene is transparent.
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.viewport(0, 0, screen.w, screen.h);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.viewport(b[0], screen.h - b[1] - bh, bw, bh);
      begin(bw, bh);
      const k = slide ? (t - slide.start) / SLIDE_MS : 1;
      if (k < 1 && targets[1] && targets[1].w === bw && targets[1].h === bh) {
        const e = ease(k);
        blit(targets[1].tex, bw, bh, -slide.ex * bw * e, -slide.ey * bh * e);
        blit(cur.tex, bw, bh, slide.ex * bw * (1 - e), slide.ey * bh * (1 - e));
      } else {
        slide = null;
        blit(cur.tex, bw, bh, 0, 0);
      }
      gl.disable(gl.BLEND);
      // Leave the context as the frame's presenter expects it (its
      // letterbox is opaque black; only stage frames are transparent).
      for (const a of ['rect', 'source', 'color', 'info', 'turn']) {
        gl.vertexAttribDivisor(loc[a], 0);
        gl.disableVertexAttribArray(loc[a]);
      }
      gl.clearColor(0, 0, 0, 0);
      return true;
    },
  };
}
