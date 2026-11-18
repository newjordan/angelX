type GcRect is Data:
  GcRect{x: U32, y: U32, w: U32, h: U32}

type GcPt is Data:
  GcPt{x: F32, y: F32}

type GcRN is Data:
  GcRN{run: J.Val, nook: J.Val}

type GcTgt is Data:
  GcTgt{d: F32, id: U32, x: F32, y: F32}

type GcFly is Data:
  GcFly{s: J.Val, children: List<&2, J.Val>, out: List<&2, J.Val>}

type GcShot is Data:
  GcShot{s: J.Val, p: J.Val, children: List<&2, J.Val>}

type GcOrb is Data:
  GcOrb{id: U32, x: F32, y: F32}

type GcHit is Data:
  GcHit{t: F32, id: U32, blocked: Bool}

type GcSc is Data:
  GcSc{p: U32, d: U32, j: W.W}

type GcPiece is Data:
  GcPiece{len: U32, turn: F32, drop: F32, width: F32, walls: Bool, ground: U32, lanes: U32}

type GcSamp is Data:
  GcSamp{px: F32, py: F32, pz: F32, yaw: F32, slope: F32, bend: F32, width: F32, walls: Bool, ground: U32, lanes: U32, funnel: Bool}

type GcItem is Data:
  GcItem{s: F32, x: F32, z: F32, kind: U32}

type GcBlock is Data:
  GcBlock{s: F32, len: F32, x0: F32, x1: F32, z0: F32, z1: F32}

type GcGap is Data:
  GcGap{lane: U32, s0: F32, s1: F32}

type GcTrack is Data:
  GcTrack{looped: Bool, samples: List<&2, GcSamp>, finish: F32, items: List<&2, GcItem>, blocks: List<&2, GcBlock>, gaps: List<&2, GcGap>, posts: List<&2, F32>}

type GcLay is Data:
  GcLay{samples: List<&2, GcSamp>, posts: List<&2, F32>}

type GcLS is Data:
  GcLS{x: F32, y: F32, z: F32, yaw: F32, samples: List<&2, GcSamp>, posts: List<&2, F32>, count: U32}

type GcRd is Data:
  GcRd{race: J.Val, rider: J.Val, noise: List<&2, String>}

type GcWr is Data:
  GcWr{rider: J.Val, noise: List<&2, String>}
