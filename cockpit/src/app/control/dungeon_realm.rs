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
        let Some(run) = self.dungeon.shooter.as_mut() else {
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
            if let Some(run) = self.dungeon.shooter.as_mut() {
                run.bank = hauls;
                run.reclaimed = reclaimed;
                run.triumph = triumph;
            }
            self.dungeon.notice = format!("Could not save the realm: {error}");
            let _ = self.dungeon.checkpoint(std::time::Instant::now(), true);
            self.publish_realm();
            return false;
        }
        let now = std::time::Instant::now();
        if !hauls.is_empty() {
            self.dungeon.chorus.cue("banked", now);
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
        self.publish_realm();
        true
    }

    /// Pay for what the party asked for at the Undercroft's plates out of
    /// the treasury, remember the deepest floor reached and the landing
    /// chosen, and tell the run what now stands and what it can spend.
    pub(crate) fn settle_home(&mut self) -> bool {
        let Some(run) = self.dungeon.shooter.as_mut() else {
            return true;
        };
        let orders = std::mem::take(&mut run.orders);
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
                        && !realm.home.bounties.iter().any(|pinned| pinned.id == bounty.id)
            });
            let collection_achievement_can_be_earned =
                (!realm.home.feats.contains("full_hall")
                    && shooter::trophies::PLINTHS
                        .iter()
                        .all(|plinth| realm.home.trophies.contains_key(plinth.id)))
                    || (!realm.home.feats.contains("full_house")
                        && shooter::rescues::RESIDENTS
                            .iter()
                            .all(|resident| realm.home.residents.contains(resident.id)));
            let may_change = !orders.is_empty()
                || !noticed.is_empty()
                || !marks.is_empty()
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
        let mut changed = false;
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
        let mut said: Vec<(String, String)> = Vec::new();
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
                            realm.treasury.get(crate::drive::together_realm::Spoil::Gold)
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
            if let Some(run) = self.dungeon.shooter.as_mut() {
                run.orders = orders;
                run.feats = noticed;
                run.marks = marks;
            }
            self.dungeon.notice = format!("Could not save the realm: {error}");
            self.publish_realm();
            return false;
        }
        let (home, treasury) = (realm.home.clone(), realm.treasury.clone());
        self.dungeon.bounties_unsaved = unsaved && !changed;
        if let Some(run) = self.dungeon.shooter.as_mut()
            && (changed
                || run.treasury != treasury
                || run.home.levels != home.levels
                || (!fighting && run.home.knights != home.knights))
        {
            run.rebuild_home(home, treasury);
        }
        if let Some(run) = self.dungeon.shooter.as_mut() {
            for &(item, knight) in &sales {
                run.sell(item, knight);
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
        self.publish_realm();
    }

    /// The guest page shows the treasury and the wishes.
    pub(crate) fn publish_realm(&mut self) {
        if self.dungeon.guest.is_none() {
            return;
        }
        // Redraw the realm picture when a wish rises (and once per guest).
        let standing = self.realm().built().len();
        if self.dungeon.realm_drawn != Some(standing) {
            self.dungeon.realm_drawn = Some(standing);
            let img = crate::stage::world_viz::overworld::wish_view(&self.world.overworld_scene());
            if let Some(server) = &self.dungeon.guest {
                server.set_realm_picture(&img.rgb_bytes(), img.w as u32, img.h as u32);
            }
        }
        let drafting = self.dungeon.drafting.as_ref().map(|(id, _, _)| id.clone());
        let view = crate::drive::together_guest::RealmHud::of(self.realm(), drafting);
        if let Some(server) = &self.dungeon.guest {
            server.set_realm(view);
        }
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
        self.publish_realm();
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
            self.publish_realm();
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
                message
            }
            Err(error) => error,
        };
        self.dungeon.notice = message.clone();
        self.publish_realm();
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
            self.publish_realm();
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
