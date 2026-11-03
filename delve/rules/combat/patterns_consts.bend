# GENERATED from rules/patterns.rs: the emitters' tuple consts.

# Emitter numbers: (shots at no extra thickness, shots per step of thickness, speed).

def patterns.BURST_0() -> U32:
  3

def patterns.BURST_1() -> U32:
  1

def patterns.BURST_2() -> F32:
  8.0

def patterns.LEAD_0() -> U32:
  3

def patterns.LEAD_1() -> U32:
  2

def patterns.LEAD_2() -> F32:
  7.0

def patterns.GAP_RING_0() -> U32:
  20

def patterns.GAP_RING_1() -> U32:
  4

def patterns.GAP_RING_2() -> F32:
  4.5

def patterns.FLOWER_0() -> U32:
  6

def patterns.FLOWER_1() -> U32:
  1

def patterns.FLOWER_2() -> F32:
  4.0

def patterns.TWIN_SPIRAL_0() -> U32:
  7

def patterns.TWIN_SPIRAL_1() -> U32:
  1

def patterns.TWIN_SPIRAL_2() -> F32:
  5.0

def patterns.WEAVE_0() -> U32:
  6

def patterns.WEAVE_1() -> U32:
  2

def patterns.WEAVE_2() -> F32:
  6.0

def patterns.CURVE_0() -> U32:
  6

def patterns.CURVE_1() -> U32:
  2

def patterns.CURVE_2() -> F32:
  6.5

def patterns.SURGE_0() -> U32:
  12

def patterns.SURGE_1() -> U32:
  3

def patterns.SURGE_2() -> F32:
  1.2

def patterns.REAIM_0() -> U32:
  8

def patterns.REAIM_1() -> U32:
  2

def patterns.REAIM_2() -> F32:
  7.0

def patterns.SPLIT_0() -> U32:
  1

def patterns.SPLIT_1() -> U32:
  1

def patterns.SPLIT_2() -> F32:
  5.0

def patterns.BOUNCE_0() -> U32:
  3

def patterns.BOUNCE_1() -> U32:
  1

def patterns.BOUNCE_2() -> F32:
  7.0

def patterns.MINES_0() -> U32:
  4

def patterns.MINES_1() -> U32:
  1

def patterns.MINES_2() -> F32:
  8.0

def patterns.GAP_WALL_0() -> U32:
  13

def patterns.GAP_WALL_1() -> U32:
  2

def patterns.GAP_WALL_2() -> F32:
  5.0

def patterns.RAIN_0() -> U32:
  8

def patterns.RAIN_1() -> U32:
  3

def patterns.RAIN_2() -> F32:
  5.0

def patterns.SEEKER_0() -> U32:
  2

def patterns.SEEKER_1() -> U32:
  1

def patterns.SEEKER_2() -> F32:
  3.5

def patterns.DARTS_0() -> U32:
  3

def patterns.DARTS_1() -> U32:
  1

def patterns.DARTS_2() -> F32:
  patterns.SHOT_TOP()

def patterns.ORBS_0() -> U32:
  1

def patterns.ORBS_1() -> U32:
  1

def patterns.ORBS_2() -> F32:
  2.6

def patterns.LOBS_0() -> U32:
  1

def patterns.LOBS_1() -> U32:
  1

def patterns.LOBS_2() -> F32:
  0.0

def patterns.DRIFT_0() -> U32:
  3

def patterns.DRIFT_1() -> U32:
  1

def patterns.DRIFT_2() -> F32:
  10.0

# CROSS_LEFT / CROSS_RIGHT: rows off the knight's row.
def patterns.CROSS_LEFT() -> List<&2, F32>:
  [F32.neg(3.0), 0.0, 3.0]

def patterns.CROSS_RIGHT() -> List<&2, F32>:
  [F32.neg(1.5), 1.5]
