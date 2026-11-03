import Base
import ./terrain.bend as T
import ./level.bend as L

# What the rules read off the road. (`Road` itself is laid out in level.bend.)

def ro_paths(+r: L.Road) -> List<&2, T.Path>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      paths

def ro_foes(+r: L.Road) -> List<&2, L.Fs>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      foes

def ro_sparks(+r: L.Road) -> List<&2, L.Sp>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      sparks

def ro_springs(+r: L.Road) -> List<&2, L.Spr>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      springs

def ro_pads(+r: L.Road) -> List<&2, L.Pad>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      pads

def ro_posts(+r: L.Road) -> List<&2, L.Post>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      posts

def ro_crates(+r: L.Road) -> List<&2, L.Crate>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      crates

def ro_signs(+r: L.Road) -> List<&2, L.Sign>:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      signs

def ro_width(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      width

def ro_dead_y(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      dead_y

def ro_a0(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      a0

def ro_a1(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      a1

def ro_a2(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      a2

def ro_a3(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      a3

def ro_main(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      main

def ro_start_path(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      start_path

def ro_start_s(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      start_s

def ro_bell_x(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      bell_x

def ro_bell_y(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      bell_y

def ro_par(+r: L.Road) -> U32:
  match r:
    case L.Road{paths, foes, sparks, springs, pads, posts, crates, signs, width, dead_y, a0, a1, a2, a3, main, start_path, start_s, bell_x, bell_y, par}:
      par

