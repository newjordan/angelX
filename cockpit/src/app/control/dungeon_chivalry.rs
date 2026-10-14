//! The stables and the lists from the composer: a way into those rooms of
//! the world, and text forms of what their plates do. Every change goes
//! through the same marks the plates leave, settled by `settle_home`.
use super::super::App;
use crate::drive::chivalry::{Mount, Place};
use crate::drive::together_shooter::{self as shooter, RoomKind};

const HELP: &str = "/dungeon stable [select Bramble|Cinder|Mist|<herd n>|tend|status|market|buy <n>|sell <n>|breed <a> <b>] · /dungeon tournament [status] · /world visit stables|lists · walk in from the Delve's gate";

impl App {
    /// Open the Delve at the stables or the lists: start a delve if none is
    /// open (it begins at the gate, next door) and walk the party in.
    pub(crate) fn visit_horsemanship(&mut self, place: Place) -> String {
        if self.dungeon.joined.is_some() {
            return format!(
                "You are in a friend's delve: {} is a walk from its gate.",
                place.label().to_lowercase()
            );
        }
        if self.dungeon.shooter.is_none() {
            self.start_shooter(None);
        }
        let kind = match place {
            Place::Stables => RoomKind::Stables,
            Place::Tournament => RoomKind::Lists,
        };
        let Some(mut run) = self.dungeon.shooter.as_mut() else {
            return self.dungeon.notice.clone();
        };
        if !run.at_home_now() {
            return format!(
                "The party is down in the Delve; {} waits at the gate for the next delve.",
                place.label().to_lowercase()
            );
        }
        if run.joust.is_some() {
            drop(run);
            self.expand_dungeon();
            return "A bout is being ridden at the lists.".into();
        }
        if let Some(index) = shooter::world::room_of(&run.dungeon, kind) {
            let at = match place {
                Place::Stables => (18.0, 6.6),
                // On the mount plate itself: its board says who and how.
                Place::Tournament => (2.0, 8.5),
            };
            run.arrive(index, at);
        }
        drop(run);
        self.expand_dungeon();
        match place {
            Place::Stables => self.realm_stable_status(),
            Place::Tournament => self.realm_lists_status(),
        }
    }

