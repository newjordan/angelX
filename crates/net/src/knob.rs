//! Knobs: the numbers a developer moves while the game runs, to feel a
//! change at once instead of rebuilding for it.
//!
//! A knob is a name, where it lives, its bounds and step, and the number in
//! the code it stands for. Its default is that number exactly, so nothing
//! changes until a slider moves; once a number feels right it is copied
//! back into the code as the new fixed number (the panel's "copy as
//! constants"), and the knob's default follows.
//!
//! - **Server knobs** change the game for everyone: a game's
//!   (`angel_serve::GameServer::knobs`) and the engine's own (how often a
//!   snapshot goes out). A client asks with `ToServer::Action` of kind
//!   `action::TUNE`; the server takes it only from the host (an in-process
//!   connection) unless the session runs in dev mode, applies it between
//!   ticks, and tells every client the new table (`ToClient::Knobs`). A
//!   number a client's prediction uses must reach it with the game's state,
//!   not through the table, so the client steps exactly as the server does.
//! - **Client knobs** change only this client's own view: the engine's
//!   (how far behind everyone else is drawn, how long a correction eases)
//!   and a game's (`client::GameClient::knobs`).

use crate::codec::{Bytes, Malformed, put_f64, put_str};

/// Engine action kinds, carried by `ToServer::Action` beside a game's own.
/// A game's kinds stay below `ENGINE`; the server takes these itself and
/// never hands them to the game.
pub mod action {
    pub const ENGINE: u32 = 0xFFFF_0000;
    /// Set a server knob: `tune_body(name, value)`.
    pub const TUNE: u32 = ENGINE | 1;
    /// Send this connection the server's numbers once a second
    /// (`ToClient::Stats`), or stop: one byte, 1 or 0.
    pub const STATS: u32 = ENGINE | 2;

    /// Whether a kind is the engine's, not the game's.
    pub fn engine(kind: u32) -> bool {
        kind & ENGINE == ENGINE
    }
}

/// Knob names: letters, digits and `_`, at most this long.
pub const MAX_NAME: usize = 32;

/// Where a knob acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// This client's own view.
    Client,
    /// The game, for everyone, set on the server.
    Server,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::Client => "client",
            Side::Server => "server",
        }
    }
}

/// One knob and where it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Knob {
    /// The name it is set by.
    pub name: &'static str,
    /// The panel's heading it sits under.
    pub group: &'static str,
    pub label: &'static str,
    /// The fixed number in the code it stands for, and where: what "copy
    /// as constants" names.
    pub constant: &'static str,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// The code's number today.
    pub default: f64,
    pub value: f64,
    pub side: Side,
}

impl Knob {
    /// A knob standing at its default.
    pub fn new(name: &'static str, group: &'static str, label: &'static str, constant: &'static str, side: Side) -> Knob {
        Knob { name, group, label, constant, min: 0.0, max: 1.0, step: 0.01, default: 0.0, value: 0.0, side }
    }

    /// Its bounds and step.
    pub fn range(mut self, min: f64, max: f64, step: f64) -> Knob {
        self.min = min;
        self.max = max;
        self.step = step;
        self
    }

    /// Its default, which it stands at.
    pub fn default(mut self, default: f64) -> Knob {
        self.default = default;
        self.value = default;
        self
    }

    /// Where it stands now.
    pub fn at(mut self, value: f64) -> Knob {
        self.value = value;
        self
    }

    /// `value` as this knob takes it: inside its bounds, on its step from
    /// the minimum. The default is always taken as it is, so a knob put
    /// back lands on the code's number exactly; `None` for a number that is
    /// not one.
    pub fn fit(&self, value: f64) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        if value == self.default {
            return Some(value);
        }
        let mut v = value.clamp(self.min, self.max);
        if self.step > 0.0 {
            v = self.min + ((v - self.min) / self.step).round() * self.step;
            // Six decimals: a step of 0.1 does not drift into 0.30000000000000004.
            v = (v * 1e6).round() / 1e6;
            v = v.clamp(self.min, self.max);
        }
        Some(v)
    }
}

/// The table a panel draws: whether this session runs in dev mode, whether
/// this client may set server knobs, and the knobs, as JSON.
pub fn table_json(dev: bool, may: bool, knobs: &[Knob]) -> String {
    let mut out = String::with_capacity(256 * knobs.len() + 32);
    out.push_str("{\"dev\":");
    out.push_str(if dev { "true" } else { "false" });
    out.push_str(",\"may\":");
    out.push_str(if may { "true" } else { "false" });
    out.push_str(",\"knobs\":");
    knobs_json(knobs, &mut out);
    out.push('}');
    out
}

