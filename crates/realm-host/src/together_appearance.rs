//! Prepare a character appearance resource off the UI/tick path. The generic
//! module kernel owns registration, state and resource leases; this adapter
//! translates the character contract to the current Delve presentation model.
use super::together_frames::Frame;
use angel_delve::realm::{Avatar, appearance::Animation};
use angel_live::Asset;
use image::{ImageFormat, RgbaImage};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;

pub const CONTRACT: &str = "character-appearance/1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Specification {
    player: u32,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    frames: Vec<Frame>,
    #[serde(default = "default_ticks")]
    frame_ticks: u32,
    #[serde(default = "default_colour")]
    colour: [u8; 3],
}
fn default_ticks() -> u32 {
    6
}
fn default_colour() -> [u8; 3] {
    [160, 120, 240]
}

#[derive(Clone, Debug)]
pub struct Appearance {
    pub player: u32,
    pub fingerprint: String,
    pub avatar: Option<Avatar>,
    // A consumer can retain a complete prepared generation, including the
    // source bytes, while a new module generation becomes active.
    _assets: Vec<Arc<Asset>>,
}
impl Appearance {
    pub fn with_clip(
        mut self,
        steps: &[angel_delve::realm::appearance::Step],
    ) -> Result<Self, String> {
        if self.avatar.is_none() {
            return Err("animation requires enabled character art".into());
        }
        if let Some(avatar) = &mut self.avatar {
            let animation = avatar
                .animation
                .as_mut()
                .ok_or("animation requires prepared art frames")?;
            *animation = animation.with_steps(steps)?;
            self.fingerprint = angel_live::host::sha256_hex(
                &serde_json::to_vec(&(&self.fingerprint, steps))
                    .map_err(|error| error.to_string())?,
            );
            avatar.source_id = self.fingerprint.clone();
        }
        Ok(self)
    }
    pub fn matches(&self, avatar: Option<&Avatar>) -> bool {
        match (&self.avatar, avatar) {
            (Some(_), Some(current)) => current.source_id == self.fingerprint,
            (None, None) => true,
            _ => false,
        }
    }
    pub fn install(&self, run: &mut angel_delve::rules::Run) -> bool {
        let Some(hero) = run.players.get_mut(&self.player) else {
            return false;
        };
        if self.matches(hero.avatar.as_ref()) {
            return false;
        }
        let mut avatar = self.avatar.clone();
        if let Some(next) = avatar.as_mut().and_then(|avatar| avatar.animation.as_mut())
            && let Some(old) = hero
                .avatar
                .as_ref()
                .and_then(|avatar| avatar.animation.as_ref())
        {
            next.continue_from(old, run.tick);
        }
        hero.avatar = avatar;
        true
    }
}

