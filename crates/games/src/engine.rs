use crate::door;
use angel_live::game::{Cartridge, Picture};
use angel_live::realm::HostCx;
use angel_net::client::GameClient;
use angel_serve::client::{LinkOptions, LinkStatus, NativeLink};
use angel_serve::{ClientBundle, GameServer, ServeConfig, Server};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const PRESS_LEASE: Duration = Duration::from_millis(500);
const REPEAT_LEASE: Duration = Duration::from_millis(180);
const RELEASED_LEASE: Duration = Duration::from_secs(30);

pub struct Spec<S> {
    pub id: &'static str,
    pub title: &'static str,
    pub help: &'static str,
    pub make: fn() -> S,
    pub keys: fn(KeyCode) -> u32,
}

pub struct Engine<S: GameServer, C: GameClient> {
    spec: Spec<S>,
    live: Option<Live<C>>,
    epoch: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Door {
    Solo,
    Hosting,
    Joined,
}

struct Live<C: GameClient> {
    server: Option<Server>,
    link: NativeLink<C>,
    door: Door,
    links: Vec<String>,
    held: HashMap<u32, Instant>,
    releases: bool,
    notice: String,
    paged: bool,
}

impl<S: GameServer, C: GameClient<Cmd: Send> + Send + 'static> Engine<S, C> {
    pub fn new(spec: Spec<S>) -> Self {
        Engine {
            spec,
            live: None,
            epoch: Instant::now(),
        }
    }

    fn ms(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64() * 1000.0
    }

    fn serve(&self, bind: SocketAddr, door: Door) -> Result<Live<C>, String> {
        let title = self.spec.title;
        let mut config = ServeConfig::new(bind);
        let page = if door == Door::Hosting {
            page(self.spec.id)
        } else {
            None
        };
        let paged = page.is_some();
        config.bundle = page;
        let server = Server::start((self.spec.make)(), config)
            .map_err(|e| format!("{title} could not open: {e}"))?;
        let conn = server
            .connect_local()
            .map_err(|e| format!("{title} has no seat for the host: {e}"))?;
        let link = NativeLink::local(conn, "host", LinkOptions::default());
        let links = if door == Door::Hosting {
            server.links()
        } else {
            Vec::new()
        };
        let mut live = Live::new(Some(server), link, door, links);
        live.paged = paged;
        Ok(live)
    }

    fn close(&mut self) {
        if let Some(mut live) = self.live.take() {
            live.link.bye();
            if let Some(server) = live.server.take() {
                server.stop();
            }
        }
    }

    fn host(&mut self) -> String {
        let bind = match door::bind(&door::default_bind(0)) {
            Ok(bind) => bind,
            Err(e) => return format!("{} cannot host: {e}", self.spec.title),
        };
        self.close();
        match self.serve(bind, Door::Hosting) {
            Ok(live) => {
                self.live = Some(live);
                self.status()
            }
            Err(e) => e,
        }
    }

    fn join(&mut self, link: &str) -> String {
        if link.is_empty() {
            return format!("/game {} join <link>", self.spec.id);
        }
        match NativeLink::<C>::connect(link, "guest", LinkOptions::default()) {
            Ok(native) => {
                self.close();
                self.live = Some(Live::new(
                    None,
                    native,
                    Door::Joined,
                    vec![link.to_string()],
                ));
                self.status()
            }
            Err(e) => format!("{} cannot join: {e}", self.spec.title),
        }
    }
}

impl<C: GameClient> Live<C> {
    fn new(server: Option<Server>, link: NativeLink<C>, door: Door, links: Vec<String>) -> Self {
        Live {
            server,
            link,
            door,
            links,
            held: HashMap::new(),
            releases: false,
            notice: String::new(),
            paged: false,
        }
    }
}

pub fn pages() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ANGEL_GAME_PAGES").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".angelX").join("pages"))
}

fn page(id: &str) -> Option<ClientBundle> {
    ClientBundle::from_dir(&pages()?.join(id)).ok()
}

fn plain(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        code => code,
    }
}

