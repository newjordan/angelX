//! Checked pack-specific leadership templates; never ordinary mobs relabelled as bosses.
use super::{Boss, Pack, RoomKind, boss_gates::Leader};

fn natives(pack: Pack) -> Vec<Boss> {
    let files: &[(&str, &str)] = match pack {
        Pack::Cavern => &[
            (
                "iron-tusk-chief",
                include_str!("../../../assets/dungeon/bosses/iron-tusk-chief.boss"),
            ),
            (
                "red-pick-chief",
                include_str!("../../../assets/dungeon/bosses/red-pick-chief.boss"),
            ),
        ],
        Pack::Crypt => &[
            (
                "crypt-relic-liege",
                include_str!("../../../assets/dungeon/bosses/crypt-relic-liege.boss"),
            ),
            (
                "crypt-tithe-marshal",
                include_str!("../../../assets/dungeon/bosses/crypt-tithe-marshal.boss"),
            ),
            (
                "crypt-tax-keeper",
                include_str!("../../../assets/dungeon/bosses/crypt-tax-keeper.boss"),
            ),
        ],
        Pack::Archive => &[
            (
                "archive-bellkeeper",
                include_str!("../../../assets/dungeon/bosses/archive-bellkeeper.boss"),
            ),
            (
                "salvage-claimant",
                include_str!("../../../assets/dungeon/bosses/salvage-claimant.boss"),
            ),
        ],
        Pack::Fungal => &[
            (
                "fungal-spore-crown",
                include_str!("../../../assets/dungeon/bosses/fungal-spore-crown.boss"),
            ),
            (
                "fungal-rootwarden",
                include_str!("../../../assets/dungeon/bosses/fungal-rootwarden.boss"),
            ),
        ],
        Pack::Hellforge => &[
            (
                "hellforge-slag-sovereign",
                include_str!("../../../assets/dungeon/bosses/hellforge-slag-sovereign.boss"),
            ),
            (
                "hellforge-ash-castellan",
                include_str!("../../../assets/dungeon/bosses/hellforge-ash-castellan.boss"),
            ),
            (
                "hellforge-chain-keeper",
                include_str!("../../../assets/dungeon/bosses/hellforge-chain-keeper.boss"),
            ),
            (
                "hellforge-fuel-reeve",
                include_str!("../../../assets/dungeon/bosses/hellforge-fuel-reeve.boss"),
            ),
            (
                "hellforge-free-captain",
                include_str!("../../../assets/dungeon/bosses/hellforge-free-captain.boss"),
            ),
        ],
        Pack::Unknown => &[
            (
                "unknown-memory-warden",
                include_str!("../../../assets/dungeon/bosses/unknown-memory-warden.boss"),
            ),
            (
                "unknown-silence-warden",
                include_str!("../../../assets/dungeon/bosses/unknown-silence-warden.boss"),
            ),
        ],
    };
    super::bosses::builtin()
        .into_iter()
        .filter(|b| b.only_in == pack)
        .chain(files.iter().map(|(id, raw)| {
            super::bosses::check(id, raw)
                .expect("native leaders are checked")
                .0
        }))
        .collect()
}

fn threshold_guardian() -> Boss {
    super::bosses::check(
        "shoggoth-heart",
        include_str!("../../../assets/dungeon/bosses/shoggoth-heart.boss"),
    )
    .expect("native threshold guardian is checked")
    .0
}

pub(super) fn leaders(bosses: &mut Vec<Boss>, pack: Pack, count: usize) -> Option<Vec<usize>> {
    for native in natives(pack) {
        if bosses.len() < 64
            && !bosses
                .iter()
                .any(|b| b.id == native.id && b.only_in == native.only_in)
        {
            bosses.push(native);
        }
    }
    let mut choices: Vec<_> = bosses
        .iter()
        .enumerate()
        .take(64)
        .filter(|(_, b)| {
            b.only_in == pack
                && !matches!(
                    b.id.as_str(),
                    "shoggoth-heart"
                        | "crypt-relic-liege"
                        | "hellforge-slag-sovereign"
                        | "fungal-spore-crown"
                )
        })
        .map(|(i, _)| i)
        .collect();
    choices.sort_by_key(|&i| bosses[i].id.clone());
    choices.dedup_by(|a, b| bosses[*a].id == bosses[*b].id);
    (choices.len() >= count).then_some(choices)
}
pub(super) fn apex(
    bosses: &mut Vec<Boss>,
    pack: Pack,
    kind: RoomKind,
    leaders: &[Leader],
) -> Option<usize> {
    if kind == RoomKind::Threshold {
        let custom = bosses.iter().enumerate().take(64).find(|(_, b)| {
            b.only_in == pack
                && !matches!(
                    b.id.as_str(),
                    "unknown-memory-warden" | "unknown-silence-warden" | "shoggoth-heart"
                )
        });
        if let Some((i, _)) = custom {
            return Some(i);
        }
        if let Some(i) = bosses
            .iter()
            .take(64)
            .position(|b| b.id == "shoggoth-heart" && b.only_in == pack)
        {
            return Some(i);
        }
        if bosses.len() >= 64 {
            return None;
        }
        bosses.push(threshold_guardian());
        return Some(bosses.len() - 1);
    }
    let preferred = match pack {
        Pack::Crypt => "crypt-relic-liege",
        Pack::Hellforge => "hellforge-slag-sovereign",
        Pack::Fungal => "fungal-spore-crown",
        _ => "",
    };
    if let Some(i) = bosses
        .iter()
        .take(64)
        .position(|b| b.only_in == pack && b.id == preferred)
    {
        return Some(i);
    }
    bosses
        .iter()
        .enumerate()
        .take(64)
        .filter(|(_, b)| b.only_in == pack)
        .min_by_key(|(i, b)| {
            (
                leaders.iter().any(|l| usize::from(l.boss) == *i),
                b.id.as_str(),
            )
        })
        .map(|(i, _)| i)
}

pub(super) fn custom(boss: &Boss) -> bool {
    !natives(boss.only_in).iter().any(|b| b.id == boss.id)
}