/// The knobs as a JSON array.
pub fn knobs_json(knobs: &[Knob], out: &mut String) {
    out.push('[');
    for (i, k) in knobs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        json_str(k.name, out);
        out.push_str(",\"group\":");
        json_str(k.group, out);
        out.push_str(",\"label\":");
        json_str(k.label, out);
        out.push_str(",\"constant\":");
        json_str(k.constant, out);
        for (key, v) in [("min", k.min), ("max", k.max), ("step", k.step), ("default", k.default), ("value", k.value)] {
            out.push_str(",\"");
            out.push_str(key);
            out.push_str("\":");
            json_num(v, out);
        }
        out.push_str(",\"side\":");
        json_str(k.side.word(), out);
        out.push('}');
    }
    out.push(']');
}

/// A string as JSON.
pub fn json_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A number as JSON (anything not finite is `null`).
pub fn json_num(v: f64, out: &mut String) {
    if v.is_finite() {
        // Rust's shortest round-trip form; a whole number has no `.0`.
        if v == v.trunc() && v.abs() < 1e15 {
            out.push_str(&format!("{}", v as i64));
        } else {
            out.push_str(&format!("{v}"));
        }
    } else {
        out.push_str("null");
    }
}

/// Whether a name is one a knob may have.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A `TUNE` action's body.
pub fn tune_body(name: &str, value: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(name.len() + 10);
    put_str(&mut out, name);
    put_f64(&mut out, value);
    out
}

/// A `TUNE` action's name and value.
pub fn read_tune(body: &[u8]) -> Result<(&str, f64), Malformed> {
    let mut b = Bytes::new(body);
    let name = b.str(MAX_NAME)?;
    let value = b.f64()?;
    b.end()?;
    if !valid_name(name) {
        return Err(Malformed);
    }
    Ok((name, value))
}

/// The knobs as a text block to paste back into the code, one line each:
/// what the panel's "copy as constants" gives.
pub fn constants(knobs: &[Knob]) -> String {
    let mut out = String::new();
    for k in knobs {
        out.push_str(&format!("{} = {}  // {}{}\n", k.constant, k.value, k.name, if k.value == k.default { "" } else { " (moved)" }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pace() -> Knob {
        Knob::new("hero_speed", "knight", "Walk pace", "HERO_SPEED (delve rules.rs)", Side::Server)
            .range(4.0, 30.0, 0.5)
            .default(13.0)
    }

    #[test]
    fn a_value_is_held_to_its_bounds_and_step_and_the_default_lands_exactly() {
        let k = pace();
        assert_eq!(k.fit(100.0), Some(30.0));
        assert_eq!(k.fit(-3.0), Some(4.0));
        assert_eq!(k.fit(14.26), Some(14.5));
        assert_eq!(k.fit(f64::NAN), None);
        let odd = Knob::new("x", "g", "x", "X", Side::Client).range(0.0, 1.0, 0.25).default(0.3);
        assert_eq!(odd.fit(0.3), Some(0.3), "the default, off the step, still lands");
        let tenth = Knob::new("y", "g", "y", "Y", Side::Client).range(0.0, 1.0, 0.1).default(0.0);
        assert_eq!(tenth.fit(0.31), Some(0.3));
    }

    #[test]
    fn the_table_is_json_a_panel_reads() {
        let k = pace().at(15.5);
        let json = table_json(true, false, &[k]);
        assert_eq!(
            json,
            "{\"dev\":true,\"may\":false,\"knobs\":[{\"name\":\"hero_speed\",\"group\":\"knight\",\"label\":\"Walk pace\",\
             \"constant\":\"HERO_SPEED (delve rules.rs)\",\"min\":4,\"max\":30,\"step\":0.5,\"default\":13,\"value\":15.5,\"side\":\"server\"}]}"
        );
        let mut s = String::new();
        json_str("a \"b\"\n\\", &mut s);
        assert_eq!(s, "\"a \\\"b\\\"\\n\\\\\"");
    }

    #[test]
    fn a_tune_reads_back_and_a_bad_one_is_refused() {
        let body = tune_body("hero_speed", 15.5);
        assert_eq!(read_tune(&body), Ok(("hero_speed", 15.5)));
        assert_eq!(read_tune(&tune_body("no spaces", 1.0)), Err(Malformed));
        assert_eq!(read_tune(&tune_body(&"x".repeat(MAX_NAME + 1), 1.0)), Err(Malformed));
        let mut trailing = body.clone();
        trailing.push(0);
        assert_eq!(read_tune(&trailing), Err(Malformed));
        assert!(action::engine(action::TUNE) && action::engine(action::STATS) && !action::engine(7));
    }

    #[test]
    fn copied_constants_name_the_code_and_what_moved() {
        let text = constants(&[pace().at(15.5), pace()]);
        assert_eq!(text, "HERO_SPEED (delve rules.rs) = 15.5  // hero_speed (moved)\nHERO_SPEED (delve rules.rs) = 13  // hero_speed\n");
    }
}
