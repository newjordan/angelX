//! Choosing the game's stairs: one link rect per pair of neighbouring
//! levels, an arrival point beside each, and the flood fill that keeps a
//! level's floor one region.

/// x, y, w, h in level coordinates.
pub type Rect = (i32, i32, i32, i32);

pub fn overlaps(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub fn grow(r: Rect, by: i32) -> Rect {
    (r.0 - by, r.1 - by, r.2 + 2 * by, r.3 + 2 * by)
}

pub fn cells(r: Rect) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for y in r.1..r.1 + r.3 {
        for x in r.0..r.0 + r.2 {
            v.push((x, y));
        }
    }
    v
}

pub fn walkable(c: u8) -> bool {
    matches!(c, b'.' | b'D' | b'>')
}

/// The two ends of a staircase, split along its longer side (a 5-wide
/// shaft gives two 2-wide halves with a gap between). None for 1x1.
pub fn halves(r: Rect) -> Option<[Rect; 2]> {
    let (x, y, w, h) = r;
    if w >= h {
        if w < 2 {
            return None;
        }
        let a = w / 2;
        Some([(x, y, a, h), (x + w - a, y, a, h)])
    } else {
        let a = h / 2;
        Some([(x, y, w, a), (x, y + h - a, w, a)])
    }
}

/// 4-connected groups of true cells, in scan order.
pub fn components(mask: &[Vec<bool>]) -> Vec<Vec<(i32, i32)>> {
    let h = mask.len();
    let w = mask.first().map_or(0, |r| r.len());
    let mut seen = vec![vec![false; w]; h];
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !mask[y][x] || seen[y][x] {
                continue;
            }
            let mut comp = Vec::new();
            let mut stack = vec![(x, y)];
            seen[y][x] = true;
            while let Some((cx, cy)) = stack.pop() {
                comp.push((cx as i32, cy as i32));
                let n = [(cx.wrapping_sub(1), cy), (cx + 1, cy), (cx, cy.wrapping_sub(1)), (cx, cy + 1)];
                for (nx, ny) in n {
                    if nx < w && ny < h && mask[ny][nx] && !seen[ny][nx] {
                        seen[ny][nx] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            comp.sort_by_key(|&(x, y)| (y, x));
            out.push(comp);
        }
    }
    out
}

pub fn bbox(c: &[(i32, i32)]) -> Rect {
    let x0 = c.iter().map(|p| p.0).min().unwrap_or(0);
    let x1 = c.iter().map(|p| p.0).max().unwrap_or(0);
    let y0 = c.iter().map(|p| p.1).min().unwrap_or(0);
    let y1 = c.iter().map(|p| p.1).max().unwrap_or(0);
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// Every tile of `r` is plain floor and off the border.
pub fn all_floor(tiles: &[Vec<u8>], r: Rect) -> bool {
    let h = tiles.len() as i32;
    let w = tiles.first().map_or(0, |t| t.len()) as i32;
    if r.0 < 1 || r.1 < 1 || r.0 + r.2 > w - 1 || r.1 + r.3 > h - 1 {
        return false;
    }
    cells(r).into_iter().all(|(x, y)| tiles[y as usize][x as usize] == b'.')
}

/// The staircase to use from a mask of stair tiles: the largest flight,
/// nearest the middle; of it the half for pair `k` (alternating, so a level
/// between two others gets distinct up and down ends), else the other half,
/// else the whole flight.
pub fn pick(mask: &[Vec<bool>], k: usize, center: (i32, i32), ok: impl Fn(Rect) -> bool) -> Option<Rect> {
    let mut comps = components(mask);
    let d2 = |r: Rect| {
        let (cx, cy) = (2 * r.0 + r.2 - 2 * center.0, 2 * r.1 + r.3 - 2 * center.1);
        cx * cx + cy * cy
    };
    comps.sort_by(|a, b| {
        b.len().cmp(&a.len()).then(d2(bbox(a)).cmp(&d2(bbox(b)))).then(a[0].1.cmp(&b[0].1)).then(a[0].0.cmp(&b[0].0))
    });
    for c in comps {
        let r = bbox(&c);
        let mut cands = Vec::new();
        if let Some(h) = halves(r) {
            cands.push(h[k % 2]);
            cands.push(h[1 - k % 2]);
        }
        cands.push(r);
        // Machinery or a statue may stand in the flight: then any 2x2 (or
        // 1x2) of it, nearest the preferred end.
        let want = halves(r).map(|h| h[k % 2]).unwrap_or(r);
        let mut small = Vec::new();
        for (w, h) in [(2, 2), (2, 1), (1, 2)] {
            for y in r.1..=r.1 + r.3 - h {
                for x in r.0..=r.0 + r.2 - w {
                    small.push((x, y, w, h));
                }
            }
        }
        let dw = |q: &Rect| {
            let (dx, dy) = (2 * q.0 + q.2 - 2 * want.0 - want.2, 2 * q.1 + q.3 - 2 * want.1 - want.3);
            (-(q.2 * q.3), dx * dx + dy * dy, q.1, q.0)
        };
        small.sort_by_key(dw);
        cands.extend(small);
        for cand in cands {
            // A half of a ragged flight may hold some non-stair tiles.
            let on = cells(cand).iter().filter(|&&(x, y)| mask[y as usize][x as usize]).count();
            if 2 * on >= (cand.2 * cand.3) as usize && ok(cand) {
                return Some(cand);
            }
        }
    }
    None
}

/// The nearest block of the given size to `anchor` that is all plain floor
/// with nothing standing on it, clear of `avoid` by `margin`, and (when
/// `reach` is given) on tiles already reached.
pub fn free_block(
    tiles: &[Vec<u8>],
    dress: &[Vec<u8>],
    size: (i32, i32),
    anchor: (i32, i32),
    avoid: &[Rect],
    margin: i32,
    reach: Option<&[Vec<bool>]>,
) -> Option<Rect> {
    let h = tiles.len() as i32;
    let w = tiles.first().map_or(0, |t| t.len()) as i32;
    let mut best: Option<(i32, Rect)> = None;
    for y in 1..h - size.1 {
        for x in 1..w - size.0 {
            let r = (x, y, size.0, size.1);
            if !all_floor(tiles, r) {
                continue;
            }
            if cells(r).iter().any(|&(cx, cy)| dress[cy as usize][cx as usize] != b' ') {
                continue;
            }
            if let Some(re) = reach
                && cells(r).iter().any(|&(cx, cy)| !re[cy as usize][cx as usize]) {
                    continue;
                }
            if avoid.iter().any(|a| overlaps(grow(*a, margin), r)) {
                continue;
            }
            let (dx, dy) = (x + size.0 / 2 - anchor.0, y + size.1 / 2 - anchor.1);
            let d = dx * dx + dy * dy;
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, r));
            }
        }
    }
    best.map(|b| b.1)
}