    fn realm_stable_status(&mut self) -> String {
        let home = &self.realm().home;
        let stable = &home.stable;
        let wins = home.ranch.wins;
        let stalls = Mount::ALL
            .map(|m| {
                format!(
                    "{}{} ({})",
                    m.name(),
                    if stable.is_tended(m) { ", tended" } else { "" },
                    m.says()
                )
            })
            .join(" · ");
        let herd = if stable.herd.is_empty() {
            "The yard beyond is empty: no herd horses yet.".to_string()
        } else {
            stable
                .herd
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let age = h.born.map(|_| format!(" · {:?}", shooter::ranch::age(h, wins))).unwrap_or_default();
                    format!("{}. {}{}{} · {}", i + 1,
                        if stable.selected_herd == Some(i) { "selected · " } else { "" },
                        if h.mare { "mare" } else { "stallion" }, age, h.lore(stable))
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        format!(
            "The stables · saddled: {} · {stalls}. Hold E at a stall's plate to saddle that mount; at the trough to tend it (a knock more of balance for one bout). {herd}",
            stable.selected.name()
        )
    }

    fn realm_lists_status(&mut self) -> String {
        let stable = self.realm().home.stable.clone();
        let next = shooter::joust::RIVALS[shooter::joust::next_rival(&stable)].name;
        format!(
            "The lists · next rival: {next} · on {}{} · {} bouts, {} rivals unhorsed. Hold E at the plate by the red pavilion to mount; F spurs, W/S aims high or low, F strikes as the lances meet, Space braces.",
            stable.selected.name(),
            if stable.is_tended(stable.selected) {
                ", tended"
            } else {
                ""
            },
            stable.bouts,
            stable.unhorsed,
        )
    }

    /// The horse market: today's stock is pure in the stable's service
    /// record, so the listing a player reads is the listing `buy` sells.
    fn market_of(stable: &crate::drive::chivalry::Stable) -> Vec<crate::drive::chivalry::Horse> {
        stable.todays_market()
    }

    /// Buy, sell and breed in the yard, settled and saved like a plate's
    /// marks: clone, change, save, and rebuild the live home on success.
    fn horse_trade(&mut self, tail: &[&str]) -> String {
        use crate::drive::together_realm::{Spoil, Spoils};
        if self.dungeon.joined.is_some() {
            return "The host's stable is theirs to keep: walk to it in their delve.".into();
        }
        let mut stable = self.realm().home.stable.clone();
        let treasury_before = self.realm().treasury.clone();
        let gold = self.realm().treasury.get(Spoil::Gold);
        let line = match tail {
            ["market"] => Self::market_of(&stable)
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    format!(
                        "{}. {} ({}) — {} gold. {}",
                        i + 1,
                        h.name,
                        if h.mare { "mare" } else { "stallion" },
                        h.value(&stable),
                        h.lore(&stable)
                    )
                })
                .collect::<Vec<_>>()
                .join(" "),
            ["select", n] => {
                let Some(at) = n.parse::<usize>().ok().and_then(|i| i.checked_sub(1))
                    .filter(|&i| i < stable.herd.len()) else {
                    return format!("No herd horse {n}. {HELP}");
                };
                stable.selected_herd = Some(at);
                format!("{} selected for the coach and the stud book.", stable.herd[at].name)
            }
            ["buy", n] => {
                let Some(horse) = n
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| Self::market_of(&stable).into_iter().nth(i.checked_sub(1)?))
                else {
                    return format!("No horse {n} at market. {HELP}");
                };
                match stable.buy(horse.clone(), gold) {
                    Some(price) => {
                        let mut cost = Spoils::default();
                        cost.add(Spoil::Gold, price);
                        self.realm().treasury.take(&cost);
                        format!(
                            "{} joins your yard for {price} gold. {}",
                            horse.name,
                            horse.lore(&stable)
                        )
                    }
                    None => format!(
                        "{} is beyond the purse ({gold} gold) or the yard is full.",
                        horse.name
                    ),
                }
            }
            ["sell", n] => {
                let Some(at) = n.parse::<usize>().ok().and_then(|i| i.checked_sub(1)) else {
                    return format!("No herd horse {n}. {HELP}");
                };
                match stable.sell(at) {
                    Some(paid) => {
                        self.realm().treasury.add(Spoil::Gold, paid);
                        format!("Sold at the road for {paid} gold.")
                    }
                    None => format!("No herd horse {n}. {HELP}"),
                }
            }
            ["breed", a, b] => {
                let (Some(a), Some(b)) = (
                    a.parse::<usize>().ok().and_then(|i| i.checked_sub(1)),
                    b.parse::<usize>().ok().and_then(|i| i.checked_sub(1)),
                ) else {
                    return format!("Name two herd horses by number. {HELP}");
                };
                let seed: u64 = stable.yards.values().map(|&n| u64::from(n)).sum();
                match stable.breed(a, b, seed ^ 0xF0A1).cloned() {
                    Some(foal) => {
                        let line = foal.lore(&stable);
                        format!("A foal in the yard: {line}")
                    }
                    None => {
                        "No foal: pick two different herd horses, in a yard under twelve.".into()
                    }
                }
            }
            _ => return format!("Unknown. {HELP}"),
        };
        let realm = self.realm();
        let before = std::mem::replace(&mut realm.home.stable, stable);
        if let Err(error) = realm.save() {
            realm.home.stable = before;
            realm.treasury = treasury_before;
            return format!("Not saved: {error}");
        }
        let (home, treasury) = (realm.home.clone(), realm.treasury.clone());
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            run.rebuild_home(home, treasury);
        }
        line
    }

    /// `/dungeon stable …` and `/dungeon tournament …`.
    pub(crate) fn chivalry_command(&mut self, place: Place, tail: &str) -> String {
        let args: Vec<_> = tail.split_whitespace().collect();
        if place == Place::Stables
            && (matches!(args.first().copied(), Some("market" | "buy" | "sell" | "breed"))
                || matches!(args.as_slice(), ["select", n] if n.parse::<usize>().is_ok()))
        {
            return self.horse_trade(&args);
        }
        let mark = match (place, args.as_slice()) {
            (_, [] | ["enter"]) => return self.visit_horsemanship(place),
            (Place::Stables, ["status"]) => return self.realm_stable_status(),
            (Place::Tournament, ["status"]) => return self.realm_lists_status(),
            (Place::Stables, ["select", name]) => match Mount::parse(name) {
                Some(m) => format!("stable:select:{}", m.name().to_lowercase()),
                None => return format!("No mount called {name}. {HELP}"),
            },
            (Place::Stables, ["tend"]) => "stable:tend".to_string(),
            _ => return format!("Unknown. {HELP}"),
        };
        if self.dungeon.joined.is_some() {
            return "The host's stable is theirs to keep: walk to it in their delve.".into();
        }
        // The same mark a plate leaves, settled the same way.
        let marks = std::collections::BTreeMap::from([(mark, 1u32)]);
        let mut stable = self.realm().home.stable.clone();
        let said = shooter::joust::settle_stable(&mut stable, &marks);
        let realm = self.realm();
        let before = std::mem::replace(&mut realm.home.stable, stable);
        if let Err(error) = realm.save() {
            realm.home.stable = before;
            return format!("Not saved: {error}");
        }
        let (home, treasury) = (realm.home.clone(), realm.treasury.clone());
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            run.rebuild_home(home, treasury);
        }
        said.into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/dungeon_chivalry__tests.rs"]
mod tests;
