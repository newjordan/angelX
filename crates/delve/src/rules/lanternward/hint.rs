//! The wisp's words: the one line a knight reads at the foot of their
//! screen (the picture and the terminal both print it), and what E would do.
//! Few words, always about what to do next.
use super::world::*;
use super::*;

/// What a target that is not a foe says about itself when it is locked.
pub fn lock_hint(s: &Session, k: &Seat) -> Option<&'static str> {
    let Lock::Prop(i) = k.lock? else {
        return None;
    };
    let p = s.props.get(usize::from(i))?;
    Some(match p.kind {
        Pk::Sign => props::sign_text(usize::from(s.room), p.id),
        Pk::Folk => "A KINGSMAN: E TALKS",
        Pk::Crystal => "A CRYSTAL: LIGHT OR A PARRIED BOLT",
        Pk::Great => "THE GREAT BRAZIER: THROW LIGHT",
        Pk::Brazier => "A BRAZIER: LIGHT IT",
        Pk::Sigil => "A HIDDEN SIGIL: FLARE IT",
        Pk::Lever => "A LEVER: E PULLS",
        _ => return None,
    })
}

/// The wisp's one line. Few words, always about what to do next.
pub fn hint(s: &Session, k: &Seat, r: &Rules) -> Option<&'static str> {
    if k.act == Act::Down {
        return Some("WAIT FOR A FRIEND'S HAND");
    }
    let near_foe = s
        .foes
        .iter()
        .any(|f| f.alive() && dist(f.x - k.x, f.y - k.y) < r.lock);
    Some(match (s.room, s.stage) {
        (0, 0) if k.lock.is_none() && near_foe => "HOLD R TO LOCK ON",
        (0, 0) if k.lock.is_some() && k.held & kit::B_SWORD == 0 && s.elapsed < 600 => "LOCK ON, THEN HOP: E",
        (0, 0) if k.lock.is_some() => "Q SLASH  HOLD SPACE BLOCK",
        (0, 0) if s.elapsed < 150 => "WALK TO THE SNUFFER",
        (0, _) => "THE STAIR IS OPEN",
        (1, 0) if k.taper == 0 => "E AT THE EMBER BOWL",
        (1, 0) => "LIGHT ALL THREE AT ONCE",
        (1, 1) => "OPEN THE CHEST",
        (1, 2) => "HOLD F TO FLARE",
        (1, 3) => "TAP F AT THE GREAT BRAZIER",
        (2, 0) if s.props.iter().any(|p| p.kind == Pk::Gate && p.id == 1 && p.on == 1) => {
            "LOCK THE CRYSTAL, SHIELD UP"
        }
        (2, 0) if s.seats.len() >= 2
            && s.props.iter().any(|p| p.kind == Pk::Plate && p.id == 0 && p.on == 1) =>
        {
            "A FRIEND STANDS ON THE SECOND PLATE"
        }
        (2, 0) => "PUSH THE BLOCK ONTO THE PLATE",
        (2, _) if s.inv.lantern && k.lock.is_none() => "LOCK THE CRYSTAL, SHIELD UP",
        (2, _) => "SPACE SHIELD TURNS THE BOLT",
        (3, 0) => "HOLD F: BURN THE HOOD OFF",
        (3, 1) => "TAKE THE MOON KEY",
        (3, 2) => "E AT THE LOCKED DOOR",
        (3, 3) => "OPEN THE BOSS KEY CHEST",
        (3, _) => "CARRY THE KEY TO THE DOOR",
        (4, 0) => "THROW LIGHT AT ITS HEART",
        (4, 1) => match &s.boss {
            Some(w) if w.phase >= 1 && s.seats.len() >= 2 => "TWO LIGHTS AT ONCE",
            Some(w) if w.st == Ws::Tell && w.atk == Atk::Slam => "HOP THE RING",
            Some(w) if w.st == Ws::Tell && w.atk == Atk::Beam => "SHIELD UP: PARRY THE BEAM",
            _ => "LIGHT THE HEART, THEN STRIKE",
        },
        (4, 2) => "TAKE THE HEART",
        (4, 3) => "HOLD E AT THE BEACON",
        _ => return None,
    })
}


/// The line at the foot of seat `k`'s screen: a sign or kingsman being read,
/// else what a locked switch says, else the wisp.
pub fn line(s: &Session, k: &Seat, r: &Rules) -> Option<String> {
    if let Some((id, _)) = k.read {
        let words = if id >= 100 {
            props::folk_text(usize::from(s.room), id - 100)
        } else {
            props::sign_text(usize::from(s.room), id)
        };
        return Some(words.to_string());
    }
    lock_hint(s, k).or_else(|| hint(s, k, r)).map(str::to_string)
}

/// What E would do: its label, or the roll or hop it falls back to.
pub fn context(k: &Seat) -> String {
    match k.ctx {
        Ctx::None if k.lock.is_some() => "E HOP".to_string(),
        Ctx::None => "E ROLL".to_string(),
        c => format!("E {}", c.label()),
    }
}
