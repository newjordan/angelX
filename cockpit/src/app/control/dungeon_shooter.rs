//! Real-time controls and a bounded clock, isolated from harness input ownership.
use super::*;
use crate::drive::together_guest::{self as guest, GuestServer};
use crate::drive::together_shooter::{self as shooter, Input as ShooterInput};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const DUNGEON_HELP: &str = "\
The Delve — knights against monsters, for spoils that build the realm.
/dungeon                send your knight to the Delve's gate on the map; click the mini-viz there (or F4) to choose a knight and enter
/dungeon start [name]   skip the walk and begin at once (a name adds a second knight on this keyboard)
/dungeon_host --N [--view]  host for N friends (up to 3): one invite line each, to paste into their own angelX; --view lets a link open a read-only browser view · /dungeon invite|kick
/dungeon stable       host-local stalls · select/tend a mount · tournament: three practice passes
/dungeon join <line>    play in a friend's delve from your angelX · /dungeon leave goes home
/dungeon settlement     visit the saved loop site (new run only); west from the PLAYER HALL
/dungeon deposit [name] host-only admission after a saved loop receipt: .angelX/settlement-exhibits/<name>.json; no name reads legacy .angelX/settlement-exhibits.json
/dungeon resume|pause|status|off      F4/F6 expands, Esc returns to coding (a hosted friend plays on); off ends the run, keeping half of what you carried
/dungeon wishes [reload] · wish <words> · grant <n>   the treasury and the wishes it pays for
/dungeon cards [new <name>|reload] · card <file> · bosses   make your own cards and guardians
/dungeon voices on|off|<0-100>
Keys: WASD move · arrows aim and fire (F fires too) · Space rolls, or holds a shield up · E bomb · 1–4 play a card · G keeps vigil when you step away (stone, safe, mending knights near you; G again wakes) · Tab cards and realm · V voices · R replays after a win or a fall.
Player 2 on this keyboard: IJKL move · Enter fires · O rolls/shields · U bomb.";

const STEP: Duration = Duration::from_nanos(1_000_000_000 / shooter::HZ as u64);
const MAX_STEPS: u32 = 4;
const KEY_LEASE: Duration = Duration::from_millis(180);
/// A tapped move key where the terminal sends no key releases: about one
/// knight's width of walking. A key the terminal repeats keeps the whole lease.
const TAP_LEASE: Duration = Duration::from_millis(95);
/// A remote friend's held controls lapse this long after their last renewal
/// (clients renew every 80 ms), so a dropped connection stops the knight. It
/// leaves room for a real network's round trips, not only loopback's.
const GUEST_LEASE: Duration = Duration::from_millis(500);
/// The listener's default port when no address is given.
const DEFAULT_PORT: u16 = 8767;

impl DungeonView {
    /// A key came down, or the terminal repeated one that already was.
    pub(super) fn hold(&mut self, code: KeyCode, shift: bool) {
        let at = Instant::now();
        let repeat = self
            .held
            .get(&code)
            .is_some_and(|held| at.saturating_duration_since(held.at) < KEY_LEASE);
        self.held
            .insert(code, super::dungeon::Held { at, shift, repeat });
    }

    pub(super) fn local_inputs(&mut self, now: Instant) -> BTreeMap<u32, ShooterInput> {
        if !self.key_releases {
            self.held
                .retain(|_, held| now.saturating_duration_since(held.at) < KEY_LEASE);
        }
        let releases = self.key_releases;
        let down = |key| {
            i8::from(self.held.get(&key).is_some_and(|held| {
                releases
                    || held.repeat
                    || !matches!(
                        key,
                        KeyCode::Char('w' | 'a' | 's' | 'd' | 'i' | 'j' | 'k' | 'l')
                    )
                    || now.saturating_duration_since(held.at) < TAP_LEASE
            }))
        };
        let p1 = ShooterInput {
            move_x: down(KeyCode::Char('d')) - down(KeyCode::Char('a')),
            move_y: down(KeyCode::Char('s')) - down(KeyCode::Char('w')),
            aim_x: down(KeyCode::Right) - down(KeyCode::Left),
            aim_y: down(KeyCode::Down) - down(KeyCode::Up),
            fire: down(KeyCode::Char('f')) != 0,
            // Space is the guard key: roll, or hold the shield up.
            dash: down(KeyCode::Char(' ')) != 0
                || self.held.iter().any(|(code, held)| {
                    held.shift && matches!(code, KeyCode::Char('w' | 'a' | 's' | 'd'))
                }),
            bomb: down(KeyCode::Char('e')) != 0,
            swing: down(KeyCode::Char('q')) != 0,
            cast: ['z', 'b', 'n']
                .iter()
                .position(|c| down(KeyCode::Char(*c)) != 0)
                .map_or(0, |i| i as u8 + 1),
            play: (1..=4)
                .find(|&n| down(KeyCode::Char(char::from(b'0' + n))) != 0)
                .unwrap_or(0),
            vigil: down(KeyCode::Char('g')) != 0,
            ult: down(KeyCode::Char('r')) != 0,
        };
        let mut p2 = ShooterInput {
            move_x: down(KeyCode::Char('l')) - down(KeyCode::Char('j')),
            move_y: down(KeyCode::Char('k')) - down(KeyCode::Char('i')),
            fire: down(KeyCode::Enter) != 0,
            dash: down(KeyCode::Char('o')) != 0,
            bomb: down(KeyCode::Char('u')) != 0,
            swing: down(KeyCode::Char('p')) != 0,
            cast: (5..=7)
                .find(|n| down(KeyCode::Char(char::from(b'0' + n))) != 0)
                .map_or(0, |n| n - 4),
            ult: down(KeyCode::Char('y')) != 0,
            ..Default::default()
        };
        if p2.fire {
            p2.aim_x = p2.move_x;
            p2.aim_y = p2.move_y;
        }
        BTreeMap::from([(1, p1), (2, p2)])
    }
}

