# SPDX-License-Identifier: MIT
# angelX VFX: independent procedural pixel renderer. Bend 2.0.39.
import Base

type Layer is Data:
  Layer{kind: U32, colors: List<&2, U32>, ps: List<&2, F32>, glyphs: List<&2, U32>}

type Scene is Data:
  Scene{w: U32, h: U32, frames: U32, seed: U32, loop: Bool,
        outline: U32, dither: F32, clean: Bool, layers: List<&2, Layer>}

def number(m: Maybe<&2, F32>) -> F32:
  match m:
    case None{}:
      0.0
    case Some{x}:
      x

def word(m: Maybe<&2, U32>) -> U32:
  match m:
    case None{}:
      0
    case Some{x}:
      x

def text(m: Maybe<&2, String>) -> String:
  match m:
    case None{}:
      ""
    case Some{x}:
      x

def p(ps: List<&2, F32>, n: Nat) -> F32:
  number(List.get(&2, F32, ps, n))

def str(xs: List<&2, String>, n: Nat) -> String:
  text(List.get(&2, String, xs, n))

def u(xs: List<&2, String>, n: Nat) -> U32:
  word(U32.read(str(xs, n)))

def floats(xs: List<&2, String>) -> List<&2, F32>:
  match xs:
    case Nil{}:
      Nil{}
    case h <> t:
      number(F32.read(h)) <> floats(t)

