//! Checked pack-specific leadership templates; never ordinary mobs relabelled as bosses.
use super::{Boss, Pack, RoomKind, boss_gates::Leader};

// Only checked include_str templates are cached. Mutable Run catalogues are
// compared/validated as before and receive independent owned Boss values.
fn natives(pack: Pack) -> &'static [Boss] {
    use std::sync::OnceLock;
    static CRYPT: OnceLock<Vec<Boss>> = OnceLock::new();
    static CAVERN: OnceLock<Vec<Boss>> = OnceLock::new();
    static HELLFORGE: OnceLock<Vec<Boss>> = OnceLock::new();
    static ARCHIVE: OnceLock<Vec<Boss>> = OnceLock::new();
    static FUNGAL: OnceLock<Vec<Boss>> = OnceLock::new();
    static UNKNOWN: OnceLock<Vec<Boss>> = OnceLock::new();
    static SWAMP: OnceLock<Vec<Boss>> = OnceLock::new();
    let cache = match pack {
        Pack::Crypt => &CRYPT,
        Pack::Cavern => &CAVERN,
        Pack::Hellforge => &HELLFORGE,
        Pack::Archive => &ARCHIVE,
        Pack::Fungal => &FUNGAL,
        Pack::Unknown => &UNKNOWN,
        Pack::Swamp => &SWAMP,
    };
    cache.get_or_init(|| parse_natives(pack))
}

