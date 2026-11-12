use super::*;
use angel_live::game::Cartridge;
use angel_plug::app::AppWorld;
use angel_live::realm::RealmHost;

pub(crate) struct Slot {
    pub(crate) carts: Vec<Box<dyn Cartridge>>,
    pub(crate) active: Option<usize>,
    pub(crate) shown: bool,
}

impl Default for Slot {
    fn default() -> Self {
        Slot {
            carts: registry(),
            active: None,
            shown: false,
        }
    }
}

pub(crate) fn registry() -> Vec<Box<dyn Cartridge>> {
    vec![Box::new(angel_games::Engine::<
        angel_muster::MusterServer,
        angel_muster::MusterClient,
    >::new(angel_games::Spec {
        id: "muster",
        title: "The Muster",
        help: "arrows/WASD walk · space waves · e plants or lifts your banner · Esc composer",
        make: || angel_muster::MusterServer::new(angel_muster::rules::SHEEP),
        keys: angel_games::keys::standard,
    }))]
}

impl Slot {
    pub(crate) fn find(&self, id: &str) -> Option<usize> {
        self.carts.iter().position(|cart| cart.id() == id)
    }

    pub(crate) fn on_screen(&self) -> Option<usize> {
        self.active
            .filter(|&index| self.shown && self.carts.get(index).is_some_and(|cart| cart.running()))
    }
}

fn quiet(rest: &str) -> bool {
    matches!(
        rest.split_whitespace().next(),
        Some("stop" | "leave" | "close" | "link" | "status")
    )
}

impl App {
    pub(crate) fn cart_on_screen(&self) -> Option<usize> {
        self.games.on_screen()
    }

    pub(crate) fn game_command(&mut self, arg: Option<&str>) -> String {
        let arg = arg.unwrap_or("").trim();
        let (id, rest) = arg
            .split_once(char::is_whitespace)
            .map(|(id, rest)| (id, rest.trim()))
            .unwrap_or((arg, ""));
        let realm_game = self.realm.game_entry();
        match id {
            "" | "list" => self.game_list(),
            _ if realm_game.is_some_and(|game| game.id == id) => {
                self.hide_cart();
                self.dungeon_command((!rest.is_empty()).then_some(rest))
            }
            _ => {
                let Some(index) = self.games.find(id) else {
                    return format!("No cartridge named `{id}`.\n{}", self.game_list());
                };
                let reply = self
                    .with_cart(index, |cart, cx| cart.command(cx, rest))
                    .unwrap_or_default();
                if self.games.carts[index].running() {
                    if !quiet(rest) {
                        self.show_cart(index);
                    }
                } else if self.games.active == Some(index) {
                    self.games.active = None;
                    self.games.shown = false;
                    self.redraw_requested = true;
                }
                reply
            }
        }
    }

    fn game_list(&self) -> String {
        let mut lines = vec!["Cartridges in the slot:".to_string()];
        if let Some(game) = self.realm.game_entry() {
            lines.push(format!("  {} · {}", game.id, game.summary));
        }
        for cart in &self.games.carts {
            let state = if cart.running() { "running" } else { "ready" };
            lines.push(format!("  {} · {} · {state}", cart.id(), cart.title()));
        }
        lines.push("/game <name> loads one and gives it the screen · /game <name> host|join <link>|stop · Esc gives the screen back".into());
        lines.join("\n")
    }

    pub(crate) fn show_cart(&mut self, index: usize) {
        if RealmHost::view_active(&self.realm, &self.realm_facts()) {
            self.with_realm(|realm, cx| RealmHost::collapse(realm, cx));
        }
        self.games.active = Some(index);
        self.games.shown = true;
        self.redraw_requested = true;
    }

    pub(crate) fn hide_cart(&mut self) {
        if let Some(index) = self.games.on_screen() {
            self.with_cart(index, |cart, cx| cart.clear_controls(cx));
        }
        self.games.shown = false;
        self.redraw_requested = true;
    }

    pub(crate) fn cart_key(&mut self, index: usize, key: event::KeyEvent) -> bool {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return false;
        }
        if key.kind != event::KeyEventKind::Release
            && matches!(key.code, KeyCode::Esc | KeyCode::F(2))
        {
            self.hide_cart();
            return true;
        }
        self.with_cart(index, |cart, cx| cart.key(cx, key));
        true
    }

    pub(crate) fn tick_carts(&mut self) {
        let shown = self.games.on_screen();
        for index in 0..self.games.carts.len() {
            if self.games.carts[index].running() {
                self.with_cart(index, |cart, cx| cart.tick(cx, shown == Some(index)));
            }
        }
    }

    pub(crate) fn note_carts(&mut self, mut each: impl FnMut(&mut dyn Cartridge)) {
        for cart in &mut self.games.carts {
            each(cart.as_mut());
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/game_slot__tests.rs"]
mod tests;
