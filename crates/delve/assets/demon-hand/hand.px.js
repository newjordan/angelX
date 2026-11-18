use("inks.hex")
ramps("nb87@", "Xgjh")

const DROP = 0.45
const SIZES = [20, 28, 38]
const PALM = 0.9, ABOVE = 1.3, FOREARM = 1.7
const FINGERS = [
  { x: -0.38, y: 0.80, a: -0.42, len: 0.80 },
  { x: -0.13, y: 0.88, a: -0.14, len: 0.96 },
  { x: 0.12, y: 0.90, a: 0.12, len: 1.0 },
  { x: 0.36, y: 0.85, a: 0.36, len: 0.92 },
]
const BONES = [0.42, 0.3, 0.22]
const BEND = [85, 180, 196]
const POSES = [
  { spread: 1.0, curl: 0, flex: 0.1 },
  { spread: 1.1, curl: 0, flex: -0.05 },
  { spread: 0.95, curl: 0, flex: 0.22 },
  { spread: 1.25, curl: -0.06, flex: 0 },
  { spread: 1.1, curl: 0.3, flex: 0 },
  { spread: 0.9, curl: 0.6, flex: 0 },
  { spread: 0.7, curl: 0.85, flex: 0 },
  { spread: 0.6, curl: 1.0, flex: 0 },
  { spread: 0.55, curl: 1.06, flex: 0 },
  { spread: 0.6, curl: 1.0, flex: 0.04 },
]
const RAD = Math.PI / 180

function finger(f, { spread, curl, flex }) {
  const a = f.a * spread * (1 - 0.5 * Math.max(0, curl))
  let p = [f.x, f.y, 0]
  const out = [p]
  const turns = BEND.map((b, j) => (b * curl + [12, 30, 55][j] * flex) * RAD)
  for (let j = 0; j < 4; j++) {
    const phi = j < 3 ? turns[j] : turns[2] + (20 + 30 * flex) * RAD
    const len = (j < 3 ? BONES[j] : 0.34) * f.len
    p = [p[0] + Math.sin(a) * Math.cos(phi) * len, p[1] + Math.cos(a) * Math.cos(phi) * len, p[2] + Math.sin(phi) * len]
    out.push(p)
  }
  return out
}

function thumb({ curl, flex }) {
  const c = Math.max(0, curl)
  const base = [0.42, 0.28, 0]
  const a = (55 - 115 * c) * RAD, phi = (10 + 70 * c + 15 * flex) * RAD
  const mid = [base[0] + Math.sin(a) * Math.cos(phi) * 0.38, base[1] + Math.cos(a) * Math.cos(phi) * 0.38, Math.sin(phi) * 0.38]
  const a2 = a - 0.5 * c, phi2 = phi + 30 * RAD
  const tip = [mid[0] + Math.sin(a2) * Math.cos(phi2) * 0.28, mid[1] + Math.cos(a2) * Math.cos(phi2) * 0.28, mid[2] + Math.sin(phi2) * 0.28]
  const claw = [tip[0] + Math.sin(a2) * 0.28, tip[1] + Math.cos(a2) * Math.cos(phi2 + 0.5) * 0.28, tip[2] + Math.sin(phi2 + 0.5) * 0.28]
  return [base, mid, tip, claw]
}

const onto = (S, wx, wy) => ([x, y, z]) => [wx + x * S, wy - y * S + z * DROP * S]

function bone(c, [x0, y0], [x1, y1], r0, r1, outline) {
  if (outline) c.capsule(x0, y0, r0 + 1, x1, y1, r1 + 1, "k")
  else c.capsule(x0, y0, r0, x1, y1, r1, "nb87@", { light: [0.35, 1] })
}
function claw(c, [x0, y0], [x1, y1], r0, outline) {
  if (outline) c.capsule(x0, y0, r0 + 1, x1, y1, 1, "k")
  else c.capsule(x0, y0, r0, x1, y1, 0.3, "Xgjh", { light: [0.35, 1] })
}

