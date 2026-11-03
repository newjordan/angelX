# Types the games_c fragments share. Kept first so patterns see their constructors.

# A tile rectangle: column, row, width, height.
type GcRect is Data:
  GcRect{x: U32, y: U32, w: U32, h: U32}


# A point.
type GcPt is Data:
  GcPt{x: F32, y: F32}

# A run and the nook being ticked (the nook is taken out of the run for the tick).
type GcRN is Data:
  GcRN{run: J.Val, nook: J.Val}

# A target of an area blow: distance, knight id, and the way from the blow to it.
type GcTgt is Data:
  GcTgt{d: F32, id: U32, x: F32, y: F32}

# A flying round: the session, the children spawned, and the shots kept.
type GcFly is Data:
  GcFly{s: J.Val, children: List<&2, J.Val>, out: List<&2, J.Val>}

# One shot's turn: the session, the shot, the children.
type GcShot is Data:
  GcShot{s: J.Val, p: J.Val, children: List<&2, J.Val>}

# An orbit point with its owner.
type GcOrb is Data:
  GcOrb{id: U32, x: F32, y: F32}

# A hit lined up along a shot's step: where along it, the knight, whether the shield caught it.
type GcHit is Data:
  GcHit{t: F32, id: U32, blocked: Bool}

# A mossbell score key: points (more first), drains, the jackpot tick.
type GcSc is Data:
  GcSc{p: U32, d: U32, j: W.W}

# A track piece (race3d::Piece): length, turn (degrees), drop, width, walls, ground code, lane mask.
type GcPiece is Data:
  GcPiece{len: U32, turn: F32, drop: F32, width: F32, walls: Bool, ground: U32, lanes: U32}

# A track sample (race3d::Sample). Ground: 0 Floor 1 Bridge 2 Crossing 3 Lodestone 4 Sluice 5 Collapse 6 Sorter 7 Rails 8 Trestle 9 Gorge.
type GcSamp is Data:
  GcSamp{px: F32, py: F32, pz: F32, yaw: F32, slope: F32, bend: F32, width: F32, walls: Bool, ground: U32, lanes: U32, funnel: Bool}

# An item on the track: s, x, z, kind (0 Gem, 1 Ring, 2 Gargoyle, 3 Timber).
type GcItem is Data:
  GcItem{s: F32, x: F32, z: F32, kind: U32}

type GcBlock is Data:
  GcBlock{s: F32, len: F32, x0: F32, x1: F32, z0: F32, z1: F32}

type GcGap is Data:
  GcGap{lane: U32, s0: F32, s1: F32}

type GcTrack is Data:
  GcTrack{looped: Bool, samples: List<&2, GcSamp>, finish: F32, items: List<&2, GcItem>, blocks: List<&2, GcBlock>, gaps: List<&2, GcGap>, posts: List<&2, F32>}

# lay()'s result: samples and posts in order.
type GcLay is Data:
  GcLay{samples: List<&2, GcSamp>, posts: List<&2, F32>}

# lay()'s running state: position, heading, samples and posts so far (newest first), the sample count.
type GcLS is Data:
  GcLS{x: F32, y: F32, z: F32, yaw: F32, samples: List<&2, GcSamp>, posts: List<&2, F32>, count: U32}

# A race tick's working set: the race, the rider being moved, the sounds so far.
type GcRd is Data:
  GcRd{race: J.Val, rider: J.Val, noise: List<&2, String>}

# A rider and the sounds so far.
type GcWr is Data:
  GcWr{rider: J.Val, noise: List<&2, String>}
