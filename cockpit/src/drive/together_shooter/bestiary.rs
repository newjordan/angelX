//! The Herald's Bestiary: every kind of monster the realm has felled, how
//! many, and what the Herald thinks of it. The realm counts (`Home::bestiary`, by kind,
//! from the run's kill marks); the card screen shows it, the kinds not yet
//! met as rumours.

use super::*;

/// One entry: the kind, its name in the book, the Herald's note.
pub(crate) struct Entry {
    pub(crate) kind: EnemyKind,
    pub(crate) name: &'static str,
    pub(crate) says: &'static str,
}

pub(crate) const ENTRIES: [Entry; 22] = [
    Entry {
        kind: EnemyKind::Bat,
        name: "Bat",
        says: "A rat that took up flying out of spite.",
    },
    Entry {
        kind: EnemyKind::Skeleton,
        name: "Skeleton",
        says: "Bones with a grudge and a bow.",
    },
    Entry {
        kind: EnemyKind::Slime,
        name: "Slime",
        says: "Divides, then conquers, then divides again.",
    },
    Entry {
        kind: EnemyKind::Wraith,
        name: "Wraith",
        says: "A sigh that learned to throw things.",
    },
    Entry {
        kind: EnemyKind::Imp,
        name: "Imp",
        says: "Small, hot, and furious about both.",
    },
    Entry {
        kind: EnemyKind::Sapper,
        name: "Sapper",
        says: "Carries a keg. Has a plan. The plan is the keg.",
    },
    Entry {
        kind: EnemyKind::Hob,
        name: "Hob",
        says: "Throws bombs where you were, and is very proud of it.",
    },
    Entry {
        kind: EnemyKind::Necromancer,
        name: "Necromancer",
        says: "Raises the dead. Never asks how they feel about it.",
    },
    Entry {
        kind: EnemyKind::Warboar,
        name: "Warboar",
        says: "A pig with a line and a grudge.",
    },
    Entry {
        kind: EnemyKind::Shaman,
        name: "Shaman",
        says: "Builds little walls round you and calls it a gift.",
    },
    Entry {
        kind: EnemyKind::Ward,
        name: "Serpent ward",
        says: "Stands there spitting until it doesn't.",
    },
    Entry {
        kind: EnemyKind::Goblin,
        name: "Loot goblin",
        says: "Has your gold. Is leaving. Is gone.",
    },
    Entry {
        kind: EnemyKind::Mimic,
        name: "Mimic",
        says: "It was a chest. It is still, technically, a chest.",
    },
    Entry {
        kind: EnemyKind::Demon,
        name: "Demon",
        says: "The imp's large, disappointed father.",
    },
    Entry {
        kind: EnemyKind::Dragon,
        name: "Dragon",
        says: "The old story. The one everyone tells.",
    },
    Entry {
        kind: EnemyKind::Flesher,
        name: "Flesher",
        says: "A hook, a cleaver, and a very short list of hobbies.",
    },
    Entry {
        kind: EnemyKind::Silkmother,
        name: "Silkmother",
        says: "Keeps the Archive's stacks. Keeps her children closer.",
    },
    Entry {
        kind: EnemyKind::Spiderling,
        name: "Spiderling",
        says: "Eight legs, no manners.",
    },
    Entry {
        kind: EnemyKind::PitTyrant,
        name: "Pit Tyrant",
        says: "Lives in the Pit. The Pit is his. You are in it.",
    },
    Entry {
        kind: EnemyKind::Hexer,
        name: "Hexer",
        says: "Turns knights into frogs. Has never once turned one back.",
    },
    Entry {
        kind: EnemyKind::Lich,
        name: "Lich",
        says: "Throws the cold around. It always comes back. Stand apart.",
    },
    Entry {
        kind: EnemyKind::Hollow,
        name: "Hollow One",
        says: "Opens a hole in the world and leans on it. Walk out, or hit it till it stops.",
    },
];

/// The key a kind is counted under (the same as its kill mark's).
pub(crate) fn key(kind: EnemyKind) -> String {
    format!("{kind:?}")
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_bestiary__tests.rs"]
mod tests;
