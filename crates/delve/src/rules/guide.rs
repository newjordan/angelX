//! The way on, below. Once a room is clear, one of its doorways is marked
//! toward the floor's next aim: the boss while the boss door will open,
//! somewhere unexplored while the key is still out there, otherwise the
//! way down. The marked doorway is the first step of the shortest walk
//! through open doors.
use super::*;

impl Run {
    pub fn way_on(&self) -> Option<(usize, &'static str)> {
        if self.at_home_now()
            || self.room().kind.in_world()
            || self.room().way_back().is_some()
            || self.barred()
        {
            return None;
        }
        let rooms = &self.dungeon.rooms;
        let boss = self
            .boss_gates
            .as_ref()
            .map(|g| (g.guardian, g.guardian_defeated));
        let (aims, name): (Vec<usize>, &'static str) = match boss {
            Some((_, false)) if self.boss_door_shut() && self.key_bearer().is_none() => (
                (0..rooms.len()).filter(|&i| !rooms[i].visited).collect(),
                "Find the key",
            ),
            Some((room, false)) => (vec![room], "To the boss"),
            _ => {
                let down: Vec<usize> = (0..rooms.len())
                    .filter(|&i| matches!(rooms[i].kind, RoomKind::Stairs | RoomKind::Lair))
                    .collect();
                if down.is_empty() {
                    ((0..rooms.len()).filter(|&i| !rooms[i].visited).collect(), "Explore")
                } else {
                    (down, "Down the stairs")
                }
            }
        };
        if aims.contains(&self.at) {
            return None;
        }
        let mut far = vec![usize::MAX; rooms.len()];
        let mut queue = std::collections::VecDeque::new();
        for &aim in &aims {
            far[aim] = 0;
            queue.push_back(aim);
        }
        while let Some(room) = queue.pop_front() {
            for dir in 0..4 {
                if let Some(next) = self.dungeon.neighbour(room, dir)
                    && far[next] == usize::MAX
                {
                    far[next] = far[room] + 1;
                    queue.push_back(next);
                }
            }
        }
        (0..4)
            .filter_map(|dir| {
                let next = self.dungeon.neighbour(self.at, dir)?;
                (far[next] != usize::MAX).then_some((far[next], dir))
            })
            .min()
            .map(|(_, dir)| (dir, name))
    }
}

impl Run {
    pub fn home_way(&self) -> Option<((f32, f32), &'static str)> {
        if !self.at_home_now() || self.room().kind != RoomKind::Home || self.home.deepest > 0 {
            return None;
        }
        let (c, r, w, h) = home::STAIRWELL;
        Some((
            ((c as f32 + w as f32 / 2.0) * TILE_UNITS, (r as f32 + h as f32 / 2.0) * TILE_UNITS),
            "Down the stair",
        ))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_guide__tests.rs"]
mod tests;
