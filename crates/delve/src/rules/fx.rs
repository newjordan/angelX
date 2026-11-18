//! What a tick shows rather than decides: the screen's shake and the chain
//! sparks. The step only emits them as events (`Run::fx`); `Run::fade` (after
//! a ride, as the tick starts) and `Run::present` (as it ends) turn them into
//! what the picture draws, ease and age what is already shown, and keep the
//! caps. Nothing in the rules reads them back.

use super::*;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Fx {
    Shake(u32),
    Spark(Spark),
}

const MAX_SPARKS: usize = 32;

pub fn ground_pace(
    hero: &Hero,
    webs: &[hunters::Web],
    song: Option<&str>,
    roads: Option<&map::Geography>,
    tune: &super::tune::Tune,
) -> f32 {
    let mut pace = tune.hero_speed * (100 + hero.bonus.speed) as f32 / 100.0 * hero.pit.pace() * (100 + hero.blessing()) as f32 / 100.0;
    if hero.peasant() { pace *= peasant::PACE; }
    pace *= hero.curio_pace();
    if hero.block.is_some() {
        pace *= stockyard::BLOCK_PACE;
    }
    if hero.immune == 0 && Run::webbed(webs, hero.x, hero.y) {
        pace *= hunters::WEB_PACE;
    }
    if hero.has_rune(runes::RuneKind::Haste) {
        pace *= runes::HASTE;
    }
    if hero.frog() {
        pace *= hexer::FROG_PACE;
    }
    if hero.chilled > 0 {
        pace *= lich::CHILL_PACE;
    }
    if hero.mired > 0 {
        pace *= horrors::GRIP_PACE;
    }
    if roads.is_some_and(|geo| kingdom::on_road(geo, hero.x, hero.y)) {
        pace *= tune.road_pace;
    }
    if hero.singing && song == Some("haste") {
        pace *= tavern::SONG_HASTE;
    }
    pace
}

pub fn sparks_room(sparks: &[Spark], fx: &[Fx]) -> bool {
    sparks.len() + fx.iter().filter(|f| matches!(f, Fx::Spark(_))).count() < MAX_SPARKS
}

impl Run {
    pub fn ground_pace_of(&self, id: u32) -> Option<f32> {
        let hero = self.players.get(&id)?;
        let outside = self.exits.outside.contains_key(&id);
        let at = self.exits.outside.get(&id).map_or(self.at, |e| e.at);
        let roads = (self.dungeon.rooms[at].kind == RoomKind::Kingdom)
            .then(|| map::geography(&self.kingdom.wishes));
        let (webs, song) = if outside {
            (&[][..], None)
        } else {
            (&self.webs[..], self.song.as_deref())
        };
        Some(ground_pace(hero, webs, song, roads.as_deref(), &self.tune))
    }

    pub(super) fn fade(&mut self) {
        self.show_fx();
        self.shake = self.shake.saturating_sub(1);
    }

    pub(super) fn present(&mut self) {
        self.show_fx();
        for spark in &mut self.sparks {
            spark.ttl = spark.ttl.saturating_sub(1);
        }
        self.sparks.retain(|s| s.ttl > 0);
        self.age_gore();
    }

    pub(super) fn show_fx(&mut self) {
        for fx in std::mem::take(&mut self.fx) {
            match fx {
                Fx::Shake(ticks) => self.shake = self.shake.max(ticks),
                Fx::Spark(spark) => {
                    if self.sparks.len() < MAX_SPARKS {
                        self.sparks.push(spark);
                    }
                }
            }
        }
    }
}
