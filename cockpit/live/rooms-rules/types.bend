import Base

type Quad is Data:
  Quad{w: U32, h: U32, rows: List<&2, String>}

type Art is Data:
  Art{quads: List<&2, Quad>}

type Size is Data:
  Size{w: U32, h: U32}

type Consts is Data:
  Consts{sizes: List<&2, Size>, share: U32, least_floor: U32}