function digits(S, pose, at) {
  const out = []
  const r = Math.max(2, 0.13 * S)
  for (const f of FINGERS) {
    const j = finger(f, pose)
    for (let k = 0; k < 3; k++) {
      const rr0 = r * (k === 0 ? 1.05 : 1 - 0.1 * k) * (f.len < 0.85 ? 0.85 : 1)
      out.push([at(j[k]), at(j[k + 1]), rr0, rr0 * 0.78, (j[k][2] + j[k + 1][2]) / 2, false])
    }
    out.push([at(j[3]), at(j[4]), Math.max(1.6, r * 0.75), 0.3, j[3][2] + 0.05, true])
  }
  const t = thumb(pose)
  out.push([at(t[0]), at(t[1]), r * 1.25, r * 1.05, t[1][2] / 2 + 0.02, false])
  out.push([at(t[1]), at(t[2]), r * 1.05, r * 0.9, (t[1][2] + t[2][2]) / 2 + 0.02, false])
  out.push([at(t[2]), at(t[3]), Math.max(1.6, r * 0.75), 0.3, t[2][2] + 0.05, true])
  return out
}

const NEAR = 0.14

for (const S of SIZES) {
  const W = Math.round(2.4 * S) + 4
  const wy = Math.round((PALM + ABOVE) * S) + 2
  const H = wy + Math.round(FOREARM * S)
  const wx = Math.floor(W / 2)
  const at = onto(S, wx, wy)
  const R = Math.round

  anim(`hand_back_${S}`, { w: W, h: H, frames: POSES.length, fps: 8, outline: "k" }, (c) => {
    const pose = POSES[c.frame]
    const fw = 0.34 * S, fb = 0.46 * S
    for (let i = 0; i < 4; i++) {
      const y = wy + (0.35 + i * 0.32) * S, half = fw + (fb - fw) * ((y - wy) / (FOREARM * S))
      c.poly([R(wx - half), R(y - 1), R(wx - half - 0.16 * S), R(y + 0.12 * S), R(wx - half + 1), R(y + 0.16 * S)], "X")
    }
    c.capsule(wx, wy + 1, fw + 1, wx, H + fb, fb + 1, "k")
    c.capsule(wx, wy + 1, fw, wx, H + fb, fb, "nb87@", { light: [0.25, 1], rim: false })
    for (const [v, seed] of [[-0.16, 1], [0.1, 2], [0.24, 3]]) {
      let px = wx + v * S, py = wy + 0.3 * S + seed * 2
      for (let n = 0; py < H; n++) {
        const nx = px + Math.sin(n * 1.7 + seed * 2.3) * 1.6, ny = py + 3
        const low = (ny - wy) / (FOREARM * S)
        if ((n + seed) % 4 !== 3) c.line(R(px), R(py), R(nx), R(ny), low > 0.55 && (n + c.frame) % 3 === 0 ? "@" : "7")
        px = nx; py = ny
      }
    }
    const palm = [[-0.36, 0.02], [-0.47, 0.42], [-0.45, 0.8], [-0.2, 0.92], [0.18, 0.95], [0.42, 0.85], [0.5, 0.45], [0.38, 0.05]].map(([x, y]) => at([x, y, 0]))
    c.poly(palm.flat().map(R), "8")
    c.relight(null, { light: [0.2, 1], reach: 2, ramps: ["nb87@"] })
    const crease = (pts) => { for (let i = 0; i + 1 < pts.length; i++) { const [a, b] = [at(pts[i]), at(pts[i + 1])]; c.line(R(a[0]), R(a[1]), R(b[0]), R(b[1]), "n") } }
    crease([[-0.38, 0.62, 0], [-0.05, 0.66, 0], [0.3, 0.58, 0]])
    crease([[0.32, 0.2, 0], [0.12, 0.42, 0], [0.05, 0.7, 0]])
    const far = digits(S, pose, at).filter((d) => d[4] <= NEAR).sort((a, b) => a[4] - b[4])
    for (const [p, q, r0, r1, , isClaw] of far) {
      if (isClaw) { claw(c, p, q, r0, true); claw(c, p, q, r0, false) }
      else { bone(c, p, q, r0, r1, true); bone(c, p, q, r0, r1, false) }
    }
  })

  anim(`hand_front_${S}`, { w: W, h: H, frames: POSES.length, fps: 8, outline: "k" }, (c) => {
    const pose = POSES[c.frame]
    const near = digits(S, pose, at).filter((d) => d[4] > NEAR).sort((a, b) => a[4] - b[4])
    for (const [p, q, r0, r1, , isClaw] of near) {
      if (isClaw) { claw(c, p, q, r0, true); claw(c, p, q, r0, false) }
      else { bone(c, p, q, r0, r1, true); bone(c, p, q, r0, r1, false) }
    }
  })

  anim(`hand_${S}`, { w: W, h: H, frames: POSES.length, fps: 8 },
    (c) => { c.stamp(`hand_back_${S}`, 0, 0, { frame: c.frame }); c.stamp(`hand_front_${S}`, 0, 0, { frame: c.frame }) })
}