/// Which tiles walking reaches from the seeds.
pub fn flood(tiles: &[Vec<u8>], seeds: &[(i32, i32)]) -> Vec<Vec<bool>> {
    let h = tiles.len();
    let w = tiles.first().map_or(0, |t| t.len());
    let mut seen = vec![vec![false; w]; h];
    let mut q = std::collections::VecDeque::new();
    for &(x, y) in seeds {
        let (x, y) = (x as usize, y as usize);
        if y < h && x < w && walkable(tiles[y][x]) && !seen[y][x] {
            seen[y][x] = true;
            q.push_back((x, y));
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let n = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)];
        for (nx, ny) in n {
            if nx < w && ny < h && !seen[ny][nx] && walkable(tiles[ny][nx]) {
                seen[ny][nx] = true;
                q.push_back((nx, ny));
            }
        }
    }
    seen
}

/// Where a party arriving by `r` stands: the nearest walkable tile, by
/// walking, with five plain floor tiles in a row centred on it (the fan of
/// four knights and their leader), none of them inside a link.
pub fn arrive(tiles: &[Vec<u8>], r: Rect, rects: &[Rect]) -> Option<(i32, i32)> {
    let h = tiles.len() as i32;
    let w = tiles.first().map_or(0, |t| t.len()) as i32;
    let in_rect = |x: i32, y: i32| rects.iter().any(|q| overlaps(*q, (x, y, 1, 1)));
    let good = |x: i32, y: i32| {
        (x - 2..=x + 2).all(|xx| xx >= 0 && xx < w && tiles[y as usize][xx as usize] == b'.' && !in_rect(xx, y))
    };
    let mut seen = vec![vec![false; w as usize]; h as usize];
    let mut q = std::collections::VecDeque::new();
    for (x, y) in cells(r) {
        seen[y as usize][x as usize] = true;
        q.push_back((x, y));
    }
    while let Some((x, y)) = q.pop_front() {
        if good(x, y) {
            return Some((x, y));
        }
        for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if nx >= 0 && ny >= 0 && nx < w && ny < h && !seen[ny as usize][nx as usize] && walkable(tiles[ny as usize][nx as usize]) {
                seen[ny as usize][nx as usize] = true;
                q.push_back((nx, ny));
            }
        }
    }
    None
}