def words(xs: List<&2, String>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case h <> t:
      word(U32.read(h)) <> words(t)

def read_layers(n: Nat, +xs: List<&2, String>) -> List<&2, Layer>:
  match n:
    case 0n:
      Nil{}
    case Succ{+k}:
      l = {Layer{u(xs, 0n), words(String.split(str(xs, 1n), ',')),
                floats(String.split(str(xs, 2n), ',')),
                words(String.split(str(xs, 3n), ','))} : Layer}
      l <> read_layers(k, List.drop(&2, String, xs, 4n))

def read_scene(+xs: List<&2, String>) -> Scene:
  Scene{u(xs, 0n), u(xs, 1n), u(xs, 2n), u(xs, 3n), U32.is_eq(u(xs, 4n), 1),
        u(xs, 5n), number(F32.read(str(xs, 6n))), U32.is_eq(u(xs, 7n), 1),
        read_layers(U32.to_nat(u(xs, 8n)), List.drop(&2, String, xs, 9n))}

def choose(c: Bool, a: F32, b: F32) -> F32:
  Bool.pick(F32, c, a, b)

def sat(x: F32) -> F32:
  F32.clamp(x, 0.0, 1.0)

def frac(+x: F32) -> F32:
  (x - F32.floor(x) : F32)

def noise(x: F32, y: F32, s: U32) -> F32:
  ix = F32.to_u32(F32.floor(((x + 2048.0) * 16.0 : F32)))
  iy = F32.to_u32(F32.floor(((y + 2048.0) * 16.0 : F32)))
  +v = (ix * 374761393 + iy * 668265263 + s * 1442695041 : U32)
  +z = ((v .^. (v >> 13n)) * 1274126177 : U32)
  (U32.to_f32(((z .^. (z >> 16n)) .&. 65535 : U32)) / 65536.0 : F32)

def disc(+x: F32, +y: F32, r: F32, th: F32) -> F32:
  sat(((r - F32.sqrt((x * x + y * y : F32))) / F32.max(th, 0.008) : F32))

def ring(+r: F32, radius: F32, th: F32) -> F32:
  sat((1.0 - F32.abs((r - radius : F32)) / F32.max(th, 0.008) : F32))

def phase(+t: F32, offset: F32, looping: Bool) -> F32:
  choose(looping, frac((t + offset : F32)), t)

# ps: t0,t1,x,y,r0,r1,th,angle,squash,gain,speed,spin,count,size,life,mode,gravity
def motes(n: Nat, +kind: U32, +x: F32, +y: F32, +t: F32,
          +speed: F32, +size: F32, +life: F32, +gravity: F32,
          +mode: U32, +seed: U32, +looping: Bool, acc: F32) -> F32:
  match n:
    case 0n:
      acc
    case Succ{+k}:
      +i = F32.from_nat(k)
      +a = (noise(i, 1.0, seed) * 6.2831853 : F32)
      +age = phase(t, noise(i, 2.0, seed), looping)
      +travel = (choose(U32.is_eq(mode, 4), (1.0 - age : F32), age) * speed : F32)
      +angle = (a + choose(U32.is_eq(mode, 7) || U32.is_eq(mode, 5), (age * 6.2831853 : F32), 0.0) : F32)
      +vertical = U32.is_eq(mode, 1) || U32.is_eq(mode, 2)
      +px = choose(vertical, ((noise(i, 3.0, seed) - 0.5) * 1.8 : F32),
              choose(U32.is_eq(mode, 6), (0.5 - travel * 1.8 : F32),
                choose(U32.is_eq(mode, 3), ((noise(i, 4.0, seed) - 0.5) * 1.8 : F32),
                  (F32.cos(angle) * choose(U32.is_eq(mode, 5), 0.65, travel) : F32))))
      +py = choose(U32.is_eq(mode, 1), (0.9 - travel * 2.0 : F32),
              choose(U32.is_eq(mode, 2), (travel * 2.0 - 0.9 : F32),
                choose(U32.is_eq(mode, 3) || U32.is_eq(mode, 6), ((noise(i, 5.0, seed) - 0.5) * 1.6 : F32),
                  (F32.sin(angle) * choose(U32.is_eq(mode, 5), 0.65, travel) + gravity * age * age : F32))))
      +r = (size * choose(U32.is_eq(kind, 11), (0.5 + age : F32), 1.0) : F32)
      h = (disc((x - px : F32), (y - py : F32), r, (r * 0.7 : F32)) * sat((1.0 - age / life : F32)) * choose(looping, sat((age * 8.0 : F32)), 1.0) : F32)
      motes(k, kind, x, y, t, speed, size, life, gravity, mode, seed, looping, F32.max(acc, h))

def glyph_pixel(bits: U32, +x: U32, +y: U32) -> F32:
  bit = ((bits >> U32.to_nat((y * 3 + x : U32))) .&. 1 : U32)
  U32.to_f32(bit)

def letters(xs: List<&2, U32>, +x: F32, +y: F32, +size: F32, +pos: F32) -> F32:
  match xs:
    case Nil{}:
      0.0
    case h <> t:
      +gx = ((x - pos) / size : F32)
      +gy = ((y + 0.2) / size : F32)
      here = choose((gx >= 0.0 : F32) && (gx < 3.0 : F32) && (gy >= 0.0 : F32) && (gy < 5.0 : F32),
                    glyph_pixel(h, F32.to_u32(F32.max(gx, 0.0)), F32.to_u32(F32.max(gy, 0.0))), 0.0)
      F32.max(here, letters(t, x, y, size, (pos + size * 4.0 : F32)))

def shape(kind: U32, +ps: List<&2, F32>, glyphs: List<&2, U32>, +x: F32, +y: F32,
          +t: F32, +seed: U32, +looping: Bool) -> F32:
  match kind:
    case 0:
      +r = F32.lerp(p(ps, 4n), p(ps, 5n), F32.sqrt(t))
      +rough = (1.0 + (noise(F32.floor((x * 18.0 : F32)), F32.floor((y * 18.0 : F32)), seed) - 0.5) * t * 0.7 : F32)
      (disc(x, y, (r * rough : F32), (r * 0.7 : F32)) * sat((1.5 - t * 1.5 : F32)) : F32)
    case 1:
      ring(F32.hypot(x, y), F32.lerp(p(ps, 4n), p(ps, 5n), F32.sqrt(t)), p(ps, 6n))
    case 2:
      +wave = (F32.sin((y * 13.0 - t * 6.2831853 * p(ps, 10n) + U32.to_f32((seed % 360 : U32)) : F32)) * 0.09 : F32)
      +width = (p(ps, 5n) * sat((0.65 + y * 0.5 : F32)) : F32)
      sat(((width - F32.abs((x + wave : F32))) / 0.16 : F32))
    case 3:
      +fade = sat((1.0 - F32.abs(x) / F32.max(p(ps, 5n), 0.01) : F32))
      (sat((1.0 - F32.abs(y) / F32.max(p(ps, 6n), 0.01) : F32)) * fade : F32)
    case 4:
      +outer = disc((x + 0.15 : F32), y, p(ps, 5n), p(ps, 6n))
      +inner = disc((x + 0.15 - p(ps, 6n) * 1.5 : F32), y, p(ps, 5n), 0.02)
      (F32.max((outer - inner : F32), 0.0) * sat((1.8 - t * 1.3 : F32)) : F32)
    case 5:
      disc(x, y, (p(ps, 5n) * (0.9 + 0.1 * F32.sin((t * 6.2831853 : F32))) : F32), p(ps, 6n))
    case 6:
      +head = disc((x - 0.4 : F32), y, p(ps, 6n), (p(ps, 6n) * 0.6 : F32))
      +tail = (sat((1.0 - F32.abs((y / (0.08 + (x + 1.0) * 0.08) : F32)) : F32)) * sat((x + 1.0 : F32)) : F32)
      F32.max(head, choose((x <= 0.4 : F32), (tail * 0.7 : F32), 0.0))
    case 7:
      +r = F32.hypot(x, y)
      +a = F32.atan2(y, x)
      +spiral = (0.5 + 0.5 * F32.sin((a * 4.0 + r * 18.0 - t * 6.2831853 * p(ps, 11n) : F32)) : F32)
      (ring(r, p(ps, 5n), (p(ps, 6n) * 2.0 : F32)) * (0.4 + spiral * 0.6) : F32)
    case 8:
      +width = (p(ps, 6n) * (0.8 + F32.sin((t * 6.2831853 : F32)) * 0.2) : F32)
      (sat((1.0 - F32.abs(x) / width : F32)) * sat((1.0 - F32.abs(y) / F32.max(p(ps, 5n), 0.01) : F32)) : F32)
    case 9:
      +a = F32.atan2(y, x)
      +r = F32.hypot(x, y)
      +spoke = F32.pow(F32.abs(F32.cos((a * p(ps, 12n) * 0.5 + t * p(ps, 11n) * 6.2831853 : F32))), 12.0)
      (spoke * sat((1.0 - r / p(ps, 5n) : F32)) : F32)
    case 10:
      +r = F32.hypot(x, y)
      +a = F32.atan2(y, x)
      +spokes = F32.pow(F32.abs(F32.sin((a * 6.0 + t * 6.2831853 * p(ps, 11n) : F32))), 16.0)
      F32.max(ring(r, p(ps, 5n), p(ps, 6n)), (ring(r, (p(ps, 5n) * 0.7 : F32), 0.025) * spokes : F32))
    case 11:
      motes(U32.to_nat(F32.to_u32(p(ps, 12n))), kind, x, y, t, p(ps, 10n), p(ps, 13n), p(ps, 14n), p(ps, 16n), F32.to_u32(p(ps, 15n)), seed, looping, 0.0)
    case 12:
      motes(U32.to_nat(F32.to_u32(p(ps, 12n))), kind, x, y, t, p(ps, 10n), p(ps, 13n), p(ps, 14n), p(ps, 16n), F32.to_u32(p(ps, 15n)), seed, looping, 0.0)
    case 13:
      +cell = F32.floor((y * 10.0 : F32))
      +zig = ((noise(cell, F32.floor((t * 8.0 : F32)), seed) - 0.5) * 0.6 : F32)
      (sat((1.0 - F32.abs((x - zig : F32)) / p(ps, 6n) : F32)) * sat((1.0 - F32.abs(y) : F32)) : F32)
    case 14:
      +r = F32.hypot(x, y)
      +a = (F32.atan2(y, x) + noise(F32.floor((r * 8.0 : F32)), 0.0, seed) * 0.35 : F32)
      +line = F32.pow(F32.abs(F32.cos((a * p(ps, 12n) * 0.5 : F32))), 60.0)
      (line * sat((1.0 - r / F32.max((t * p(ps, 5n) : F32), 0.01) : F32)) : F32)
    case 15:
      +cell = F32.floor(((x + 1.0) * p(ps, 12n) * 0.5 : F32))
      +cx = (frac(((x + 1.0) * p(ps, 12n) * 0.5 : F32)) - 0.5 : F32)
      +height = (noise(cell, 0.0, seed) * 0.7 + 0.2 : F32)
      choose((y <= 0.5 : F32), sat(((0.5 - F32.abs(cx) - (0.5 - y) / F32.max((height * F32.sqrt(t) : F32), 0.01) * 0.45) * 4.0 : F32)), 0.0)
    case 16:
      +r = F32.hypot(x, y)
      +a = F32.atan2(y, x)
      +edge = (p(ps, 5n) * (0.3 + 0.7 * F32.pow(F32.abs(F32.cos((a * p(ps, 12n) * 0.5 : F32))), 8.0)) : F32)
      sat(((edge - r) / p(ps, 6n) : F32))
    case 17:
      +band = F32.floor(((y + 1.0) * p(ps, 12n) : F32))
      +reach = (noise(band, 0.0, seed) * 0.7 + 0.2 : F32)
      (sat((1.0 - F32.abs((frac(((y + 1.0) * p(ps, 12n) : F32)) - 0.5 : F32)) / 0.3 : F32)) * sat((reach - F32.abs(x) : F32)) : F32)
    case 18:
      +r = F32.hypot(x, y)
      +rim = ring(r, p(ps, 5n), p(ps, 6n))
      F32.max(rim, (disc(x, y, p(ps, 5n), 0.1) * 0.15 : F32))
    case 19:
      (disc(x, y, p(ps, 5n), 0.02) * sat((1.0 - t / p(ps, 14n) : F32)) : F32)
    case 20:
      letters(glyphs, x, (y + t * p(ps, 10n) : F32), p(ps, 13n), (0.0 - p(ps, 4n) : F32))
    case 21:
      +width = (0.12 + (0.9 - y) * 0.35 : F32)
      +stripe = (0.65 + 0.35 * F32.sin((y * 25.0 + F32.asin(F32.clamp((x / width : F32), (0.0 - 1.0 : F32), 1.0)) * 3.0 - t * p(ps, 11n) * 6.2831853 : F32)) : F32)
      (sat(((width - F32.abs(x)) / 0.1 : F32)) * stripe : F32)
    case _:
      0.0

def layer_active(active: Bool, +kind: U32, +ps: List<&2, F32>, glyphs: List<&2, U32>,
                 +x: F32, +y: F32, +t: F32, seed: U32, +looping: Bool) -> F32:
  match active:
    case False{}:
      0.0
    case True{}:
      +a = ((p(ps, 7n) + choose(U32.is_eq(kind, 4), (t * 140.0 - 70.0 : F32), 0.0)) * 0.0174532925 : F32)
      +dx = (x - p(ps, 2n) : F32)
      +dy = ((y - p(ps, 3n)) / p(ps, 8n) : F32)
      rx = (dx * F32.cos(a) + dy * F32.sin(a) : F32)
      ry = (dy * F32.cos(a) - dx * F32.sin(a) : F32)
      (shape(kind, ps, glyphs, rx, ry, t, seed, looping) * p(ps, 9n) * choose(looping, 1.0, sat(((1.0 - t) * 4.0 : F32))) : F32)

def color_at(colors: List<&2, U32>, n: Nat) -> U32:
  word(List.get(&2, U32, colors, n))

def painted(visible: Bool, colors: List<&2, U32>, value: F32, total: F32) -> U32:
  match visible:
    case False{}:
      0
    case True{}:
      (color_at(colors, U32.to_nat(F32.to_u32(F32.clamp(value, 0.0, (total - 1.0 : F32))))) .|. 4278190080 : U32)

def sample(layers: List<&2, Layer>, +x: F32, +y: F32, +t: F32,
           +seed: U32, +looping: Bool, +dither: F32, previous: U32) -> U32:
  match layers:
    case Nil{}:
      previous
    case Layer{kind, +colors, +ps, glyphs} <> rest:
      +local = sat(((t - p(ps, 0n)) / (p(ps, 1n) - p(ps, 0n)) : F32))
      +h = layer_active((t >= p(ps, 0n) : F32) && (t <= p(ps, 1n) : F32), kind, ps, glyphs, x, y, local, seed, looping)
      +count = F32.from_nat(List.length(&2, U32, colors))
      +c = painted((h > 0.07 : F32), colors, (sat(h) * (count - 1.0) + dither : F32), count)
      next = Bool.pick(U32, U32.is_eq(c, 0), previous, c)
      sample(rest, x, y, t, seed, looping, dither, next)

def finish(+outline: U32, clean: Bool, +c: U32, +layers: List<&2, Layer>,
           +x: F32, +y: F32, +t: F32, +seed: U32, +looping: Bool, +d: F32, +step: F32) -> U32:
  match outline clean:
    case 0 False{}:
      c
    case _ _:
      +a = sample(layers, (x - step : F32), y, t, seed, looping, d, 0)
      +b = sample(layers, (x + step : F32), y, t, seed, looping, d, 0)
      +e = sample(layers, x, (y - step : F32), t, seed, looping, d, 0)
      +f = sample(layers, x, (y + step : F32), t, seed, looping, d, 0)
      +any = U32.is_ne(a, 0) || U32.is_ne(b, 0) || U32.is_ne(e, 0) || U32.is_ne(f, 0)
      +all = U32.is_ne(a, 0) && U32.is_ne(b, 0) && U32.is_ne(e, 0) && U32.is_ne(f, 0)
      +edge = (U32.is_eq(outline, 1) && U32.is_eq(c, 0) && any) || (U32.is_eq(outline, 2) && U32.is_ne(c, 0) && Bool.not(all))
      kept = Bool.pick(U32, clean && Bool.not(any), 0, c)
      Bool.pick(U32, edge, 4279834656, kept)

def shaded(+s: Scene, +x: F32, +y: F32, +t: F32) -> U32:
  Scene{+w, +h, frames, +seed, +looping, +outline, +dither, +clean, +layers} = s
  +step = (2.0 / U32.to_f32(U32.min(w, h)) : F32)
  +d = ((noise(F32.floor((x / step : F32)), F32.floor((y / step : F32)), 0) - 0.5) * dither : F32)
  +c = sample(layers, x, y, t, seed, looping, d, 0)
  finish(outline, clean, c, layers, x, y, t, seed, looping, d, step)

def pixel(+s: Scene, +i: U32, t: F32) -> U32:
  Scene{+w, +h, frames, seed, looping, outline, dither, clean, layers} = s
  +scale = U32.to_f32(U32.min(w, h))
  x = ((U32.to_f32((i % w : U32)) + 0.5 - U32.to_f32(w) * 0.5) * 2.0 / scale : F32)
  y = ((U32.to_f32((i / w : U32)) + 0.5 - U32.to_f32(h) * 0.5) * 2.0 / scale : F32)
  shaded(s, x, y, t)

def pixels(n: Nat, +s: Scene, +i: U32, +t: F32, bytes: List<&2, U32>) -> List<&2, U32>:
  match n:
    case 0n:
      List.reverse(&2, U32, bytes)
    case Succ{+k}:
      +c = pixel(s, i, t)
      a = ((c >> 24n) .&. 255 : U32)
      b = ((c >> 16n) .&. 255 : U32)
      g = ((c >> 8n) .&. 255 : U32)
      r = (c .&. 255 : U32)
      pixels(k, s, (i + 1 : U32), t, a <> b <> g <> r <> bytes)

# Balanced, independent pixel chunks run on the native CPU fork/join scheduler.
def parallel_pixels(depth: Nat, +s: Scene, +n: U32, +i: U32, +t: F32) -> List<&2, U32>:
  match depth:
    case 0n:
      pixels(U32.to_nat(n), s, i, t, Nil{})
    case Succ{+k}:
      +half = (n / 2 : U32)
      left right = parallel_pixels(k, s, half, i, t) parallel_pixels(k, s, (n - half : U32), (i + half : U32), t)
      List.append(&2, U32, left, right)

def pass_write(pair: File & Result<&1, &1, U32 & String, Unit>, cont: File -> IO(Unit)) -> IO(Unit):
  (next, result) = pair
  do IO<Unit>:
    IO.pass(Unit, result)
    cont(next)

def write_frames(n: Nat, +s: Scene, +frame: U32, file: File) -> IO(Unit):
  match n:
    case 0n:
      File.close(file)
    case Succ{+k}:
      Scene{+w, +h, +frames, seed, +looping, outline, dither, clean, layers} = s
      +denom = Bool.pick(U32, looping, frames, U32.max((frames - 1 : U32), 1))
      data = parallel_pixels(2n, s, (w * h : U32), 0, (U32.to_f32(frame) / U32.to_f32(denom) : F32))
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write_bytes(file, data), pair => pass_write(pair, next => write_frames(k, s, (frame + 1 : U32), next)))

def render_file(+scene: Scene, output: File) -> IO(Unit):
  Scene{w, h, frames, seed, looping, outline, dither, clean, layers} = scene
  write_frames(U32.to_nat(frames), scene, 0, output)

def read_result(pair: File & Result<&1, &1, U32 & String, String>, output_path: String) -> IO(Unit):
  (input, result) = pair
  do IO<Unit>:
    source : String <- IO.pass(String, result)
    File.close(input)
    scene : Scene = read_scene(String.split(source, '\n'))
    output : File <- IO.try(File, File.open(output_path, "w"))
    render_file(scene, output)

def run(args: List<String>) -> IO(Unit):
  match args:
    case prog <> input_path <> output_path <> rest:
      do IO<Unit>:
        input : File <- IO.try(File, File.open(input_path, "r"))
        pair : File & Result<&1, &1, U32 & String, String> <- File.read(input, 262144)
        read_result(pair, output_path)
    case _:
      IO.die(Unit, 2, "usage: vfx-render <wire-input> <rgba-output>")

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- IO.args()
    run(args)