fn parse_natives(pack: Pack) -> Vec<Boss> {
    let files: &[(&str, &str)] = match pack {
        Pack::Cavern => &[
            (
                "iron-tusk-chief",
                include_str!("../../../../cockpit/assets/dungeon/bosses/iron-tusk-chief.boss"),
            ),
            (
                "red-pick-chief",
                include_str!("../../../../cockpit/assets/dungeon/bosses/red-pick-chief.boss"),
            ),
        ],
        Pack::Crypt => &[
            (
                "crypt-relic-liege",
                include_str!("../../../../cockpit/assets/dungeon/bosses/crypt-relic-liege.boss"),
            ),
            (
                "crypt-tithe-marshal",
                include_str!("../../../../cockpit/assets/dungeon/bosses/crypt-tithe-marshal.boss"),
            ),
            (
                "crypt-tax-keeper",
                include_str!("../../../../cockpit/assets/dungeon/bosses/crypt-tax-keeper.boss"),
            ),
        ],
        Pack::Archive => &[
            (
                "archive-bellkeeper",
                include_str!("../../../../cockpit/assets/dungeon/bosses/archive-bellkeeper.boss"),
            ),
            (
                "salvage-claimant",
                include_str!("../../../../cockpit/assets/dungeon/bosses/salvage-claimant.boss"),
            ),
            // Made from the chief template (`templates/foe-chief.boss`).
            (
                "archive-drowned-reeve",
                include_str!(
                    "../../../../cockpit/assets/dungeon/bosses/archive-drowned-reeve.boss"
                ),
            ),
        ],
        Pack::Fungal => &[
            (
                "fungal-spore-crown",
                include_str!("../../../../cockpit/assets/dungeon/bosses/fungal-spore-crown.boss"),
            ),
            (
                "fungal-rootwarden",
                include_str!("../../../../cockpit/assets/dungeon/bosses/fungal-rootwarden.boss"),
            ),
        ],
        Pack::Hellforge => &[
            (
                "hellforge-slag-sovereign",
                include_str!(
                    "../../../../cockpit/assets/dungeon/bosses/hellforge-slag-sovereign.boss"
                ),
            ),
            (
                "hellforge-ash-castellan",
                include_str!(
                    "../../../../cockpit/assets/dungeon/bosses/hellforge-ash-castellan.boss"
                ),
            ),
            (
                "hellforge-chain-keeper",
                include_str!(
                    "../../../../cockpit/assets/dungeon/bosses/hellforge-chain-keeper.boss"
                ),
            ),
            (
                "hellforge-fuel-reeve",
                include_str!("../../../../cockpit/assets/dungeon/bosses/hellforge-fuel-reeve.boss"),
            ),
            (
                "hellforge-free-captain",
                include_str!(
                    "../../../../cockpit/assets/dungeon/bosses/hellforge-free-captain.boss"
                ),
            ),
        ],
        // The swamp's one guardian is a built-in (`the-mire-king`).
        Pack::Swamp => &[],
        Pack::Unknown => &[
            (
                "unknown-memory-warden",
                include_str!(
                    "../../../../cockpit/assets/dungeon/bosses/unknown-memory-warden.boss"
                ),
            ),
            (
                "unknown-silence-warden",
                include_str!(
                    "../../../../cockpit/assets/dungeon/bosses/unknown-silence-warden.boss"
                ),
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
        include_str!("../../../../cockpit/assets/dungeon/bosses/shoggoth-heart.boss"),
    )
    .expect("native threshold guardian is checked")
    .0
}

pub(super) fn leaders(bosses: &mut Vec<Boss>, pack: Pack, count: usize) -> Option<Vec<usize>> {
    for native in natives(pack) {
        // Signature guardians enter new floor catalogs explicitly. Replaying
        // an older floor must preserve its original encounter choices.
        if super::signatures::special(&native)
            && !bosses
                .iter()
                .any(|b| b.id == native.id && b.only_in == pack)
        {
            continue;
        }
        if bosses.len() < 64
            && !bosses
                .iter()
                .any(|b| b.id == native.id && b.only_in == native.only_in)
        {
            bosses.push(native.clone());
        }
    }
    let mut choices: Vec<_> = bosses
        .iter()
        .enumerate()
        .take(64)
        .filter(|(_, b)| {
            // A chief's camp is led by an elite (`keyring`), so the pack's
            // signature bosses are kept for the stairs (`boss_population`).
            b.only_in == pack
                && super::claim_bosses::Guardian::for_id(&b.id).is_none()
                && !super::signatures::special(b)
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
                && super::claim_bosses::Guardian::for_id(&b.id).is_none()
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
        .filter(|(_, b)| b.only_in == pack && super::claim_bosses::Guardian::for_id(&b.id).is_none())
        .min_by_key(|(i, b)| {
            (
                leaders.iter().any(|l| usize::from(l.boss) == *i),
                b.id.as_str(),
            )
        })
        .map(|(i, _)| i)
}

pub(super) fn custom(boss: &Boss) -> bool {
    super::claim_bosses::Guardian::for_id(&boss.id).is_none()
        && !natives(boss.only_in).iter().any(|b| b.id == boss.id)
}

#[cfg(test)]
mod template_cache_tests {
    use super::*;
    fn reference_leaders(bosses: &mut Vec<Boss>, pack: Pack, count: usize) -> Option<Vec<usize>> {
        for native in parse_natives(pack) {
            // Signature guardians enter new floor catalogs explicitly. Replaying
            // an older floor must preserve its original encounter choices.
            if crate::rules::signatures::special(&native)
                && !bosses
                    .iter()
                    .any(|b| b.id == native.id && b.only_in == pack)
            {
                continue;
            }
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
                // A chief's camp is led by an elite (`keyring`), so the pack's
                // signature bosses are kept for the stairs (`boss_population`).
                b.only_in == pack
                    && !crate::rules::signatures::special(b)
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
    #[test]
    fn every_pack_cache_matches_original_checked_templates_and_identity() {
        for pack in Pack::ALL {
            let original = parse_natives(pack);
            let cached = natives(pack);
            assert_eq!(cached, original.as_slice(), "pack {pack:?}");
            assert!(std::ptr::eq(cached, natives(pack)));
            assert!(
                crate::rules::bosses::check("broken-template", "not a boss definition").is_err()
            );
            assert_eq!(
                natives(pack),
                original.as_slice(),
                "checker rejection mutated cached templates"
            );
        }
    }
    #[test]
    fn cached_leaders_match_original_for_all_packs_custom_mutations_and_bounds() {
        for pack in Pack::ALL {
            let templates = parse_natives(pack);
            assert!(!templates.is_empty());
            for variant in 0..8 {
                let mut input = match variant {
                    0 => Vec::new(),
                    1 => crate::rules::bosses::builtin(),
                    _ => templates.clone(),
                };
                match variant {
                    2 => {
                        for boss in &mut input {
                            boss.hp = boss.hp.saturating_add(17);
                            boss.name.push_str(" customized");
                            boss.art.push("retained art".into());
                        }
                    }
                    3 => {
                        for boss in &mut input {
                            boss.id = format!("custom-{}", boss.id);
                        }
                    }
                    4 | 5 => {
                        let n = if variant == 4 { 64 } else { 65 };
                        input = (0..n)
                            .map(|i| {
                                let mut boss = templates[0].clone();
                                boss.id = format!("custom-{i:02}");
                                boss
                            })
                            .collect();
                    }
                    6 => input.reverse(),
                    7 => {
                        input.push(templates[0].clone());
                        input[0].only_in = Pack::ALL.into_iter().find(|p| *p != pack).unwrap();
                    }
                    _ => {}
                }
                for count in [0, 1, 2, 4, 64, 65] {
                    let mut old = input.clone();
                    let mut cached = input.clone();
                    let expected = reference_leaders(&mut old, pack, count);
                    let actual = leaders(&mut cached, pack, count);
                    assert_eq!(
                        actual, expected,
                        "pack={pack:?} variant={variant} count={count}"
                    );
                    assert_eq!(
                        cached, old,
                        "custom catalogue contents changed pack={pack:?} variant={variant} count={count}"
                    );
                }
                assert_eq!(
                    natives(pack),
                    templates.as_slice(),
                    "mutable catalogue leaked into checked templates"
                );
            }
        }
    }
    #[test]
    fn concurrent_readers_share_immutable_per_pack_templates() {
        for pack in Pack::ALL {
            let readers = (0..8)
                .map(|_| {
                    std::thread::spawn(move || {
                        let cached = natives(pack);
                        assert_eq!(cached, parse_natives(pack).as_slice());
                        (cached.as_ptr() as usize, cached.len())
                    })
                })
                .collect::<Vec<_>>();
            let expected = (natives(pack).as_ptr() as usize, natives(pack).len());
            for reader in readers {
                assert_eq!(reader.join().unwrap(), expected);
            }
        }
    }
}
