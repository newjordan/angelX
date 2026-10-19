//! Cached geometric placeholders for the art team's swappable Peasant slots.
use super::{Img, View};
use crate::rules::{Run, Projectile, peasant};
use std::sync::OnceLock;
struct Slots { views: [View; 3], stone: Img, decoy: Img, collapsed: Img, dust: Img }
fn slots() -> &'static Slots {
    static SLOTS: OnceLock<Slots> = OnceLock::new();
    SLOTS.get_or_init(|| {
        let s: serde_json::Value = serde_json::from_str(include_str!("../../../../../assets/peasant/slots.json")).expect("Peasant slots");
        let views = std::array::from_fn(|i| {
            let v = &s["knight"]["views"][i];
            View { body: super::bard::rows(&v["body"]), item: super::bard::layer(&v["item"]), strike: super::bard::layer(&v["strike"]) }
        });
        let img = |v: &serde_json::Value| Img::from_rows(super::bard::rows(v));
        Slots { views, stone: img(&s["stone"]["rows"]), decoy: img(&s["decoy"]["rows"]),
            collapsed: img(&s["decoy"]["collapsed_rows"]), dust: img(&s["dust"]["rows"]) }
    })
}
pub(super) fn views() -> &'static [View; 3] { &slots().views }
pub(crate) fn stone_shot(cv: &mut Img, p: &Projectile, x: i32, y: i32) -> bool {
    if !peasant::stone(p) { return false; }
    let lift = (peasant::stone_height(p.ttl) * super::super::UNIT) as i32;
    cv.line(x - 2, y, x + 2, y, 'O');
    let im = &slots().stone;
    cv.stamp(im, x - im.w / 2, y - im.h / 2 - lift);
    true
}
pub(crate) fn desertion(cv: &mut Img, run: &Run) {
    let s = slots();
    for d in &run.scarecrows {
        let (x,y) = super::super::at(d.x,d.y);
        let im = if d.active(run.tick) { &s.decoy } else { &s.collapsed };
        super::super::stand(cv,im,x,y);
    }
    for hero in run.players.values().filter(|h| h.hp > 0 && h.flee_ticks > 0) {
        let (x,y) = super::super::at(hero.x,hero.y);
        let shift = (run.tick % 4) as i32;
        cv.stamp(&s.dust,x - s.dust.w / 2 - shift,y - 2);
    }
}
