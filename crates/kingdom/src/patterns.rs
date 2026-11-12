//! Stamp-down patterns: a group of buildings, roads, shafts and lines laid
//! as one command, from the catalogue's `patterns`. Everything a stamp does
//! is an ordinary command run in order inside one atomic apply, so a stamp
//! that cannot be finished anywhere changes nothing at all.
use crate::*;

/// Quarter turns clockwise (y runs down the screen).
fn turned((dx, dy): (i16, i16), turns: u8) -> (i16, i16) {
    match turns % 4 {
        0 => (dx, dy),
        1 => (-dy, dx),
        2 => (-dx, -dy),
        _ => (dy, -dx),
    }
}

impl Kingdom {
    pub(crate) fn stamp(&mut self, c: &Catalog, name: &str, origin: Tile, turns: u8) -> Result<String, String> {
        let p = self.pattern(c, name).ok_or("unknown pattern")?.clone();
        let at = |off: (i16, i16)| {
            let (dx, dy) = turned(off, turns);
            (origin.0 + dx, origin.1 + dy)
        };
        let mut roads = 0;
        for &r in &p.roads {
            // A road already there is part of the pattern, not an error.
            if !self.roads.contains(&at(r)) {
                self.execute(c, Command::Road(at(r)))?;
                roads += 1;
            }
        }
        for &s in &p.shafts {
            if !self.power.contains(PowerKind::Mechanical, at(s)) {
                self.execute(c, Command::PowerLine { kind: PowerKind::Mechanical, tile: at(s) })?;
            }
        }
        let mut ids = Vec::new();
        for b in &p.builds {
            let tile = at(b.at);
            self.execute(c, Command::Build { plan: b.plan.clone(), tile })?;
            let id = self.at(tile).ok_or("a stamped building did not stand")?;
            if let Some(recipe) = &b.recipe {
                self.execute(c, Command::Recipe { building: id, recipe: recipe.clone() })?;
            }
            ids.push(id);
        }
        for l in &p.lines {
            self.execute(
                c,
                Command::Line {
                    kind: l.kind,
                    from: ids[l.from],
                    to: ids[l.to],
                    path: Vec::new(),
                    vehicles: l.vehicles.max(1),
                },
            )?;
        }
        Ok(format!(
            "{} stamped: {} buildings, {} lines, {} new roads",
            p.name,
            ids.len(),
            p.lines.len(),
            roads
        ))
    }

    /// What stamping a pattern here would cost, or why it cannot be. Run on a
    /// copy with a bottomless treasury, so the answer is exact, not a sum.
    pub fn stamp_cost(&self, c: &Catalog, name: &str, origin: Tile, turns: u8) -> Result<Stock, String> {
        let mut trial = self.clone();
        trial.start(c);
        let rich: Stock = c.resources.keys().map(|r| (r.clone(), 900_000)).collect();
        trial.stock = rich.clone();
        trial.researched = c.all_techs().map(|(k, _)| k.clone()).collect();
        trial.execute(c, Command::Stamp { pattern: name.into(), origin, turns })?;
        let mut cost = Stock::new();
        for (r, n) in rich {
            let left = trial.stock.get(&r).copied().unwrap_or(0);
            if left < n {
                cost.insert(r, n - left);
            }
        }
        Ok(cost)
    }
}
