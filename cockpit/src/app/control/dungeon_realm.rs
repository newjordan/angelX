//! The party's play economy in the cockpit: banked spoils reach the island's
//! treasury, wishes are asked, drafted by angelX and granted.
use super::*;
use crate::drive::together_realm::{self as realm, Realm, Status};
use crate::drive::together_shooter as shooter;

impl App {
    /// The island's realm, loaded beside the world's rewards on first use.
    pub(crate) fn realm(&mut self) -> &mut Realm {
        if self.dungeon.realm.is_none() {
            self.dungeon.realm = Some(Realm::beside(self.world.rewards_path()));
        }
        self.dungeon.realm.as_mut().expect("loaded above")
    }

    /// The host's name on plaques: the first word of git's user.name, else $USER.
    pub(crate) fn host_name(&self) -> String {
        static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        NAME.get_or_init(|| {
            std::process::Command::new("git")
                .args(["config", "user.name"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.split_whitespace().next().map(str::to_owned))
                .or_else(|| std::env::var("USER").ok())
                .unwrap_or_else(|| "Host".into())
        })
        .clone()
    }

    /// Move what the run banked into the treasury, under each knight's name.
    pub(crate) fn settle_realm(&mut self) -> bool {
        let Some(mut run) = self.dungeon.shooter.as_mut() else {
            return true;
        };
        if run.bank.is_empty() && run.reclaimed.is_empty() && run.triumph.is_none() {
            return true;
        }
        let hauls = std::mem::take(&mut run.bank);
        let reclaimed = std::mem::take(&mut run.reclaimed);
        let triumph = run.triumph.take();
        let names: std::collections::BTreeMap<u32, String> = run
            .players
            .iter()
            .map(|(&id, h)| (id, h.name.clone()))
            .collect();
        // The sim steps on while the realm is settled: hold the run no longer.
        drop(run);
        let host = self.host_name();
        let realm_before = self.realm().clone();
        let mut lines = Vec::new();
        let realm = self.realm();
        for haul in &hauls {
            let name = match names.get(&haul.hero).map(String::as_str) {
                Some("You") | None if haul.hero == 1 => host.clone(),
                Some(name) => name.to_string(),
                None => format!("Knight {}", haul.hero),
            };
            realm.bank(&name, &haul.spoils);
            lines.push(format!(
                "{name} banked {} ({})",
                haul.spoils.label(),
                haul.why
            ));
        }
        for pack in &reclaimed {
            *realm.reclaimed.entry(pack.name().to_string()).or_default() += 1;
        }
        // King Brannoc's tribute: his miners follow the knights down and dig
        // what every cleared floor frees.
        let tribute =
            shooter::barony::pay_tribute(&mut realm.home, &mut realm.treasury, reclaimed.len());
        if let Some(paid) = &tribute {
            lines.push(format!("King Brannoc's tribute: {}", paid.label()));
        }
        let tribute_paid = tribute.is_some();
        // A rent day at every holding for every floor cleared; the
        // Court learns which floors are cleared.
        let realm_reclaimed = realm.reclaimed.clone();
        shooter::holdings::note_realm(&mut realm.home.holdings, &realm_reclaimed, &realm.home.stable.yards.clone());
        let rents = shooter::holdings::rent_day(&mut realm.home.holdings, reclaimed.len());
        lines.extend(rents);
        match triumph {
            Some(shooter::Triumph::Dragon) => {
                realm.raids_won += 1;
                realm.offer_catalog();
            }
            Some(shooter::Triumph::Grail) => realm.grails += 1,
            None => {}
        }
        let affordable: Vec<String> = realm
            .wishes
            .iter()
            .filter(|w| w.status == Status::Drafted && realm.treasury.covers(&w.price))
            .map(|w| w.name.clone())
            .collect();
        let saved = realm.save();
        if let Err(error) = saved {
            self.dungeon.realm = Some(realm_before);
            // Back into the run, beside whatever it banked meanwhile.
            if let Some(mut run) = self.dungeon.shooter.as_mut() {
                let run = &mut *run;
                run.bank.splice(0..0, hauls);
                run.reclaimed.splice(0..0, reclaimed);
                run.triumph = run.triumph.or(triumph);
            }
            self.dungeon.notice = format!("Could not save the realm: {error}");
            let _ = self.dungeon.checkpoint(std::time::Instant::now(), true);
            return false;
        }
        let now = std::time::Instant::now();
        if !hauls.is_empty() {
            self.dungeon.chorus.cue("banked", now);
        }
        if tribute_paid {
            self.dungeon.chorus.cue("tribute", now);
        }
        if affordable.len() > self.dungeon.ready_heard {
            self.dungeon.chorus.cue("treasury_ready", now);
        }
        self.dungeon.ready_heard = affordable.len();
        if !lines.is_empty() {
            let mut notice = lines.join(" · ");
            if !affordable.is_empty() {
                notice.push_str(&format!(
                    " · the treasury can raise: {} (Tab → realm)",
                    affordable.join(", ")
                ));
            }
            self.dungeon.notice = notice;
        }
        // The run's bank is empty now; checkpoint so a crash cannot bank twice.
        let _ = self.dungeon.checkpoint(std::time::Instant::now(), true);
        true
    }

    /// Pay for what the party asked for at the Undercroft's plates out of
    /// the treasury, remember the deepest floor reached and the landing
    /// chosen, and tell the run what now stands and what it can spend.
    pub(crate) fn settle_home(&mut self) -> bool {
        self.flush_holdings_ops();
        let Some(mut run) = self.dungeon.shooter.as_mut() else {
            return true;
        };
        let orders = std::mem::take(&mut run.orders);
        // The surface peddler's sales, each with its price while the knight
        // still has room for it.
        let peddled: Vec<(shooter::merchant::Purchase, u32)> = std::mem::take(&mut run.peddled)
            .into_iter()
            .filter_map(|p| run.peddle_price(p).map(|gold| (p, gold)))
            .collect();
        let noticed = std::mem::take(&mut run.feats);
        let stall = run.stall.clone();
        let stall_prices: Vec<u32> = stall
            .iter()
            .map(|id| run.book.get(id).map_or(0, shooter::yard::stall_price))
            .collect();
        let stall_names: Vec<String> = stall
            .iter()
            .map(|id| run.book.get(id).map_or(id.clone(), |c| c.name.clone()))
            .collect();
        let depth = run.dungeon.depth;
        let landing = run.home.landing;
        let country_games = run.home.countryside.games.clone();
        let wardrobe = run.home.wardrobe.clone();
        let privy_wall = run.home.privy_wall.clone();
        let audience = run.audience;
        let show_over = matches!(run.phase, shooter::Phase::Won | shooter::Phase::Wiped);
        let marks = std::mem::take(&mut run.marks);
        let fighting = run.phase == shooter::Phase::Fighting;
        let names: std::collections::BTreeMap<u32, String> = run
            .players
            .iter()
            .map(|(&id, h)| (id, h.name.clone()))
            .collect();
        // Which knight of the company each player is, for Sir Ector.
        let keys: std::collections::BTreeMap<u32, String> = run
            .players
            .iter()
            .map(|(&id, h)| (id, shooter::talents::knight_key(h)))
            .collect();
        // The sim steps on while the realm is settled: hold the run no longer.
        drop(run);
        let host = self.host_name();
        let unsaved = self.dungeon.bounties_unsaved;
        let realm_before = {
            let realm = self.realm();
            let bounty_may_change = realm.home.bounties.iter().any(|pinned| {
                let Some(bounty) = shooter::bounties::bounty(&pinned.id) else {
                    return true;
                };
                if pinned.have >= bounty.need {
                    return true;
                }
                match bounty.goal {
                    shooter::bounties::Goal::Reach(floor) => {
                        pinned.have != u32::from(depth >= floor)
                    }
                    shooter::bounties::Goal::Show(_) => audience > pinned.have,
                    _ => false,
                }
            });
            let bounty_can_be_pinned = realm.home.bounties.len() < shooter::bounties::PINNED
                && shooter::bounties::BOUNTIES.iter().any(|bounty| {
                    bounty.from <= depth.max(realm.home.deepest)
                        && !realm
                            .home
                            .bounties
                            .iter()
                            .any(|pinned| pinned.id == bounty.id)
                });
            let collection_achievement_can_be_earned = (!realm.home.feats.contains("full_hall")
                && shooter::trophies::PLINTHS
                    .iter()
                    .all(|plinth| realm.home.trophies.contains_key(plinth.id)))
                || (!realm.home.feats.contains("full_house")
                    && shooter::rescues::RESIDENTS
                        .iter()
                        .all(|resident| realm.home.residents.contains(resident.id)));
            let may_change = !orders.is_empty()
                || !peddled.is_empty()
                || !noticed.is_empty()
                || !marks.is_empty()
                || wardrobe != realm.home.wardrobe
                || country_games != realm.home.countryside.games
                || depth > realm.home.deepest
                || (depth == 0 && landing != realm.home.landing)
                || (audience > realm.home.best_show
                    && (show_over || audience >= realm.home.best_show + 100))
                || (unsaved && !fighting);
            (may_change
                || bounty_may_change
                || bounty_can_be_pinned
                || collection_achievement_can_be_earned)
                .then(|| realm.clone())
        };
        let realm = self.realm();
        let mut changed = wardrobe != realm.home.wardrobe;
        realm.home.wardrobe = wardrobe;
        if realm.home.privy_wall != privy_wall {
            realm.home.privy_wall = privy_wall;
            changed = true;
        }
        changed |= realm.home.countryside.games.merge(&country_games);
        let (country_changed, mut country_news) =
            shooter::countryside::settle(&mut realm.home.countryside, &mut realm.treasury, &marks);
        changed |= country_changed;
        let (games_changed, games_news) =
            shooter::countryside::settle_games(&mut realm.home.countryside, &mut realm.treasury, &marks);
        changed |= games_changed;
        country_news.extend(games_news);
        let (college_changed, college_news) =
            shooter::college::settle(&mut realm.home.college, &mut realm.treasury, &marks);
        changed |= college_changed;
        if depth > realm.home.deepest {
            realm.home.deepest = depth;
            changed = true;
        }
        if depth == 0 && landing != realm.home.landing {
            realm.home.landing = landing;
            changed = true;
        }
        // The best show, kept as it grows (a tenth of a million at a time)
        // and when the show is over.
        if audience > realm.home.best_show && (show_over || audience >= realm.home.best_show + 100)
        {
            realm.home.best_show = audience;
            changed = true;
        }
        let mut said: Vec<(String, String)> = country_news
            .into_iter()
            .map(|line| ("country".into(), line))
            .collect();
        let mut earned: Vec<&'static str> = Vec::new();
        // Wren's bounties: what the delve just did. A finished one is paid
        // on the spot, and Wren pins the next. Progress is saved between
        // fights, not on every kill.
        let board = realm.home.bounties.clone();
        let finished = realm.home.work_bounties(&marks, depth, audience);
        let mut paid = Vec::new();
        for bounty in &finished {
            let reward = bounty.reward();
            realm.treasury.merge(&reward);
            changed = true;
            paid.push(format!("{}, for {}", bounty.title, reward.label()));
        }
        let pinned = realm.home.pin_bounties();
        if !pinned.is_empty() {
            changed = true;
        }
        if !paid.is_empty() {
            let next = pinned.last().map_or(String::new(), |b| {
                format!(" Next on her board: {}.", b.title)
            });
            said.push((
                "bounty_paid".into(),
                format!("Wren's bounty done: {}.{next}", paid.join("; ")),
            ));
        } else if let Some(bounty) = pinned.last() {
            said.push((
                "bounty_pinned".into(),
                format!("Wren pins a bounty: {}. \"{}\"", bounty.title, bounty.says),
            ));
        }
        // The Trophy Hall: what fell, kept by the realm. Sir Kay says so the
        // first time a piece comes in.
        for (mark, &n) in &marks {
            let Some(id) = mark.strip_prefix("trophy:") else {
                continue;
            };
            let count = realm.home.trophies.entry(id.to_string()).or_default();
            let first = *count == 0;
            *count += n;
            changed = true;
            if first && let Some(plinth) = shooter::trophies::PLINTHS.iter().find(|p| p.id == id) {
                said.push((
                    "trophy_new".into(),
                    format!(
                        "Sir Kay has a new piece for the Trophy Hall: {}",
                        plinth.name
                    ),
                ));
            }
        }
        if shooter::trophies::PLINTHS
            .iter()
            .all(|p| realm.home.trophies.contains_key(p.id))
        {
            earned.push("full_hall");
        }
        // Rescues: whoever was let out of a cage goes home up the stair, and
        // stays.
        for mark in marks.keys() {
            let Some(who) = mark
                .strip_prefix("rescue:")
                .and_then(shooter::rescues::resident)
            else {
                continue;
            };
            if realm.home.residents.insert(who.id.to_string()) {
                changed = true;
                said.push((
                    "rescue_home".into(),
                    format!(
                        "{} is going home to the Undercroft, with {}",
                        who.name, who.gives
                    ),
                ));
            }
        }
        if shooter::rescues::RESIDENTS
            .iter()
            .all(|r| realm.home.residents.contains(r.id))
        {
            earned.push("full_house");
        }
        // Maud's round was drunk at the top of the stair: the realm's tab
        // is clear for the next.
        if marks.contains_key("round_drunk") && realm.home.round.take().is_some() {
            changed = true;
        }
        // And Sir Dinadan's song went down with them.
        if marks.contains_key("song_sung") && realm.home.song.take().is_some() {
            changed = true;
        }
        // And Beaumains, with his wage in his pocket.
        if marks.contains_key("hired") && realm.home.hire.take().is_some() {
            changed = true;
        }
        // King Brannoc's barony: his coming, the works, the forges relit, his
        // missions.
        let mines = realm
            .reclaimed
            .get(shooter::Pack::Cavern.name())
            .copied()
            .unwrap_or(0);
        let who = |id: u32| match names.get(&id).map(String::as_str) {
            Some("You") | None if id == 1 => host.clone(),
            Some(name) => name.to_string(),
            None => format!("Knight {id}"),
        };
        let mut treasury = realm.treasury.clone();
        let barony = shooter::barony::settle(&mut realm.home, &mut treasury, mines, &marks, &who);
        realm.treasury = treasury;
        if barony.changed {
            changed = true;
        }
        said.extend(barony.said);
        said.extend(college_news.into_iter().map(|line| ("college".into(), line)));
        earned.extend(barony.earned);
        // The games and places: the lists and their stables and market, the
        // Hippodrome's purses, the Court of Deeds, the ranch
        // (`activity::settle_all`).
        let games = shooter::activity::settle_all(
            &mut shooter::activity::Realm {
                home: &mut realm.home,
                treasury: &mut realm.treasury,
                reclaimed: &realm.reclaimed,
                raids_won: realm.raids_won,
            },
            &marks,
        );
        changed |= games.changed;
        said.extend(games.said);
        // Experience: every knight in the party learns from what fell. A
        // new level is a lesson waiting with Sir Ector.
        let xp = marks.get("xp").copied().unwrap_or(0);
        if xp > 0 {
            for (id, key) in &keys {
                let prowess = realm.home.knights.entry(key.clone()).or_default();
                let before = prowess.level();
                prowess.xp += xp;
                if prowess.level() > before {
                    let who = names.get(id).cloned().unwrap_or_else(|| key.clone());
                    said.push((
                        "level_up".into(),
                        format!(
                            "{who} reached level {}: Sir Ector has a lesson waiting in the Training Yard",
                            prowess.level()
                        ),
                    ));
                }
            }
        }
        // The Herald's Bestiary counts every kill.
        let mut counted = false;
        for (mark, &n) in &marks {
            if let Some(kind) = mark.strip_prefix("slay:") {
                *realm.home.bestiary.entry(kind.to_string()).or_default() += n;
                counted = true;
            }
        }
        let unsaved = unsaved || realm.home.bounties != board || xp > 0 || counted;
        if unsaved && !fighting {
            changed = true;
        }
        // Every goblin that got away; the first brings Grubbins home.
        let escapes = noticed.iter().filter(|f| **f == "sticky_fingers").count() as u32;
        if escapes > 0 {
            realm.home.goblins += escapes;
            changed = true;
        }
        // The surface peddler is paid from the treasury, as Grubbins is.
        let mut peddles = Vec::new();
        for &(purchase, gold) in &peddled {
            let mut cost = crate::drive::together_realm::Spoils::default();
            cost.add(crate::drive::together_realm::Spoil::Gold, gold);
            if !realm.treasury.covers(&cost) {
                said.push((
                    "cant_afford".into(),
                    format!(
                        "That's {gold} gold, and the treasury holds {}.",
                        realm
                            .treasury
                            .get(crate::drive::together_realm::Spoil::Gold)
                    ),
                ));
                continue;
            }
            realm.treasury.take(&cost);
            changed = true;
            peddles.push(purchase);
        }
        let mut sales: Vec<(usize, u32)> = Vec::new();
        let mut unboxed: Option<(shooter::feats::Tier, crate::drive::together_realm::Spoils)> =
            None;
        for order in &orders {
            use shooter::home::Station;
            if let Some(item) = match order.station {
                Station::StallA => Some(0),
                Station::StallB => Some(1),
                Station::StallC => Some(2),
                _ => None,
            } {
                // Grubbins sells for gold, and only gold.
                if stall.get(item).is_none_or(|c| c.is_empty()) {
                    continue;
                }
                let price = stall_prices[item];
                let mut cost = crate::drive::together_realm::Spoils::default();
                cost.add(crate::drive::together_realm::Spoil::Gold, price);
                if !realm.treasury.covers(&cost) {
                    said.push((
                        "cant_afford".into(),
                        format!(
                            "Grubbins: that's {price} gold, friend. You've got {}.",
                            realm
                                .treasury
                                .get(crate::drive::together_realm::Spoil::Gold)
                        ),
                    ));
                    continue;
                }
                realm.treasury.take(&cost);
                changed = true;
                sales.push((item, order.knight));
                said.push((
                    "grubbins_sold".into(),
                    format!("Grubbins sold you {} for {price} gold", stall_names[item]),
                ));
                continue;
            }
            if let Some(side) = match order.station {
                shooter::home::Station::LessonA => Some(0u8),
                shooter::home::Station::LessonB => Some(1),
                _ => None,
            } {
                // Sir Ector teaches the knight standing at his lectern.
                let Some(key) = keys.get(&order.knight) else {
                    continue;
                };
                match realm.home.learn(key, side) {
                    Ok(talent) => {
                        changed = true;
                        earned.push("first_lesson");
                        if realm.home.prowess(key).learned.len() == shooter::talents::LESSONS.len()
                        {
                            earned.push("master_of_arms");
                        }
                        let who = names
                            .get(&order.knight)
                            .cloned()
                            .unwrap_or_else(|| key.clone());
                        said.push((
                            "lesson_learned".into(),
                            format!(
                                "{who} learned {} from Sir Ector ({}): \"{}\"",
                                talent.name, talent.does, talent.says
                            ),
                        ));
                    }
                    Err(why) => said.push(("cant_afford".into(), why)),
                }
                continue;
            }
            if let Some(tap) = match order.station {
                shooter::home::Station::TapA => Some(0),
                shooter::home::Station::TapB => Some(1),
                shooter::home::Station::TapC => Some(2),
                _ => None,
            } {
                // Maud pours a round for the next delve: gold from the
                // treasury, one round at a time.
                let drink = &shooter::tavern::DRINKS[tap];
                if let Some(waiting) = realm.home.round.as_deref().and_then(shooter::tavern::drink)
                {
                    said.push((
                        "cant_afford".into(),
                        format!(
                            "Maud: you've a {} waiting already, love. Drink that first.",
                            waiting.name
                        ),
                    ));
                    continue;
                }
                let mut cost = crate::drive::together_realm::Spoils::default();
                cost.add(crate::drive::together_realm::Spoil::Gold, drink.price);
                if !realm.treasury.covers(&cost) {
                    said.push((
                        "cant_afford".into(),
                        format!("Maud: that's {} gold, love. The tab's closed.", drink.price),
                    ));
                    continue;
                }
                realm.treasury.take(&cost);
                realm.home.round = Some(drink.id.to_string());
                changed = true;
                said.push((
                    format!("round_poured:{}", drink.id),
                    format!(
                        "Maud poured {} for the next delve ({} gold): {}",
                        drink.name, drink.price, drink.does
                    ),
                ));
                continue;
            }
            if let Some(plate) = match order.station {
                shooter::home::Station::SongA => Some(0),
                shooter::home::Station::SongB => Some(1),
                shooter::home::Station::SongC => Some(2),
                _ => None,
            } {
                // Sir Dinadan sings for the next delve: gold from the
                // treasury, one song at a time.
                let song = &shooter::tavern::SONGS[plate];
                if let Some(waiting) = realm.home.song.as_deref().and_then(shooter::tavern::song) {
                    said.push((
                        "cant_afford".into(),
                        format!(
                            "Sir Dinadan: I've {} rehearsed already. One song at a time, friend.",
                            waiting.name
                        ),
                    ));
                    continue;
                }
                let mut cost = crate::drive::together_realm::Spoils::default();
                cost.add(crate::drive::together_realm::Spoil::Gold, song.price);
                if !realm.treasury.covers(&cost) {
                    said.push((
                        "cant_afford".into(),
                        format!(
                            "Sir Dinadan: {} gold, friend. Art isn't free. Mostly.",
                            song.price
                        ),
                    ));
                    continue;
                }
                realm.treasury.take(&cost);
                realm.home.song = Some(song.id.to_string());
                changed = true;
                said.push((
                    format!("song_asked:{}", song.id),
                    format!(
                        "Sir Dinadan will sing {} for the next delve ({} gold): {}",
                        song.name, song.price, song.does
                    ),
                ));
                continue;
            }
            if order.station == shooter::home::Station::Hire {
                // Beaumains takes a wage for the next delve: gold from the
                // treasury, once.
                if realm.home.hire.is_some() {
                    said.push((
                        "cant_afford".into(),
                        "Beaumains: I'm yours for the next one already, my lord. I'll be at the stair."
                            .into(),
                    ));
                    continue;
                }
                let wage = shooter::hireling::WAGE;
                let mut cost = crate::drive::together_realm::Spoils::default();
                cost.add(crate::drive::together_realm::Spoil::Gold, wage);
                if !realm.treasury.covers(&cost) {
                    said.push((
                        "cant_afford".into(),
                        format!(
                            "Beaumains: {wage} gold, my lord, and a hot meal. I'll throw in the meal."
                        ),
                    ));
                    continue;
                }
                realm.treasury.take(&cost);
                realm.home.hire = Some("beaumains".into());
                changed = true;
                said.push((
                    "hire_asked:beaumains".into(),
                    format!(
                        "Beaumains will go down with you for the next delve ({wage} gold): he throws knives and keeps his distance"
                    ),
                ));
                continue;
            }
            if order.station == shooter::home::Station::Coffer {
                // A box opened before Fortune's audience.
                if realm.home.boxes.is_empty() {
                    said.push((
                        "cant_afford".into(),
                        "The coffer is empty: achievements fill it.".into(),
                    ));
                    continue;
                }
                let tier = realm.home.boxes.remove(0);
                let spoils = tier.contents(realm.home.opened);
                realm.home.opened += 1;
                realm.treasury.merge(&spoils);
                changed = true;
                said.push((
                    format!("box_opened:{}", shooter::feats::tier_word(tier)),
                    format!("The {} held {}", tier.name(), spoils.label()),
                ));
                unboxed = Some((tier, spoils));
                continue;
            }
            let ladder = order.station.ladder();
            let who = match names.get(&order.knight).map(String::as_str) {
                Some("You") | None if order.knight == 1 => host.clone(),
                Some(name) => name.to_string(),
                None => format!("Knight {}", order.knight),
            };
            let mut treasury = realm.treasury.clone();
            match realm.home.buy(order.station, &mut treasury) {
                Ok(level) => {
                    realm.treasury = treasury;
                    changed = true;
                    earned.push("home_improvement");
                    said.push((
                        format!("built:{}", order.station.word()),
                        format!(
                            "{who} built {} {}: {}",
                            ladder.name,
                            shooter::home::numeral(level),
                            ladder.rungs[usize::from(level) - 1].says
                        ),
                    ));
                }
                Err(why) => said.push(("cant_afford".into(), why)),
            }
        }
        // Achievements: each kept once, each with its box.
        let mut announced = Vec::new();
        for id in noticed.iter().copied().chain(earned.iter().copied()) {
            if let Some(feat) = shooter::feats::feat(id)
                && realm.home.feats.insert(id.to_string())
            {
                realm.home.boxes.push(feat.tier);
                changed = true;
                announced.push(feat);
            }
        }
        let saved = if changed { realm.save() } else { Ok(()) };
        if let Err(error) = saved {
            if let Some(realm_before) = realm_before {
                self.dungeon.realm = Some(realm_before);
            }
            self.dungeon.bounties_unsaved = unsaved;
            // Back into the run, beside whatever it noted meanwhile.
            if let Some(mut run) = self.dungeon.shooter.as_mut() {
                let run = &mut *run;
                run.orders.splice(0..0, orders);
                run.peddled.splice(0..0, peddled.iter().map(|&(p, _)| p));
                run.feats.splice(0..0, noticed);
                for (mark, n) in marks {
                    *run.marks.entry(mark).or_default() += n;
                }
            }
            self.dungeon.notice = format!("Could not save the realm: {error}");
            return false;
        }
        let (home, treasury) = (realm.home.clone(), realm.treasury.clone());
        // What angelX's work did to the holdings reaches the run without a
        // full rebuild.
        if let Some(mut run) = self.dungeon.shooter.as_mut()
            && run.home.holdings != home.holdings
        {
            run.home.holdings = home.holdings.clone();
        }
        self.dungeon.bounties_unsaved = unsaved && !changed;
        if let Some(mut run) = self.dungeon.shooter.as_mut()
            && (changed
                || run.treasury != treasury
                || run.home.levels != home.levels
                || run.home.countryside != home.countryside
                || run.home.college != home.college
                || (!fighting && run.home.knights != home.knights))
        {
            run.rebuild_home(home, treasury);
        }
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            for &(item, knight) in &sales {
                run.sell(item, knight);
            }
            for &purchase in &peddles {
                run.peddle(purchase);
            }
            for feat in &announced {
                run.announce(feat.id);
            }
            if let Some((tier, spoils)) = &unboxed {
                run.unbox(*tier, spoils);
            }
        }
        let now = std::time::Instant::now();
        let built_or_opened = !said.is_empty();
        for (cue, notice) in said {
            self.dungeon.chorus.cue(&cue, now);
            self.dungeon.notice = notice;
        }
        if let Some(feat) = announced.last() {
            // After what was just built or opened, if anything was; whole,
            // with the Herald's line, if not.
            self.dungeon.notice = if built_or_opened {
                format!(
                    "{} · Achievement: {} ({})",
                    self.dungeon.notice,
                    feat.name,
                    feat.tier.name()
                )
            } else {
                format!(
                    "NEW ACHIEVEMENT: {}. {} ({} in the Herald's coffer, Fortune's hall)",
                    feat.name,
                    feat.says,
                    feat.tier.name()
                )
            };
        }
        true
    }

