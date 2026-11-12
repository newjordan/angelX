//! The `levels/<slug>.txt` format:
//!
//! ```text
//! LEVEL <slug>
//! NAME <Display Name>
//! Z <df z>
//! SIZE <w> <h>
//! ORIGIN <df x0> <df y0>
//! TILES
//! <h rows of w chars>
//! DRESS
//! <h rows>
//! ROCK
//! <h rows>
//! ```

pub const TILE_GLYPHS: &[u8] = b".#B~D>";
pub const DRESS_GLYPHS: &[u8] = b" btcskKhar WFfpPgmwoCdnT\"=xAj+";
pub const ROCK_GLYPHS: &[u8] = b" srfictgvPnkjJxao";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelFile {
    pub slug: String,
    pub name: String,
    pub z: i32,
    pub w: usize,
    pub h: usize,
    pub origin: (i32, i32),
    pub tiles: Vec<String>,
    pub dress: Vec<String>,
    pub rock: Vec<String>,
}

impl LevelFile {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("LEVEL {}\n", self.slug));
        s.push_str(&format!("NAME {}\n", self.name));
        s.push_str(&format!("Z {}\n", self.z));
        s.push_str(&format!("SIZE {} {}\n", self.w, self.h));
        s.push_str(&format!("ORIGIN {} {}\n", self.origin.0, self.origin.1));
        for (head, rows) in [("TILES", &self.tiles), ("DRESS", &self.dress), ("ROCK", &self.rock)] {
            s.push_str(head);
            s.push('\n');
            for r in rows {
                s.push_str(r);
                s.push('\n');
            }
        }
        s
    }

    pub fn parse(text: &str) -> Result<LevelFile, String> {
        let lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
        let field = |i: usize, key: &str| -> Result<String, String> {
            let l = lines.get(i).ok_or_else(|| format!("missing {key}"))?;
            l.strip_prefix(key)
                .and_then(|r| r.strip_prefix(' '))
                .map(|r| r.to_string())
                .ok_or_else(|| format!("line {}: expected {key}, got {l:?}", i + 1))
        };
        let slug = field(0, "LEVEL")?;
        let name = field(1, "NAME")?;
        let z = field(2, "Z")?.trim().parse::<i32>().map_err(|e| format!("Z: {e}"))?;
        let size = field(3, "SIZE")?;
        let nums: Vec<i64> = size.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        if nums.len() != 2 {
            return Err(format!("bad SIZE {size:?}"));
        }
        let (w, h) = (nums[0] as usize, nums[1] as usize);
        let origin = field(4, "ORIGIN")?;
        let o: Vec<i32> = origin.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        if o.len() != 2 {
            return Err(format!("bad ORIGIN {origin:?}"));
        }
        let mut i = 5;
        let mut sections = Vec::new();
        for (head, glyphs) in [("TILES", TILE_GLYPHS), ("DRESS", DRESS_GLYPHS), ("ROCK", ROCK_GLYPHS)] {
            if lines.get(i).copied() != Some(head) {
                return Err(format!("line {}: expected {head}", i + 1));
            }
            i += 1;
            let mut rows = Vec::with_capacity(h);
            for r in 0..h {
                let row = lines.get(i + r).ok_or_else(|| format!("{head}: missing row {r}"))?;
                if row.len() != w {
                    return Err(format!("{head} row {r}: width {} != {w}", row.len()));
                }
                if let Some(bad) = row.bytes().find(|b| !glyphs.contains(b)) {
                    return Err(format!("{head} row {r}: glyph {:?} not in the legend", bad as char));
                }
                rows.push(row.to_string());
            }
            i += h;
            sections.push(rows);
        }
        if lines[i..].iter().any(|l| !l.is_empty()) {
            return Err("trailing text after ROCK".into());
        }
        let rock = sections.pop().unwrap();
        let dress = sections.pop().unwrap();
        let tiles = sections.pop().unwrap();
        Ok(LevelFile { slug, name, z, w, h, origin: (o[0], o[1]), tiles, dress, rock })
    }

    pub fn tile(&self, x: usize, y: usize) -> u8 {
        self.tiles[y].as_bytes()[x]
    }
}
