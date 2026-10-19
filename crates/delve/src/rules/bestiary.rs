//! The Herald's Bestiary: every kind of monster the realm has felled, how
//! many, and what the Herald thinks of it. The realm counts (`Home::bestiary`, by kind,
//! from the run's kill marks); the card screen shows it, the kinds not yet
//! met as rumours.

use super::*;

/// One entry: the kind, its name in the book, the Herald's note.
pub struct Entry {
    pub kind: EnemyKind,
    pub name: &'static str,
    pub says: &'static str,
}

pub const ENTRIES: [Entry; 40] = [
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
    Entry {
        kind: EnemyKind::Biter,
        name: "Biter",
        says: "Four at a time, and every one of them sure it will be the one that gets through.",
    },
    Entry {
        kind: EnemyKind::Shard,
        name: "Shard",
        says: "What a geode becomes when it has had enough. Smaller, and in a worse mood.",
    },
    Entry {
        kind: EnemyKind::Mole,
        name: "Mole",
        says: "The floor cracks. You were standing there. You are not now.",
    },
    Entry {
        kind: EnemyKind::Pavise,
        name: "Pavise",
        says: "A tower shield with a knight attached. Go round.",
    },
    Entry {
        kind: EnemyKind::Leech,
        name: "Leech",
        says: "Attaches itself to your good health. Roll, and be rid.",
    },
    Entry {
        kind: EnemyKind::Marksman,
        name: "Marksman",
        says: "Draws a long line and means every inch of it. Step off the line.",
    },
    Entry {
        kind: EnemyKind::Mirrorling,
        name: "Mirrorling",
        says: "Watches what you shoot. Returns it, with interest, a moment later.",
    },
    Entry {
        kind: EnemyKind::Spitter,
        name: "Spitter",
        says: "Aims at where you stand. The floor keeps the rest.",
    },
    Entry {
        kind: EnemyKind::Ringer,
        name: "Ringer",
        says: "One bell, rung once, and every monster in the room walks faster.",
    },
    Entry {
        kind: EnemyKind::Geode,
        name: "Geode",
        says: "A crystal with opinions. Breaks into two smaller ones.",
    },
    Entry {
        kind: EnemyKind::Hound,
        name: "Cinderhound",
        says: "Crouches, stares, and goes. Leaves embers where it was.",
    },
    Entry {
        kind: EnemyKind::Risen,
        name: "Humphrey the Risen",
        says: "Once a neighbor. Still wants his hedge back. Always comes back.",
    },
    Entry {
        kind: EnemyKind::Broodmarrow,
        name: "Broodmarrow",
        says: "Throws its children at you. They were not consulted.",
    },
    Entry {
        kind: EnemyKind::Oathbound,
        name: "Oathbound",
        says: "Two graves, one wound. Refuses to be an only child.",
    },
    Entry {
        kind: EnemyKind::Whirlgrave,
        name: "Whirlgrave",
        says: "Leans on the world until the world leans back.",
    },
    Entry {
        kind: EnemyKind::BogHand,
        name: "Bog Hand",
        says: "It was somebody's once. Now it only wants yours.",
    },
    Entry {
        kind: EnemyKind::Drowned,
        name: "Drowned Knight",
        says: "Took his oath to the bottom of the mere and kept it there.",
    },
    Entry {
        kind: EnemyKind::Wisp,
        name: "Will-o'-Wisp",
        says: "A lantern nobody is holding, for a good reason.",
    },
];

/// The key a kind is counted under (the same as its kill mark's).
pub fn key(kind: EnemyKind) -> String {
    format!("{kind:?}")
}

#[cfg(test)]
#[path = "../../tests/rules/together_bestiary__tests.rs"]
mod tests;

/// Guardians keep individual pages, counted by their existing trophy identity.
pub struct GuardianEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub says: &'static str,
}
pub const GUARDIANS: [GuardianEntry; 2] = [
    GuardianEntry {
        id: "the-big-ripper",
        name: "The Big Ripper",
        says: "Held his breath for a crown. Lost the room's respect.",
    },
    GuardianEntry {
        id: "grinwick",
        name: "Grinwick",
        says: "Eight legs. No return tickets. Still somehow late.",
    },
];