impl<S: GameServer, C: GameClient<Cmd: Send> + Send + 'static> Cartridge for Engine<S, C> {
    fn id(&self) -> &'static str {
        self.spec.id
    }

    fn title(&self) -> &'static str {
        self.spec.title
    }

    fn help(&self) -> &'static str {
        self.spec.help
    }

    fn running(&self) -> bool {
        self.live.is_some()
    }

    fn start(&mut self, _cx: &mut dyn HostCx) -> Result<String, String> {
        if self.live.is_none() {
            self.live = Some(self.serve(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), Door::Solo)?);
        }
        Ok(self.status())
    }

    fn stop(&mut self, _cx: &mut dyn HostCx) -> String {
        let was = self.live.is_some();
        self.close();
        if was {
            format!("{} is closed.", self.spec.title)
        } else {
            format!("{} is not running.", self.spec.title)
        }
    }

    fn command(&mut self, cx: &mut dyn HostCx, arg: &str) -> String {
        let mut words = arg.split_whitespace();
        match words.next().unwrap_or("") {
            "" | "start" | "play" => self.start(cx).unwrap_or_else(|e| e),
            "host" => self.host(),
            "join" => self.join(words.next().unwrap_or("")),
            "stop" | "leave" | "close" => self.stop(cx),
            "link" | "status" => self.status(),
            other => format!(
                "{} has no `{other}`: start · host · join <link> · link · stop",
                self.spec.title
            ),
        }
    }

    fn key(&mut self, _cx: &mut dyn HostCx, key: KeyEvent) -> bool {
        let bit = (self.spec.keys)(plain(key.code));
        let Some(live) = self.live.as_mut() else {
            return false;
        };
        if bit == 0 {
            return true;
        }
        let now = Instant::now();
        match key.kind {
            KeyEventKind::Release => {
                live.releases = true;
                if live.held.remove(&bit).is_some() {
                    live.link.hands().up(bit);
                }
            }
            kind => {
                let lease = if live.releases {
                    RELEASED_LEASE
                } else if kind == KeyEventKind::Repeat {
                    REPEAT_LEASE
                } else {
                    PRESS_LEASE
                };
                if live.held.insert(bit, now + lease).is_none() {
                    live.link.hands().down(bit);
                }
            }
        }
        true
    }

    fn clear_controls(&mut self, _cx: &mut dyn HostCx) {
        let ms = self.ms();
        if let Some(live) = self.live.as_mut() {
            live.held.clear();
            live.link.release_all(ms);
        }
    }

    fn tick(&mut self, _cx: &mut dyn HostCx, shown: bool) {
        let ms = self.ms();
        let Some(live) = self.live.as_mut() else {
            return;
        };
        let now = Instant::now();
        let expired: Vec<u32> = live
            .held
            .iter()
            .filter(|(_, until)| **until <= now)
            .map(|(bit, _)| *bit)
            .collect();
        for bit in expired {
            live.held.remove(&bit);
            live.link.hands().up(bit);
        }
        if shown {
            live.link.tick(ms);
        } else {
            if !live.held.is_empty() {
                live.held.clear();
                live.link.release_all(ms);
            }
            live.link.idle(ms);
        }
        for line in live.link.take_lines() {
            if let Some(text) = line
                .strip_prefix("say ")
                .or_else(|| line.strip_prefix("notice "))
            {
                live.notice = text.to_string();
            }
        }
    }

    fn picture(&mut self, _now: Instant) -> Option<Picture<'_>> {
        let ms = self.ms();
        let live = self.live.as_mut()?;
        let frame = live.link.draw(ms);
        (frame.w > 0 && frame.h > 0).then_some(Picture {
            w: frame.w,
            h: frame.h,
            px: &frame.px,
        })
    }

    fn status(&self) -> String {
        let title = self.spec.title;
        let Some(live) = self.live.as_ref() else {
            return format!("{title} · not running · /game {} to start", self.spec.id);
        };
        let state = match live.link.status() {
            LinkStatus::Connecting => "connecting".to_string(),
            LinkStatus::Playing => "playing".to_string(),
            LinkStatus::Reconnecting => "reconnecting".to_string(),
            LinkStatus::Rejected(_, why) => format!("turned away: {why}"),
            other => format!("{other:?}").to_lowercase(),
        };
        let door = match live.door {
            Door::Solo => "solo",
            Door::Hosting => "hosting",
            Door::Joined => "joined",
        };
        let runtime = live.link.runtime();
        let mut out = format!("{title} · {door} · {state}");
        if let Some(seat) = runtime.seat() {
            out.push_str(&format!(" · seat {}", seat + 1));
        }
        if let Some(rtt) = runtime.rtt_ms() {
            out.push_str(&format!(" · {rtt:.0} ms"));
        }
        if live.door == Door::Hosting {
            if let Some(server) = live.server.as_ref() {
                out.push_str(&format!(" · {} in", server.players().len()));
            }
        }
        if let Some(link) = live.links.first() {
            out.push_str(&format!(" · {link}"));
        }
        if live.door == Door::Hosting && !live.paged {
            out.push_str(" · angelX seats only: no browser page for it");
        }
        if !live.notice.is_empty() {
            out.push_str(&format!(" · {}", live.notice));
        }
        out
    }
}

impl<S: GameServer, C: GameClient> Drop for Engine<S, C> {
    fn drop(&mut self) {
        if let Some(mut live) = self.live.take() {
            live.link.bye();
            if let Some(server) = live.server.take() {
                server.stop();
            }
        }
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
