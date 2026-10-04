//! A local player's AI produces data; the game checks it before equipping it.
use super::*;
use crate::drive::{together_avatar as avatar, together_forge as forge};
use std::io::Read;

impl App {
    pub(super) fn dungeon_gear(&mut self, kind: &str, file: &str) -> String {
        if file.is_empty() {
            return if kind == "forge" {
                let runes = self
                    .dungeon
                    .shooter
                    .as_ref()
                    .and_then(|r| r.players.get(&1))
                    .and_then(|h| h.forged.as_ref())
                    .map_or("", |w| w.runes.as_str());
                format!(
                    "{}\nSave the answer inside this workspace, then /dungeon forge <file>.",
                    forge::rules(forge::START_LOC, runes)
                )
            } else {
                format!(
                    "{}\nSave the image inside this workspace, then /dungeon avatar <file>.",
                    avatar::instructions("")
                )
            };
        }
        let Some(hero) = self
            .dungeon
            .shooter
            .as_mut()
            .and_then(|r| r.players.get_mut(&1))
        else {
            return "Start a delve first with /dungeon.".into();
        };
        let root = match self.tools.current_workspace().canonicalize() {
            Ok(root) => root,
            Err(e) => return e.to_string(),
        };
        let path = match root.join(file).canonicalize() {
            Ok(path) if path.starts_with(&root) => path,
            _ => return "Choose a regular file inside this workspace.".into(),
        };
        let limit = if kind == "forge" {
            forge::MAX_BYTES
        } else {
            avatar::MAX_BYTES
        };
        let input = match std::fs::File::open(&path) {
            Ok(input) if input.metadata().is_ok_and(|m| m.is_file()) => input,
            _ => return "Could not open a regular gear file.".into(),
        };
        let mut bytes = Vec::new();
        if input
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > limit
        {
            return "Gear file is too large or unreadable.".into();
        }
        if kind == "forge" {
            let Ok(text) = std::str::from_utf8(&bytes) else {
                return "Runes must be UTF-8 text.".into();
            };
            match forge::check(text, forge::START_LOC) {
                Ok(checked) => {
                    let message = format!(
                        "Equipped {}. {}",
                        checked.weapon.name,
                        checked.notes.join("; ")
                    );
                    hero.forged = Some(checked.weapon);
                    hero.arm = None;
                    message
                }
                Err(error) => error.fix_it(),
            }
        } else {
            match avatar::check(&bytes) {
                Ok(checked) => {
                    hero.avatar = Some(checked.avatar);
                    format!("Avatar equipped. {}", checked.notes.join("; "))
                }
                Err(errors) => avatar::fix_it(&errors),
            }
        }
    }
}

const TEMPLATE: &str = "\
# A delve card. Edit me, save, then /dungeon cards reload (or /dungeon card <this file>).
# /dungeon cards prints every word a card may use.
name   {NAME}
kind   hold
rarity rare
by     {BY}
text   What it does, in a line of flavour.
drop   4
chest  3
damage 20
speed  10
art
....55....
...5665...
..566665..
.56666665.
..hHHHHh..
..hJJJJh..
..hJ66Jh..
..hJJJJh..
...hhhh...
";

impl App {
    /// `/dungeon cards [reload|new <name>]`: the card format, the workspace's
    /// cards read again, or a new card file to edit.
    pub(super) fn dungeon_cards(&mut self, tail: &str) -> String {
        let dir = self.cards_dir();
        let (verb, rest) = tail.split_once(char::is_whitespace).unwrap_or((tail, ""));
        match verb {
            "reload" => {
                if self.dungeon.shooter.is_none() {
                    return "Start a delve first with /dungeon.".into();
                }
                let report = self.reload_cards();
                if report.is_empty() {
                    format!(
                        "No cards in {}. /dungeon cards new <name> starts one.",
                        dir.display()
                    )
                } else {
                    format!("Cards from {}:\n{}", dir.display(), report.join("\n"))
                }
            }
            "new" => {
                let id = crate::drive::together_shooter::cards::slug(rest);
                if id.is_empty() {
                    return "/dungeon cards new <card name>".into();
                }
                let path = dir.join(format!("{id}.card"));
                if path.exists() {
                    return format!(
                        "{} already exists; edit it, then /dungeon cards reload.",
                        path.display()
                    );
                }
                let by = std::env::var("USER").unwrap_or_default();
                let text = TEMPLATE.replace("{NAME}", rest.trim()).replace("{BY}", &by);
                match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, text)) {
                    Ok(()) => format!(
                        "Wrote {}. Edit it, then /dungeon cards reload; Tab in the delve shows the book.",
                        path.display()
                    ),
                    Err(error) => format!("Could not write {}: {error}", path.display()),
                }
            }
            _ => {
                let book = self
                    .dungeon
                    .shooter
                    .as_ref()
                    .map_or_else(crate::drive::together_shooter::Book::builtin, |run| {
                        run.book.clone()
                    });
                let list = book
                    .cards
                    .iter()
                    .map(|c| format!("{} ({} {})", c.name, c.rarity.word(), c.kind.word()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{}\nYour cards: {}\nThe book ({}): {list}\n/dungeon cards new <name> · /dungeon cards reload · /dungeon card <file> · Tab in the delve opens the card screen.",
                    crate::drive::together_shooter::cards::rules(),
                    dir.display(),
                    book.cards.len()
                )
            }
        }
    }

    /// `/dungeon card <file>`: check one card file and drop it at your feet.
    pub(super) fn dungeon_card(&mut self, file: &str) -> String {
        if file.is_empty() {
            return "/dungeon card <file.card> checks a card and drops it at your feet.".into();
        }
        if self.dungeon.shooter.is_none() {
            return "Start a delve first with /dungeon.".into();
        }
        let root = match self.tools.current_workspace().canonicalize() {
            Ok(root) => root,
            Err(e) => return e.to_string(),
        };
        let path = match root.join(file).canonicalize() {
            Ok(path) if path.starts_with(&root) => path,
            _ => return "Choose a regular file inside this workspace.".into(),
        };
        let limit = crate::drive::together_shooter::cards::MAX_BYTES;
        let input = match std::fs::File::open(&path) {
            Ok(input) if input.metadata().is_ok_and(|m| m.is_file()) => input,
            _ => return "Could not open a regular card file.".into(),
        };
        let mut bytes = Vec::new();
        if input
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > limit
        {
            return "Card file is too large or unreadable.".into();
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return "A card must be UTF-8 text.".into();
        };
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        match crate::drive::together_shooter::cards::check(stem, text) {
            Ok(checked) => {
                let name = checked.card.name.clone();
                let Some(run) = self.dungeon.shooter.as_mut() else {
                    return String::new();
                };
                if !run.add_card(checked.card, 1) {
                    return "The book is full.".into();
                }
                self.expand_dungeon();
                let notes = if checked.notes.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", checked.notes.join("; "))
                };
                format!("{name} enters the book and lies at your feet{notes}.")
            }
            Err(rejected) => rejected.fix_it(),
        }
    }
}