pub fn bind(
    value: &Value,
    assets: &BTreeMap<String, Arc<Asset>>,
) -> Result<angel_live::kernel::Binding, String> {
    let spec: Specification = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if !(1..=angel_delve::rules::MAX_PLAYERS).contains(&spec.player)
        || spec.frame_ticks == 0
        || spec.frame_ticks > 300
        || spec.frames.len() > 8
        || (!spec.disabled && spec.frames.is_empty())
        || (spec.disabled && !spec.frames.is_empty())
    {
        return Err(format!(
            "character appearance: player 1..{}, 1..8 frames, and frame_ticks 1..300 are required",
            angel_delve::rules::MAX_PLAYERS
        ));
    }
    let fingerprint = angel_live::host::sha256_hex(value.to_string().as_bytes());
    let (frames, held) = super::together_frames::prepare(spec.frames, assets)?;
    let avatar = if spec.disabled {
        None
    } else {
        let pixels = frames[0].clone();
        let image = RgbaImage::from_raw(
            32,
            32,
            pixels
                .iter()
                .flat_map(|pixel| pixel.iter().copied())
                .collect(),
        )
        .ok_or("invalid character pixel buffer")?;
        let encode = |image: &RgbaImage| -> Result<Vec<u8>, String> {
            let mut bytes = Vec::new();
            image
                .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            Ok(bytes)
        };
        let portrait =
            image::imageops::resize(&image, 128, 128, image::imageops::FilterType::Nearest);
        Some(Avatar {
            source_id: fingerprint.clone(),
            animation: Some(Animation {
                frames,
                frame_ticks: spec.frame_ticks,
                phase_offset: 0,
            }),
            sprite: encode(&image)?,
            portrait: encode(&portrait)?,
            colour: spec.colour,
            pixels,
        })
    };
    Ok(Arc::new(Appearance {
        player: spec.player,
        fingerprint,
        avatar,
        _assets: held,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn all_game_seats_accept_appearance_without_resetting_player_state() {
        let mut run = angel_delve::rules::Run::new(3, 9, None);
        for player in 2..=angel_delve::rules::MAX_PLAYERS {
            run.join_seat(player, &format!("Player {player}"));
        }
        for player in [5, 6] {
            let hero = run.players.get_mut(&player).unwrap();
            hero.hp = 7;
            hero.x = 12.0;
            hero.spell_cooldowns[0] = 19;
            let value = json!({"player":player,"frames":[{"pixels":vec![[10,200,10,255];1024]}]});
            let appearance = bind(&value, &BTreeMap::new()).unwrap();
            assert!(
                appearance
                    .downcast_ref::<Appearance>()
                    .unwrap()
                    .install(&mut run)
            );
            let hero = &run.players[&player];
            assert_eq!((hero.hp, hero.x, hero.spell_cooldowns[0]), (7, 12.0, 19));
            assert_eq!(
                hero.avatar.as_ref().unwrap().pixels_at(run.tick)[0],
                [10, 200, 10, 255]
            );
        }
        let restored = angel_delve::rules::runtime_state::restore(
            angel_delve::rules::runtime_state::capture(&run).unwrap(),
        )
        .unwrap();
        for player in [5, 6] {
            assert_eq!(
                restored.players[&player]
                    .avatar
                    .as_ref()
                    .unwrap()
                    .pixels_at(run.tick)[0],
                [10, 200, 10, 255]
            );
        }
        for player in [0, angel_delve::rules::MAX_PLAYERS + 1] {
            let value = json!({"player":player,"frames":[{"pixels":vec![[10,200,10,255];1024]}]});
            assert!(bind(&value, &BTreeMap::new()).is_err());
        }
    }
    #[test]
    fn prepared_character_swap_changes_frames_without_resetting_gameplay_and_preserves_phase() {
        let mut run = angel_delve::rules::Run::new(3, 9, None);
        run.tick = 27;
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = 7;
        hero.x = 12.0;
        hero.spell_cooldowns[0] = 19;
        let value = |colour, ticks| json!({"player":1,"frame_ticks":ticks,"frames":[{"pixels":vec![colour;1024]},{"pixels":vec![colour;1024]}]});
        let old = bind(&value([200, 10, 10, 255], 5), &BTreeMap::new()).unwrap();
        old.downcast_ref::<Appearance>().unwrap().install(&mut run);
        let before = run.players[&1]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .phase(run.tick);
        let next = bind(&value([10, 200, 10, 255], 10), &BTreeMap::new()).unwrap();
        assert!(next.downcast_ref::<Appearance>().unwrap().install(&mut run));
        let hero = &run.players[&1];
        assert_eq!((hero.hp, hero.x, hero.spell_cooldowns[0]), (7, 12.0, 19));
        let avatar = hero.avatar.as_ref().unwrap();
        assert_eq!(avatar.pixels_at(run.tick)[0], [10, 200, 10, 255]);
        assert_eq!(
            avatar.animation.as_ref().unwrap().phase(run.tick),
            before * 2
        );
        let restored = angel_delve::rules::runtime_state::restore(
            angel_delve::rules::runtime_state::capture(&run).unwrap(),
        )
        .unwrap();
        assert_eq!(
            restored.players[&1]
                .avatar
                .as_ref()
                .unwrap()
                .pixels_at(run.tick)[0],
            [10, 200, 10, 255]
        );
    }
    #[test]
    fn malformed_or_missing_frames_never_publish_a_prepared_appearance() {
        assert!(
            bind(
                &json!({"player":1,"frames":[{"asset":"absent"}]}),
                &BTreeMap::new()
            )
            .is_err()
        );
        assert!(
            bind(
                &json!({"player":1,"frames":[{"pixels":[[1,2,3,255]]}]}),
                &BTreeMap::new()
            )
            .is_err()
        );
    }
}