impl App {
    pub(crate) fn clear_dungeon_controls(&mut self) {
        self.dungeon.held.clear();
        self.dungeon.remote.clear();
        self.dungeon.clock = None;
    }

    /// The host's delve as the screen shows it now: part of the way from
    /// the tick before to the latest, by the time since that tick was due,
    /// with the quarter-tick step it shows.
    pub(crate) fn dungeon_view_run(&self, now: Instant) -> Option<(shooter::Run, u64)> {
        let run = self.dungeon.shooter.as_ref()?;
        let alpha = match self.dungeon.clock {
            Some(due) if self.dungeon_wants_fast_tick() => {
                (now.saturating_duration_since(due).as_secs_f32() / STEP.as_secs_f32()).min(1.0)
            }
            _ => 1.0,
        };
        let step = run.tick * 4 + (alpha * 3.0) as u64;
        Some((run.between(&self.dungeon.before, alpha), step))
    }

    pub(crate) fn dungeon_wants_fast_tick(&self) -> bool {
        self.dungeon
            .shooter
            .as_ref()
            .is_some_and(shooter::Run::active)
            && self.dungeon.chivalry_visit.is_none()
            && (self.dungeon_keyboard_active() || self.dungeon_friends_seated())
    }

    /// A hosted delve with a friend's knight in it plays on while the host is
    /// away; with no friend seated yet it pauses like a solo run.
    pub(crate) fn dungeon_friends_seated(&self) -> bool {
        self.dungeon.guest.is_some()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| run.players.keys().any(|&id| id != 1))
    }

    pub(crate) fn dungeon_keyboard_active(&self) -> bool {
        self.dungeon.shooter.is_some()
            && self.dungeon_view_active()
            && self.dungeon.controls_visible
            && !self.dungeon.cards_open
            && self.dungeon.chivalry_visit.is_none()
            && !self.dungeon_controls_blocked()
            && self.terminal_focused
    }

    /// Where this workspace keeps its own delve cards.
    pub(super) fn cards_dir(&self) -> std::path::PathBuf {
        self.tools.current_workspace().join(".angel/dungeon/cards")
    }

    /// Read the workspace's cards into the open run's book.
    pub(super) fn reload_cards(&mut self) -> Vec<String> {
        let dir = self.cards_dir();
        let Some(run) = self.dungeon.shooter.as_mut() else {
            return Vec::new();
        };
        shooter::cards::load_dir(&dir, &mut run.book)
    }

    pub(super) fn start_shooter(&mut self, guest: Option<&str>) {
        if let Some(run) = self.dungeon.shooter.as_mut() {
            run.retreat();
        }
        if !self.settle_realm() || !self.settle_home() {
            return;
        }
        // The knight waits at the Delve's lit gate while the party is below.
        self.world.delve_called = true;
        self.world.delve_lit = true;
        self.dungeon.chivalry_visit = None;
        self.dungeon.chivalry_notice.clear();
        self.world.close_chivalry();
        self.sync_chivalry_projection();
        self.realm();
        let player = self.host_name();
        self.world.load_settlement(&player);
        let tavern = self.realm().home.level(shooter::home::Station::Wing) >= 1;
        self.world.sync_settlement(&self.loop_ctl, &player, tavern);
        let gear: Vec<_> = self
            .dungeon
            .shooter
            .as_ref()
            .map(|r| {
                r.players
                    .iter()
                    .map(|(&id, h)| (id, h.forged.clone(), h.avatar.clone()))
                    .collect()
            })
            .unwrap_or_default();
        // Friends seated through the host's invitations come along into the
        // next delve as the same knights.
        let friends: Vec<(u32, String, Option<String>)> =
            if self.dungeon.guest.is_some() && guest.is_none() {
                self.dungeon
                    .shooter
                    .as_ref()
                    .map(|r| {
                        r.players
                            .iter()
                            .filter(|(id, _)| **id != 1)
                            .map(|(&id, h)| (id, h.name.clone(), h.knight.clone()))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
        self.dungeon.raid_serial = self.dungeon.raid_serial.wrapping_add(1);
        // Every delve begins in the Undercroft, as the realm has built it.
        let (home, treasury) = {
            let realm = self.realm();
            (realm.home.clone(), realm.treasury.clone())
        };
        self.dungeon.shooter = Some(shooter::Run::at_home(
            self.world.realm_seed(),
            self.dungeon.raid_serial,
            guest,
            home,
            treasury,
        ));
        self.sync_chivalry_projection();
        if let Some(run) = self.dungeon.shooter.as_mut() {
            for (id, name, knight) in friends {
                run.join_seat(id, &name);
                if let Some(knight) = knight.as_deref().and_then(shooter::knights::knight) {
                    run.outfit(id, knight);
                }
            }
            for (id, weapon, avatar) in gear {
                if let Some(hero) = run.players.get_mut(&id) {
                    if weapon.is_some() {
                        hero.arm = None;
                    }
                    hero.forged = weapon;
                    hero.avatar = avatar;
                }
            }
        }
        // At admission only, read the identical durable site the passive camera
        // sees. Neither loop frames nor receipts mutate a running Delve.
        if let Ok(Some(site)) = self.world.settlement_site()
            && site.associated()
            && let Some(run) = self.dungeon.shooter.as_mut()
            && let Err(error) = run.enter_settlement(site)
        {
            tracing::warn!(%error, "settlement admission failed");
        }
        let boss_dir = self.tools.current_workspace().join(".angel/dungeon/bosses");
        let bosses = self
            .dungeon
            .shooter
            .as_mut()
            .map(|run| shooter::bosses::load_dir(&boss_dir, &mut run.bosses))
            .unwrap_or_default();
        let mut loaded = self.reload_cards();
        loaded.extend(bosses);
        if loaded
            .iter()
            .any(|line| line.contains(';') || line.contains("skipped") || line.contains("full"))
        {
            self.dungeon.notice = format!("Cards: {}", loaded.join(" · "));
            self.expand_dungeon();
            return;
        }
        self.dungeon.notice = match self.world.settlement_site() {
            Err(error) => format!("Settlement unavailable ({error}); standard Undercroft opened, saved site untouched."),
            Ok(Some(site)) if site.associated() => format!("{}'s PLAYER HALL · settlement west · the Winding Stair goes down", site.player),
            _ => "The Undercroft · stand on a plate and hold F to build · the Winding Stair goes down".into(),
        };
        self.expand_dungeon();
    }

    /// `/dungeon_host`: open the invitations, then the Delve's menu over the
    /// party's entrance room, so friends can join at once while the host
    /// chooses to continue or to begin anew with another knight.
    pub(super) fn host_from_command(&mut self, tail: &str) -> String {
        let message = self.host_dungeon(tail);
        if self.dungeon.guest.is_some() {
            self.open_hosted_intro();
            return format!(
                "{message}\nThe Delve's menu is open: Enter continues into the entrance room."
            );
        }
        message
    }

    /// Open the delve to friends, one seat each: a fresh delve, or the open
    /// one while the party still stands in its entrance room.
    pub(super) fn host_dungeon(&mut self, tail: &str) -> String {
        if let Some(server) = &self.dungeon.guest {
            let lines = server
                .invitation_urls()
                .iter()
                .enumerate()
                .map(|(i, url)| format!("  friend {}:  /dungeon join {url}", i + 1))
                .collect::<Vec<_>>()
                .join("\n");
            return format!("Dungeon already hosted. Invitations:\n{lines}");
        }
        // `--3` (or `-3`) opens three seats; a lone name keeps the old form.
        // `--view` also lets the links open a read-only view in a browser.
        let mut seats = 1usize;
        let mut view = false;
        let mut rest = Vec::new();
        for word in tail.split_whitespace() {
            match word.trim_start_matches('-').parse::<usize>() {
                Ok(n) if word.starts_with('-') => seats = n.clamp(1, guest::MAX_SEATS),
                _ if word == "--view" => view = true,
                _ => rest.push(word),
            }
        }
        let tail = rest.join(" ");
        let tail = tail.as_str();
        let default_bind = std::env::var("ANGEL_DUNGEON_LISTEN")
            .unwrap_or_else(|_| guest::default_bind(DEFAULT_PORT));
        let (name, bind) = if tail.parse::<std::net::SocketAddr>().is_ok() {
            ("", tail)
        } else if let Some((name, address)) = tail.rsplit_once(char::is_whitespace)
            && address.parse::<std::net::SocketAddr>().is_ok()
        {
            (name.trim(), address)
        } else {
            (tail, default_bind.as_str())
        };
        let server = match GuestServer::start_seats(bind, seats) {
            Ok(server) => server,
            Err(error) => return format!("Dungeon host: {error}"),
        };
        if view {
            server.set_browser_view(true);
        }
        let viewing = if server.browser_view() {
            "\nOpened in a browser, a link shows a read-only view of the floor; playing still takes angelX."
        } else {
            ""
        };
        // Friends join a delve still in its entrance room; otherwise hosting
        // begins a fresh one.
        if self
            .dungeon
            .shooter
            .as_ref()
            .is_none_or(|run| !run.active() || !run.at_entrance())
        {
            self.start_shooter(None);
        }
        // A named single guest joins at once; otherwise each friend's knight
        // joins when they connect and say who they are.
        if seats == 1
            && !name.is_empty()
            && let Some(run) = self.dungeon.shooter.as_mut()
        {
            run.join(name);
            self.dungeon.notice = format!("{name} is invited");
        }
        self.expand_dungeon();
        let lines = server
            .invitation_urls()
            .iter()
            .enumerate()
            .map(|(i, url)| format!("  friend {}:  /dungeon join {url}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let reach = if server.local_only() {
            "These links open only on this machine (no tailnet or local network found); give /dungeon host an address:port your friends can reach.".to_string()
        } else if guest::tailnet_address() == Some(server.address().ip()) {
            "Listening on your tailnet: friends on your Tailscale can join; nothing is opened to the internet.".to_string()
        } else {
            "Listening on your local network: friends on it can join; nothing is opened to the internet.".to_string()
        };
        self.dungeon.guest = Some(server);
        self.dungeon.chivalry_visit = None;
        self.dungeon.chivalry_notice.clear();
        self.world.close_chivalry();
        self.sync_chivalry_projection();
        self.publish_to_guest(self.dungeon_wants_fast_tick());
        format!(
            "Delve hosted with {seats} seat{}. Send each friend their own line — they paste it into their angelX composer:\n{lines}\n{reach}{viewing}\nUntil a friend arrives, Esc pauses; once they play, Esc turns your knight to stone while they play on. F6 rejoins. /dungeon invite shows these again; /dungeon kick ends the invitations.",
            if seats == 1 { "" } else { "s" }
        )
    }

    /// Revoke the invitation and take player 2 out of the delve.
    fn kick_guest(&mut self) -> String {
        if self.dungeon.guest.take().is_none() {
            return "No guest is hosted. /dungeon host [guest-name] invites one.".into();
        }
        self.dungeon.remote.clear();
        let name = self.dungeon.shooter.as_mut().and_then(|run| {
            let names: Vec<String> = (2..=crate::drive::together_shooter::MAX_PLAYERS)
                .filter_map(|seat| {
                    let name = run.players.get(&seat).map(|hero| hero.name.clone());
                    run.leave(seat);
                    name
                })
                .collect();
            (!names.is_empty()).then(|| names.join(", "))
        });
        self.sync_chivalry_projection();
        let _ = self.dungeon.checkpoint(Instant::now(), true);
        self.redraw_requested = true;
        format!(
            "{} left the delve; the invitation link no longer works. /dungeon host [guest-name] invites someone again.",
            name.as_deref().unwrap_or("Your guest")
        )
    }

    /// The guest sees what the host sees: the HUD now, the room's pixels at
    /// most every other tick.
    fn publish_to_guest(&self, active: bool) {
        if let (Some(server), Some(run)) = (&self.dungeon.guest, &self.dungeon.shooter) {
            let found = run.found_line();
            server.publish(
                run,
                !active,
                found.as_deref().unwrap_or(&self.dungeon.notice),
            );
        }
    }

    fn inspect_settlement_exhibit(&mut self) {
        let result = (|| {
            if self.dungeon.joined.is_some() {
                return Err("research sources are host-local only".into());
            }
            let site = self
                .world
                .settlement_site()
                .map_err(str::to_owned)?
                .ok_or("no saved settlement")?;
            let run = self
                .dungeon
                .shooter
                .as_ref()
                .ok_or("start the settlement Delve first")?;
            site.inspect(run, self.tools.current_workspace())
        })();
        self.clear_dungeon_controls();
        self.dungeon.exhibit_scroll = 0;
        self.dungeon.exhibit_text =
            Some(result.unwrap_or_else(|error| format!("LOCAL RESEARCH — {error}\nE/Esc closes")));
        self.redraw_requested = true;
    }

    pub(crate) fn dungeon_command(&mut self, argument: Option<&str>) -> String {
        let argument = argument.unwrap_or("").trim();
        let (verb, tail) = argument
            .split_once(char::is_whitespace)
            .unwrap_or((argument, ""));
        let tail = tail.trim();
        match verb {
            "stable" | "stables" => self.chivalry_command(crate::drive::chivalry::Place::Stables, tail),
            "tournament" | "knights" => self.chivalry_command(crate::drive::chivalry::Place::Tournament, tail),
            "deposit" => {
                if self.dungeon.joined.is_some() {
                    return "Local deposition is host-only; leave your friend's Delve first.".into();
                }
                // Normalize command-edge whitespace like other commands, then validate
                // the entire remaining name before settlement sync or file reads.
                let name = if tail.is_empty() { None } else { Some(tail) };
                let manifest =
                    match crate::drive::together_settlement::exhibits::manifest_path(name) {
                        Ok(path) => path,
                        Err(error) => return format!("Local deposition not committed: {error}. Use /dungeon deposit <name>, or /dungeon deposit for the legacy manifest."),
                    };
                let player = self.host_name();
                let tavern = self.realm().home.level(shooter::home::Station::Wing) >= 1;
                self.world.sync_settlement(&self.loop_ctl, &player, tavern);
                match self.world.deposit_settlement_exhibits(
                    &self.loop_ctl,
                    self.tools.current_workspace(),
                    name,
                ) {
                    Ok(n) => {
                        self.redraw_requested = true;
                        format!("Deposited {n} real local research exhibit(s). New Delves use this saved layout; existing runs stay unchanged. No rewards granted.")
                    },
                    Err(e) => format!("Local deposition not committed: {e}. Contract: {manifest} (angel.settlement-exhibits/v1); requires this site's saved loop receipt."),
                }
            }
            "inspect" if tail.is_empty() && self.dungeon.joined.is_none() => {
                if self.dungeon.shooter.is_none() { return "Start the settlement Delve, walk beside a LOCAL RESEARCH stand, then press E.".into(); }
                self.expand_dungeon();
                self.inspect_settlement_exhibit();
                "Local inspection open; research contents are never sent to guests.".into()
            }
            "forge" | "avatar" => self.dungeon_gear(verb, tail),
            "join" => self.join_delve(tail),
            "leave" | "off" if tail.is_empty() && self.dungeon.joined.is_some() => self.leave_delve(),
            "leave" if tail.is_empty() => self.leave_delve(),
            "" | "resume" | "play" if tail.is_empty() && self.dungeon.joined.is_some() => {
                self.expand_dungeon();
                "Back in your friend's delve. Esc returns to your composer.".into()
            }
            "cards" => self.dungeon_cards(tail),
            "bosses" => {
                let list = self.dungeon.shooter.as_ref().map_or_else(shooter::bosses::builtin, |r| r.bosses.clone())
                    .iter().map(|b| format!("{} ({}, by {})", b.name, b.only_in.name(), if b.by.is_empty() { "?" } else { &b.by })).collect::<Vec<_>>().join(", ");
                format!("{}\nGuardians now: {list}\nPut yours in .angel/dungeon/bosses/<id>.boss; it takes its delve's place from the next run.", shooter::bosses::rules())
            }
            "sound" | "music" => {
                let off = tail == "off";
                self.dungeon.audio.set_muted(off);
                if off { "Music and sound effects off (voices are /dungeon voices).".into() } else { "Music and sound effects on.".into() }
            }
            "voices" => {
                let chorus = &mut self.dungeon.chorus;
                if let Ok(level) = tail.trim_end_matches('%').parse::<u32>() {
                    chorus.volume = level.min(100) as f32 / 100.0;
                    chorus.muted = level == 0;
                    return format!("Voices at {}%.", level.min(100));
                }
                chorus.muted = match tail { "off" => true, "on" => false, _ => !chorus.muted };
                if chorus.muted { "Voices off. Subtitles stay.".into() } else { "Voices on: the Herald, Old Blaise, Wren, Tobbin, the Shoggoth and the guardians.".into() }
            }
            "wishes" | "realm" => self.dungeon_wishes(tail),
            "wish" if !tail.is_empty() => {
                let host = self.host_name();
                self.ask_wish(&host, tail)
            }
            "grant" if !tail.is_empty() => self.grant_wish(tail),
            "card" => self.dungeon_card(tail),
            "host" => self.host_from_command(tail),
            "invite" if tail.is_empty() => self.dungeon.guest.as_ref().map_or_else(
                || "No guest is invited. /dungeon host [guest-name] [private-address:port] opens the delve to friends.".into(),
                |server| {
                    let lines: Vec<String> = server
                        .invitation_urls()
                        .into_iter()
                        .enumerate()
                        .map(|(i, url)| format!("  friend {}:  /dungeon join {url}", i + 1))
                        .collect();
                    format!("Send each friend their own line:\n{}", lines.join("\n"))
                },
            ),
            "kick" if tail.is_empty() => self.kick_guest(),
            "settlement" if tail.is_empty() => {
                if self.dungeon.joined.is_some() || self.dungeon.shooter.is_some() {
                    return "A Delve is already open; its map is unchanged. /dungeon off first, then /dungeon settlement.".into();
                }
                let player = self.host_name();
                self.world.load_settlement(&player);
                let tavern = self.realm().home.level(shooter::home::Station::Wing) >= 1;
                self.world.sync_settlement(&self.loop_ctl, &player, tavern);
                match self.world.settlement_site() {
                    Err(error) => return format!("Settlement not opened: {error}"),
                    Ok(None) => return "Start a loop first to associate a settlement site.".into(),
                    Ok(Some(site)) if !site.associated() => return "Start a loop first to associate a settlement site.".into(),
                    Ok(Some(_)) => {}
                }
                self.start_shooter(None);
                if self.dungeon.shooter.as_ref().is_some_and(|run| run.settlement_site.is_some()) {
                    let site = self.world.settlement_site().ok().flatten().expect("admitted site");
                    format!("PLAYER HALL · loop settlement west; Winding Stair leads to the Delve.\nSite id: {} · {} LOCAL RESEARCH exhibits. Host /dungeon deposit <name> admits only .angelX/settlement-exhibits/<name>.json after a saved loop receipt; no-arg deposit keeps .angelX/settlement-exhibits.json compatibility. E beside a stand inspects its source locally.", site.id, site.exhibits.len())
                } else {
                    // A disk failure during the entry refresh is not success.
                    self.dungeon.notice.clone()
                }
            }
            "start" => {
                if self.dungeon.shooter.is_some() {
                    self.expand_dungeon();
                    return "Dungeon already open. R restarts a finished run; /dungeon off closes it.".into();
                }
                self.start_shooter((!tail.is_empty()).then_some(tail));
                "Dungeon delve started. WASD move; arrows aim and fire (F fires too); Space rolls (or raises a shield); E inspects LOCAL RESEARCH in the settlement (throws a bomb in combat). Esc returns to coding; F4/F6 resumes.".into()
            }
            "" if tail.is_empty() && self.dungeon.shooter.is_none() => self.call_delve(),
            "" if tail.is_empty() => {
                self.open_intro();
                "The Delve's menu: Enter continues the run under way.".into()
            }
            "" | "resume" | "play" if tail.is_empty() => {
                if self.dungeon.shooter.is_none() { self.start_shooter(None); } else { self.expand_dungeon(); }
                if self.dungeon_friends_seated() { "Dungeon expanded; Esc turns your knight to stone and returns to the composer.".into() } else { "Dungeon expanded; Esc pauses and returns to the composer.".into() }
            }
            "pause" | "back" if tail.is_empty() => {
                self.collapse_dungeon(); if self.dungeon_friends_seated() { "Your knight turns to stone; your friends play on. F6 rejoins.".into() } else { "Dungeon paused; F6 resumes.".into() }
            }
            "off" if tail.is_empty() => {
                if let Some(run) = self.dungeon.shooter.as_mut() { run.retreat(); }
                if !self.settle_realm() || !self.settle_home() {
                    return self.dungeon.notice.clone();
                }
                self.dungeon.audio.stop();
                // Restore the composer before dropping its saved draft with the run.
                self.collapse_dungeon();
                self.together = Default::default();
                let serial = self.dungeon.raid_serial;
                let hosted = self.dungeon.guest.is_some();
                let path = self.dungeon.save_path.clone();
                let realm = self.dungeon.realm.take();
                self.dungeon = DungeonView::default();
                self.dungeon.realm = realm;
                self.world.close_chivalry();
                self.world.delve_called = false;
                self.world.delve_lit = false;
                self.dungeon.raid_serial = serial;
                self.dungeon.save_path = path.clone();
                if let Some(path) = path { let _ = std::fs::remove_file(path); }
                if hosted { "Dungeon closed; guest invitation revoked.".into() } else { "Dungeon closed.".into() }
            }
            "status" if tail.is_empty() => self.dungeon.shooter.as_ref().map_or_else(
                || "No dungeon open. /dungeon starts a solo run.".into(),
                |run| format!("Dungeon · floor {}/{} {} · {:?} · tick {} · {} players · {} monsters · {} projectiles · score {}{}{}", run.floor(), shooter::FLOORS, run.dungeon.pack.name(), run.phase, run.tick, run.players.len(), run.enemies.len(), run.projectiles.len(), run.score, if self.dungeon.guest.is_some() { " · friends invited" } else { "" }, if self.dungeon_wants_fast_tick() { "" } else { " · paused" }),
            ),
            _ => DUNGEON_HELP.into(),
        }
    }

    /// Pauses discard controls and elapsed time; at most four fixed steps run
    /// per frame, even after a stalled terminal or a delayed model callback.
    pub(crate) fn advance_shooter_at(&mut self, now: Instant) {
        self.refresh_chivalry_projection_if_needed();
        if self.dungeon.guest.is_some() && let Some(run) = self.dungeon.shooter.as_mut() {
            run.chivalry = None;
        }
        if self.dungeon.chivalry_visit.is_some() {
            if self.dungeon.guest.is_some() || self.dungeon.joined.is_some() {
                self.dungeon.chivalry_visit = None;
            } else {
                self.clear_dungeon_controls();
                return;
            }
        }
        // Wall-clock polling also runs while solo play is paused for editing.
        if self.dungeon.joined.is_none()
            && self
                .dungeon
                .spell_scan
                .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_millis(500))
        {
            self.dungeon.spell_scan = Some(now);
            let dir = self.cards_dir();
            if let Some(run) = self.dungeon.shooter.as_mut()
                && run.hot_load_spells(&dir) > 0
            {
                self.redraw_requested = true;
            }
        }
        let keyboard = self.dungeon_keyboard_active();
        let hosted = self.dungeon_friends_seated();
        let wishing = self.dungeon.forge.forging.is_some();
        if let Some(run) = self.dungeon.shooter.as_mut() {
            run.wishing = wishing;
            let companion = run.players.get(&2).filter(|h| h.hp > 0).map(|h| (h.x, h.y));
            if let Some(host) = run.players.get_mut(&1) {
                let stone = hosted && !keyboard;
                let stone = stone || host.vigil;
                // F6 returns the host to the party, not an abandoned corner.
                if host.stone
                    && !stone
                    && let Some((x, y)) = companion
                {
                    host.x = x;
                    host.y = y;
                }
                host.stone = stone;
            }
        }
        let active = self.dungeon_wants_fast_tick();
        let mut wishes = Vec::new();
        let mut grants = Vec::new();
        let mut reforges = Vec::new();
        let mut hellos = Vec::new();
        let mut forged = Vec::new();
        let mut learns = Vec::new();
        if let Some(server) = &self.dungeon.guest {
            for (raid, seat, gear) in server.drain_gear() {
                let Some(run) = self.dungeon.shooter.as_mut().filter(|r| r.raid_id == raid) else {
                    continue;
                };
                if let guest::Gear::Hello(name) = gear {
                    hellos.push((seat, name));
                    continue;
                }
                let Some(hero) = run.players.get_mut(&seat) else {
                    continue;
                };
                match gear {
                    guest::Gear::Weapon(weapon) => {
                        hero.forged = Some(weapon);
                        hero.arm = None;
                    }
                    guest::Gear::Avatar(avatar) => hero.avatar = Some(avatar),
                    guest::Gear::Wish(words) => {
                        let name = hero.name.clone();
                        wishes.push((name, words));
                    }
                    guest::Gear::Grant(id) => grants.push(id),
                    guest::Gear::Reforge(part, words) => reforges.push((seat, part, words)),
                    guest::Gear::ReforgeCard(part, card) => forged.push((seat, part, card)),
                    guest::Gear::Card(card) => {
                        let name = card.name.clone();
                        if run.add_card(card, seat) {
                            self.dungeon.notice = format!("A new card enters the book: {name}");
                        }
                    }
                    guest::Gear::Hello(_) => {}
                    guest::Gear::Learn(words) => {
                        if run.can_wish(seat) {
                            learns.push((seat, words));
                        }
                    }
                    guest::Gear::Boon(id) => {
                        let granted = shooter::phrasebook::get(&id)
                            .filter(|_| run.can_wish(seat))
                            .map(|wish| run.grant(seat, &wish));
                        if let Some(Ok(name)) = granted {
                            self.dungeon.notice = format!("Knight {seat} is granted {name}");
                        }
                    }
                }
            }
            // Controls sent while paused, for an earlier delve, or that sat in
            // the queue past their lease never move the knight.
            for intent in server.drain_shooter_intents() {
                if active
                    && self
                        .dungeon
                        .shooter
                        .as_ref()
                        .is_some_and(|run| run.raid_id == intent.raid_id)
                    && now.saturating_duration_since(intent.received_at) < GUEST_LEASE
                {
                    self.dungeon
                        .remote
                        .insert(intent.player, (intent.input, intent.received_at));
                }
            }
        }
        for (name, words) in wishes {
            self.dungeon.notice = self.ask_wish(&name, &words);
        }
        for (seat, words) in learns {
            self.learn_wish(seat, words);
        }
        for (seat, name) in hellos {
            if let Some(run) = self.dungeon.shooter.as_mut() {
                let new = run.players.get(&seat).is_none_or(|h| h.knight.is_none());
                run.join_seat(seat, &name);
                if new {
                    // A friend takes the next knight of the company.
                    let company = crate::drive::together_shooter::knights::COMPANY;
                    let mine = run.players.get(&1).and_then(|h| h.knight.clone());
                    let first = company
                        .iter()
                        .position(|k| Some(k.id.to_string()) == mine)
                        .unwrap_or(0);
                    run.outfit(seat, &company[(first + seat as usize - 1) % company.len()]);
                    self.dungeon.notice = format!("{name} joins the party as knight {seat}");
                }
            }
        }
        for (seat, part, words) in reforges {
            if self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| run.can_reforge(seat))
            {
                self.request_reforge(seat, part, words);
            }
        }
        for (seat, part, card) in forged {
            // A friend's own angelX drafted it; this game checks it once more.
            let result = self
                .dungeon
                .shooter
                .as_mut()
                .map(|run| run.reforge(seat, part, card));
            self.dungeon.notice = match result {
                Some(Ok(name)) => format!("Knight {seat} reforged: {name}"),
                Some(Err(error)) => format!("Knight {seat}'s reforge didn't take: {error}"),
                None => String::new(),
            };
        }
        for id in grants {
            // A guest can raise a paid-for draft; drafting stays with the host.
            if self
                .realm()
                .wishes
                .iter()
                .any(|w| w.id == id && w.status == crate::drive::together_realm::Status::Drafted)
            {
                self.dungeon.notice = self.grant_wish(&id);
            }
        }
        if !active {
            self.clear_dungeon_controls();
        } else {
            let last = *self.dungeon.clock.get_or_insert(now);
            let due = (now.saturating_duration_since(last).as_nanos() / STEP.as_nanos())
                .min(u128::from(u32::MAX)) as u32;
            let steps = due.min(MAX_STEPS);
            self.dungeon.clock = Some(if due > MAX_STEPS {
                now
            } else {
                last + STEP * steps
            });
            let mut inputs = if keyboard {
                self.dungeon.local_inputs(now)
            } else {
                self.dungeon.held.clear();
                BTreeMap::new()
            };
            if let Some(server) = &self.dungeon.guest {
                // Each seat's held controls, until their lease lapses.
                for seat in server.seats() {
                    let held = self
                        .dungeon
                        .remote
                        .get(&seat)
                        .filter(|(_, received)| {
                            now.saturating_duration_since(*received) < GUEST_LEASE
                        })
                        .map_or_else(ShooterInput::default, |&(input, _)| input);
                    inputs.insert(seat, held);
                }
            }
            if let Some(run) = self.dungeon.shooter.as_mut() {
                for step in 0..steps {
                    if step + 1 == steps {
                        self.dungeon.before = run.pose();
                    }
                    run.step(&inputs);
                }
            }
        }
        self.watch_drafting();
        self.watch_learning();
        if let Some(run) = self.dungeon.shooter.as_mut() {
            for cue in std::mem::take(&mut run.cues) {
                self.dungeon.chorus.cue(&cue, now);
            }
        }
        if self.settle_realm() {
            self.settle_home();
        }
        if let Some(run) = self.dungeon.shooter.as_mut() {
            let sounds = std::mem::take(&mut run.sounds);
            if let Some(server) = &self.dungeon.guest {
                server.push_sounds(&sounds);
            }
            for sound in sounds {
                self.dungeon.audio.sfx(sound, now);
            }
        }
        if let Some(said) = self.dungeon.chorus.tick(now)
            && let Some(server) = &self.dungeon.guest
        {
            server.set_voice(&said);
        }
        self.publish_to_guest(active);
        if let Err(error) = self.dungeon.checkpoint(now, false) {
            self.dungeon.notice = format!("Could not save delve: {error}");
        }
    }

    pub(super) fn shooter_key(&mut self, key: event::KeyEvent) -> bool {
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            code => code,
        };
        // Releases clear controls even when ownership changed after key-down.
        if key.kind == event::KeyEventKind::Release {
            self.dungeon.key_releases = true;
            self.dungeon.held.remove(&code);
            return self.dungeon_view_active();
        }
        if self.dungeon_dialog_controls_blocked() {
            self.clear_dungeon_controls();
            return false;
        }
        if self.dungeon.forge.draft.is_some() && self.dungeon_view_active() {
            return self.reforge_key(key);
        }
        let command_modifier = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if matches!(code, KeyCode::F(4) | KeyCode::F(6)) && !command_modifier {
            // From the map, the way in is always the Delve's menu.
            if !self.dungeon.expanded {
                self.open_intro();
            }
            return true;
        }
        if !self.dungeon_view_active() {
            return false;
        }
        if self.dungeon.exhibit_text.is_some() {
            match code {
                KeyCode::Esc | KeyCode::Char('e') => {
                    self.dungeon.exhibit_text = None;
                }
                KeyCode::Up => {
                    self.dungeon.exhibit_scroll = self.dungeon.exhibit_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.dungeon.exhibit_scroll =
                        self.dungeon.exhibit_scroll.saturating_add(1).min(5600);
                }
                KeyCode::PageUp => {
                    self.dungeon.exhibit_scroll = self.dungeon.exhibit_scroll.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.dungeon.exhibit_scroll =
                        self.dungeon.exhibit_scroll.saturating_add(10).min(5600);
                }
                _ => {}
            }
            self.redraw_requested = true;
            return true;
        }
        if code == KeyCode::Esc && self.dungeon.cards_open {
            self.dungeon.cards_open = false;
            self.redraw_requested = true;
            return true;
        }
        if code == KeyCode::Esc && self.dungeon.chivalry_visit.is_some() {
            return self.chivalry_side_key(code);
        }
        if matches!(code, KeyCode::Esc | KeyCode::F(2)) {
            self.collapse_dungeon();
            return true;
        }
        if code == KeyCode::Char('/') && !command_modifier {
            self.collapse_dungeon();
            if self.input.is_empty() {
                self.input.push('/');
                self.cursor = 1;
            }
            return true;
        }
        if (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(code, KeyCode::Char('c' | 'l' | 'g')))
            || matches!(code, KeyCode::F(9) | KeyCode::F(10))
        {
            self.clear_dungeon_controls();
            return false;
        }
        if command_modifier || !self.dungeon.controls_visible || !self.terminal_focused {
            return true;
        }
        if self.dungeon.chivalry_visit.is_some() {
            if key.kind == event::KeyEventKind::Repeat
                && !matches!(code, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down)
            {
                return true;
            }
            return self.chivalry_side_key(code);
        }
        if code == KeyCode::Char('e') && !self.dungeon.cards_open && self.dungeon.joined.is_none()
            && let Some(place) = self.dungeon.shooter.as_ref()
                .and_then(|run| crate::drive::together_shooter::chivalry::near_portal(run, 1))
        {
            let message = self.chivalry_command(place, "enter");
            if self.dungeon.guest.is_none() { self.dungeon.chivalry_notice = message; }
            return true;
        }
        if code == KeyCode::Char('e')
            && !self.dungeon.cards_open
            && self.dungeon.joined.is_none()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| run.at_home_now() && run.settlement_site.is_some())
        {
            self.inspect_settlement_exhibit();
            return true;
        }
        if code == KeyCode::Char('x') {
            self.clear_dungeon_controls();
            return true;
        }
        if code == KeyCode::Char('t') {
            self.open_reforge();
            return true;
        }
        if code == KeyCode::Char('v') {
            let chorus = &mut self.dungeon.chorus;
            chorus.muted = !chorus.muted;
            self.dungeon.notice = if chorus.muted {
                "Voices off (v brings them back).".into()
            } else {
                "Voices on.".into()
            };
            return true;
        }
        if matches!(code, KeyCode::Tab | KeyCode::Char('c')) {
            self.dungeon.cards_open = !self.dungeon.cards_open;
            self.dungeon.held.clear();
            self.redraw_requested = true;
            return true;
        }
        if self.dungeon.cards_open {
            let pages = self
                .dungeon
                .shooter
                .as_ref()
                .map_or(1, crate::ui::viz::shooter_viz::card_pages);
            match code {
                KeyCode::Right | KeyCode::Char('d') => self.dungeon.cards_sel += 1,
                KeyCode::Left | KeyCode::Char('a') => {
                    self.dungeon.cards_sel = self.dungeon.cards_sel.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('s') | KeyCode::PageDown => {
                    self.dungeon.cards_page =
                        (self.dungeon.cards_page + 1).min(pages.saturating_sub(1));
                    self.dungeon.cards_sel = 0;
                }
                KeyCode::Enter
                    if self.dungeon.cards_page == crate::ui::viz::shooter_viz::REALM_PAGE =>
                {
                    let sel = self.dungeon.cards_sel;
                    let id = self
                        .realm()
                        .wishes
                        .get(sel)
                        .map(|w| (w.id.clone(), w.status));
                    match id {
                        Some((id, crate::drive::together_realm::Status::Drafted)) => {
                            self.dungeon.notice = self.grant_wish(&id);
                        }
                        Some((_, crate::drive::together_realm::Status::Asked)) => {
                            self.dungeon.notice = format!(
                                "/dungeon grant {} has angelX draft this wish first.",
                                self.dungeon.cards_sel + 1
                            );
                        }
                        _ => {}
                    }
                }
                KeyCode::Up | KeyCode::Char('w') | KeyCode::PageUp => {
                    self.dungeon.cards_page = self.dungeon.cards_page.saturating_sub(1);
                    self.dungeon.cards_sel = 0;
                }
                _ => {}
            }
            self.redraw_requested = true;
            return true;
        }
        if code == KeyCode::Char('r') {
            if self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(shooter::Run::active)
            {
                // While the delve runs, R is the ultimate's key.
                self.dungeon.hold(code, false);
                if let Some(hero) = self
                    .dungeon
                    .shooter
                    .as_ref()
                    .and_then(|r| r.players.get(&1))
                {
                    use crate::drive::together_shooter::ults::ULT_FULL;
                    if hero.ult_charge < ULT_FULL {
                        self.dungeon.notice = format!(
                            "{} is charging ({}%): fight to fill it.",
                            hero.ult().name(),
                            hero.ult_charge * 100 / ULT_FULL
                        );
                    }
                }
            } else {
                // A couch friend comes along by name; hosted friends by seat.
                let guest = self
                    .dungeon
                    .shooter
                    .as_ref()
                    .filter(|_| self.dungeon.guest.is_none())
                    .and_then(|run| run.players.get(&2))
                    .map(|hero| hero.name.clone());
                self.start_shooter(guest.as_deref());
            }
            return true;
        }
        if matches!(
            code,
            KeyCode::Char(
                'w' | 'a'
                    | 's'
                    | 'd'
                    | 'f'
                    | ' '
                    | 'e'
                    | 'q'
                    | 'g'
                    | 'p'
                    | 'i'
                    | 'j'
                    | 'k'
                    | 'l'
                    | 'u'
                    | 'o'
                    | '1'
                    | '2'
                    | '3'
                    | '4'
                    | '5'
                    | '6'
                    | '7'
                    | 'z'
                    | 'b'
                    | 'n'
                    | 'y'
            ) | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Enter
        ) {
            self.dungeon.hold(
                code,
                key.modifiers.contains(KeyModifiers::SHIFT)
                    || matches!(key.code, KeyCode::Char('W' | 'A' | 'S' | 'D')),
            );
        }
        true
    }
}