    /// angelX's own work since the last settle, folded into the holdings:
    /// stores, walls, labour, scrolls, retainers, patrols and unrest (never
    /// gold), saved with the realm and shown in the open run.
    pub(crate) fn flush_holdings_ops(&mut self) {
        if self.dungeon.ops.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.dungeon.ops);
        let realm = self.realm();
        let said = shooter::holdings::feed(&mut realm.home.holdings, &ops);
        // A failed save keeps the change in memory; the next save carries it.
        let _ = realm.save();
        let holdings = realm.home.holdings.clone();
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            run.home.holdings = holdings;
        }
        if let Some(line) = said.last() {
            self.dungeon.notice = line.clone();
        }
    }

    /// A tool call starts: a subagent is a retainer at once; a commit, a
    /// test run or a compaction waits for its result.
    pub(crate) fn note_ops_call(&mut self, id: &str, name: &str, args: &str) {
        match op_of_call(name, args) {
            Some((op, true)) => self.dungeon.ops.note(op),
            Some((op, false)) if self.dungeon.ops_calls.len() < 64 => {
                self.dungeon.ops_calls.insert(id.to_string(), op);
            }
            _ => {}
        }
    }

    /// A tool call's result: counted for the holdings when it came back clean.
    pub(crate) fn note_ops_result(&mut self, id: &str, clean: bool) {
        if let Some(op) = self.dungeon.ops_calls.remove(id)
            && clean
        {
            self.dungeon.ops.note(op);
        }
    }

    /// A turn's spend: its fresh tokens and cache hits, and whether it was
    /// slow.
    pub(crate) fn note_ops_turn(&mut self, cache: u64, fresh: u64, secs: u64) {
        let ops = &mut self.dungeon.ops;
        ops.cache += cache;
        ops.tokens += fresh;
        if secs >= shooter::holdings::SLOW_TURN_SECS {
            ops.slow += 1;
        }
    }

    /// A turn ends: a failed one stirs unrest; calls that never reported
    /// are forgotten; the holdings hear it all.
    pub(crate) fn note_ops_end(&mut self, ok: bool) {
        if !ok {
            self.dungeon.ops.errors += 1;
        }
        self.dungeon.ops_calls.clear();
        self.flush_holdings_ops();
    }

    /// The loop's iterations so far: each new one is a patrol.
    pub(crate) fn note_ops_loop(&mut self, iteration: usize) {
        if iteration > self.dungeon.ops_loop {
            let new = (iteration - self.dungeon.ops_loop).min(64) as u32;
            self.dungeon.ops.loops += new;
        }
        self.dungeon.ops_loop = iteration;
    }

    /// When angelX's drafting turn ends, read its draft in — no command needed.
    pub(crate) fn watch_drafting(&mut self) {
        let Some((id, started, seen)) = self.dungeon.drafting.clone() else {
            return;
        };
        let busy = self.thinking.is_some() || self.pending_turn.is_some();
        if busy {
            if !seen {
                self.dungeon.drafting = Some((id, started, true));
            }
            return;
        }
        if !seen && started.elapsed() < std::time::Duration::from_secs(10) {
            return;
        }
        self.dungeon.drafting = None;
        let path = self.wishes_dir().join(format!("{id}.wish"));
        self.dungeon.notice = if path.is_file() {
            let report = self.dungeon_wishes("reload");
            let line = report
                .lines()
                .find(|l| l.starts_with(&format!("{id}:")))
                .unwrap_or(&report)
                .to_string();
            format!("angelX drafted the wish — {line}. Tab → realm to see its price.")
        } else {
            format!(
                "angelX's turn ended without {}; /dungeon grant it again to retry.",
                path.display()
            )
        };
    }

    /// Where angelX writes a wish's draft in this workspace.
    pub(super) fn wishes_dir(&self) -> std::path::PathBuf {
        self.tools.current_workspace().join(".angel/realm/wishes")
    }

    /// A wish in someone's own words.
    pub(crate) fn ask_wish(&mut self, by: &str, words: &str) -> String {
        let result = self.realm().ask(by, words);
        let message = match result {
            Ok(id) => {
                let _ = self.realm().save();
                format!(
                    "{by} wished for “{}”. /dungeon grant {id} asks angelX to draft it.",
                    words.trim()
                )
            }
            Err(error) => error,
        };
        message
    }

    /// Grant a wish by id or list number: draft it with angelX when it is
    /// only words, raise it when it is drafted and paid for.
    pub(crate) fn grant_wish(&mut self, which: &str) -> String {
        let which = which.trim();
        let id = {
            let realm = self.realm();
            which
                .parse::<usize>()
                .ok()
                .and_then(|n| realm.wishes.get(n.wrapping_sub(1)))
                .or_else(|| realm.wishes.iter().find(|w| w.id == which))
                .map(|w| w.id.clone())
        };
        let Some(id) = id else {
            return format!("No wish `{which}`. /dungeon wishes lists them.");
        };
        let status = self
            .realm()
            .wishes
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.status);
        if status == Some(Status::Asked) {
            let dir = self.wishes_dir();
            let path = dir.join(format!("{id}.wish"));
            let realm = self.realm();
            let wish = realm
                .wishes
                .iter()
                .find(|w| w.id == id)
                .cloned()
                .expect("found");
            let brief = realm::brief(&wish, &path, &realm.treasury);
            let _ = std::fs::create_dir_all(&dir);
            // Hand the wish to angelX in the open: the party watches it draft.
            self.collapse_dungeon();
            self.input = brief;
            self.cursor = self.input.len();
            self.submit();
            self.dungeon.drafting = Some((id.clone(), std::time::Instant::now(), false));
            return format!(
                "angelX is drafting “{}”. Its draft is read in when the turn ends; then /dungeon grant {id} raises it once the treasury can pay.",
                wish.words
            );
        }
        let result = self.realm().grant(&id);
        let message = match result {
            Ok(message) => {
                let _ = self.realm().save();
                self.dungeon
                    .chorus
                    .cue(&format!("wish_raised:{id}"), std::time::Instant::now());
                // Say so when the realm has no ground left to stand it on,
                // rather than raise it out of sight.
                let homeless =
                    crate::stage::world_viz::overworld::wishes::unplaced(&self.realm().built());
                if homeless.is_empty() {
                    message
                } else {
                    format!(
                        "{message}, but the realm has no free ground to show {} yet",
                        homeless.join(", ")
                    )
                }
            }
            Err(error) => error,
        };
        self.dungeon.notice = message.clone();
        message
    }

    /// `/dungeon wishes [reload]`: the wishing stone, or angelX's drafts read in.
    pub(crate) fn dungeon_wishes(&mut self, tail: &str) -> String {
        if tail == "reload" {
            let dir = self.wishes_dir();
            let mut report = Vec::new();
            let mut paths: Vec<_> = std::fs::read_dir(&dir)
                .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
                .unwrap_or_default();
            paths.retain(|p: &std::path::PathBuf| {
                p.extension().is_some_and(|e| e == "wish") && p.is_file()
            });
            paths.sort();
            for path in paths {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let raw = match std::fs::metadata(&path) {
                    Ok(m) if m.len() as usize <= realm::MAX_BYTES => {
                        std::fs::read_to_string(&path).unwrap_or_default()
                    }
                    _ => {
                        report.push(format!("{stem}: too large"));
                        continue;
                    }
                };
                // Pictures named by `sheet art <file>` are cut into rows.
                let sheets = self.tools.current_workspace().join(".angel/realm/sheets");
                let raw = match crate::drive::together_sheets::inline(&raw, &sheets, realm::ART as u32) {
                    Ok(raw) => raw,
                    Err(errors) => {
                        report.push(format!("{stem}: {}", errors.join("; ")));
                        continue;
                    }
                };
                match realm::check(&stem, &raw) {
                    Ok((draft, notes)) => {
                        let name = draft.name.clone();
                        match self.realm().draft(draft) {
                            Ok(()) if notes.is_empty() => {
                                report.push(format!("{stem}: {name} drafted"))
                            }
                            Ok(()) => report
                                .push(format!("{stem}: {name} drafted ({})", notes.join("; "))),
                            Err(error) => report.push(format!("{stem}: {error}")),
                        }
                    }
                    Err(errors) => report.push(format!("{stem}: {}", errors.join("; "))),
                }
            }
            let _ = self.realm().save();
            return if report.is_empty() {
                format!("No drafts in {}.", dir.display())
            } else {
                report.join("\n")
            };
        }
        let realm = self.realm();
        let mut out = vec![format!("Treasury: {}", realm.treasury.label())];
        for (i, wish) in realm.wishes.iter().enumerate() {
            let line = match wish.status {
                Status::Asked => format!(
                    "{}. “{}” — {} wished it; /dungeon grant {} has angelX draft it",
                    i + 1,
                    wish.words,
                    wish.by,
                    i + 1
                ),
                Status::Drafted => {
                    let short = realm.treasury.shortfall(&wish.price);
                    format!(
                        "{}. {} — costs {}{}",
                        i + 1,
                        wish.name,
                        wish.price.label(),
                        if short.is_empty() {
                            format!(" · ready: /dungeon grant {}", i + 1)
                        } else {
                            format!(" · {short}")
                        }
                    )
                }
                Status::Built => format!("{}. {} — stands in the realm", i + 1, wish.name),
            };
            out.push(line);
        }
        let sources: Vec<String> = realm::Spoil::ALL
            .iter()
            .map(|s| format!("{} from {}", s.word(), s.source()))
            .collect();
        out.push(format!(
            "/dungeon wish <your words> makes a new wish · spoils: {}",
            sources.join(", ")
        ));
        out.join("\n")
    }
}

/// What a tool call is to the holdings, by the herald's reading of it: a
/// commit, a test run, a compaction (each counted when it comes back
/// clean), or a subagent sent out (counted as it goes).
pub(crate) fn op_of_call(name: &str, args: &str) -> Option<(shooter::holdings::Op, bool)> {
    use crate::stage::world_viz::{Deed as Work, herald};
    use shooter::holdings::Op;
    let leaf = name.rsplit(['.', ':', '/']).next().unwrap_or(name).to_ascii_lowercase();
    if matches!(leaf.as_str(), "spawn" | "delegate") {
        return Some((Op::Subagents, true));
    }
    if leaf.contains("compact") {
        return Some((Op::Compactions, false));
    }
    let told = herald(name, args);
    match told.deed {
        Work::Seal => Some((Op::Commits, false)),
        Work::Trial if told.object.contains("test") => Some((Op::Tests, false)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_holdings__tests.rs"]
mod holdings_tests;
