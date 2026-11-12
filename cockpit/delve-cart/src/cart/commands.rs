use super::Delve;
use crate::world_viz;
use angel_live::realm::{HostCx, RealmHost, Visit};
use angel_plug::commands::{Building, CommandsRules, CommandsWorld, Herald, Step};

pub(crate) fn plug_building(building: world_viz::Building) -> Building {
    building.into()
}

pub(crate) fn world_building(building: Building) -> world_viz::Building {
    building.into()
}

impl CommandsWorld for Delve {
    fn herald(name: &str, args: &str) -> Option<Herald> {
        let told = world_viz::herald(name, args);
        Some(Herald::new(
            told.text(false),
            told.text(true),
            told.lead_len(false),
            told.lead_len(true),
        ))
    }

    fn map_enabled() -> bool {
        world_viz::overworld::map_enabled()
    }

    fn together_context(&self) -> String {
        self.together.context_block()
    }

    fn turn_opened(&mut self) {
        self.world.turn_started();
    }

    fn workshop_opened(&mut self) {
        self.world.note_workshop();
    }

    fn lesson_opened(&mut self, topic: Option<&str>) -> Option<Building> {
        self.world
            .begin_teaching_lesson(topic.unwrap_or("open shelves"));
        Some(plug_building(world_viz::Building::Scriptorium))
    }
}

impl CommandsRules for Delve {
    fn world_command(
        &mut self,
        cx: &mut dyn HostCx,
        arg: Option<&str>,
        saddled: bool,
        saddled_on_quest: bool,
    ) -> Vec<Step> {
        match arg {
            Some(a) if a == "crops" || a.starts_with("crops ") => {
                let selected = a
                    .strip_prefix("crops ")
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let report = self.world.graph_report(selected);
                self.world.visit_overworld("garden");
                vec![
                    Step::ShowWorld,
                    Step::FocusStage,
                    Step::Redraw("graph garden visit"),
                    Step::Say(report),
                ]
            }
            Some(a) if a.eq_ignore_ascii_case("follow") => {
                self.world.follow_overworld();
                vec![
                    Step::ReturnToWorld,
                    Step::Redraw("world follow resumed"),
                    Step::Say("Realm camera follows the working knight.".to_string()),
                ]
            }
            Some(a)
                if a.split_whitespace()
                    .next()
                    .is_some_and(|word| word.eq_ignore_ascii_case("visit")) =>
            {
                let name = a
                    .split_once(char::is_whitespace)
                    .map_or("", |(_, name)| name.trim());
                // The stables and the lists are rooms of the world:
                // walked in the Delve, not looked at from the map. (The
                // map's tournament screen is still a camera visit.)
                if let Some(place) = crate::chivalry::Place::parse(name).filter(|_| {
                    !matches!(name.to_ascii_lowercase().as_str(), "tournament" | "knights")
                }) {
                    let place = match place {
                        crate::chivalry::Place::Stables => Visit::Stables,
                        crate::chivalry::Place::Tournament => Visit::Lists,
                    };
                    return vec![Step::Say(RealmHost::visit(self, cx, place))];
                }
                if let Some(label) = self.world.visit_overworld(name) {
                    let mut steps = Vec::new();
                    if self.dungeon.expanded {
                        steps.push(Step::Collapse);
                    }
                    steps.extend([
                        Step::ShowWorld,
                        Step::FocusStage,
                        Step::Redraw("world landmark visit"),
                        Step::Say(format!(
                            "Viewing {label} · /world follow returns to the working knight."
                        )),
                    ]);
                    steps
                } else {
                    vec![Step::Say("Visit a realm landmark: /world visit stables|lists|tournament|artisans|colosseum|village|round-table|keep, or a model house's castle (/world visit kimi). /world follow resumes the live camera.".to_string())]
                }
            }
            Some(a) if a.eq_ignore_ascii_case("zoom") => {
                let mode = self.world.cycle_camera_zoom();
                vec![
                    Step::ResetYaw,
                    Step::OpenRealm,
                    Step::Say(format!("world camera: {mode}")),
                ]
            }
            Some(a) if a.eq_ignore_ascii_case("ride") => {
                let in_the_saddle = if self.world.quest_owns_pane() {
                    saddled_on_quest
                } else {
                    saddled
                };
                if in_the_saddle {
                    vec![Step::Say("Already in the saddle.".to_string())]
                } else {
                    vec![
                        Step::ResetYaw,
                        Step::Saddle(plug_building(self.world.destination())),
                        Step::FocusStage,
                        Step::Say("Saddle up — the road fills the glass.".to_string()),
                    ]
                }
            }
            Some(a) if a.eq_ignore_ascii_case("enter") => {
                if self.world.riding() {
                    vec![Step::Say("No door opens mid-journey.".to_string())]
                } else if !self.world.has_authored_interior() {
                    vec![Step::Say("No door waits at this landmark.".to_string())]
                } else if self.world.enter_interior() {
                    vec![
                        Step::ResetYaw,
                        Step::Say(format!(
                            "The {} door opens.",
                            self.world.destination().label()
                        )),
                    ]
                } else {
                    vec![Step::Say(
                        "No door opens until the landmark settles.".to_string(),
                    )]
                }
            }
            Some(a) if a.eq_ignore_ascii_case("leave") => {
                if self.world.leave_interior() {
                    vec![
                        Step::ResetYaw,
                        Step::Say("The door shuts behind you.".to_string()),
                    ]
                } else {
                    vec![Step::Say("Already under the open sky.".to_string())]
                }
            }
            Some(a) if a.eq_ignore_ascii_case("weather") => {
                vec![Step::Say(self.world.weather_report())]
            }
            Some(a) if a.eq_ignore_ascii_case("3d") => {
                world_viz::world3d::toggle();
                vec![Step::Say(world_viz::world3d::status_line())]
            }
            Some(a)
                if a.split_whitespace()
                    .next()
                    .is_some_and(|word| word.eq_ignore_ascii_case("view")) =>
            {
                // Old selectors remain compatible but cannot replace Dotmax
                // outdoors or implicitly enter a room.
                let wanted = a.split_whitespace().nth(1);
                let mut steps = Vec::new();
                let view = match wanted {
                    Some(word) => match world_viz::world3d::WorldView::parse(word) {
                        Some(view) => Some(view),
                        None => {
                            steps.push(Step::Say(format!(
                                "unknown world view {word:?} — /world view 3d|dotmax (room plates require Enter)"
                            )));
                            None
                        }
                    },
                    None => Some(world_viz::world3d::cycle()),
                };
                if let Some(view) = view {
                    world_viz::world3d::set(view);
                    steps.push(Step::Say(world_viz::world3d::status_line()));
                }
                steps
            }
            Some(a) if a.eq_ignore_ascii_case("quest") => {
                // The debugging window on the adventure model: every field
                // the retained 3D renderer reads, as plain text.
                vec![Step::Say(self.world.quest_report())]
            }
            Some(a) if a.eq_ignore_ascii_case("help") => {
                vec![Step::Help(world_help_text())]
            }
            Some(a) => {
                // An unknown verb is a question, not a fault: answer it
                // with the whole map of what /world can do.
                vec![Step::Say(format!(
                    "unknown /world verb {a:?}\n{}",
                    world_help_text()
                ))]
            }
            None => vec![
                Step::ResetYaw,
                Step::ReturnToWorld,
                Step::FocusStage,
                Step::Say(format!(
                    "Realm open — welcome to {}",
                    self.world.town_name()
                )),
            ],
        }
    }

    fn together_command(&mut self, cx: &mut dyn HostCx, arg: Option<&str>) -> Vec<Step> {
        let raw = arg.unwrap_or("").trim();
        if raw.split_whitespace().next() == Some("build") {
            if !self.together.can_build() {
                return vec![Step::Say(
                    "Together building requires a player at the forge; use /together on or return"
                        .to_string(),
                )];
            }
            return match crate::together::build_prompt(
                raw.strip_prefix("build").unwrap_or("").trim(),
            ) {
                Ok(prompt) => vec![Step::Ask(prompt)],
                Err(error) => vec![Step::Say(error)],
            };
        }
        let host = &mut self.host;
        match host.together.command(
            arg,
            host.world.realm_seed(),
            host.world.town_name(),
            &cx.workspace(),
        ) {
            Ok(result) => {
                if !self.together.enabled() && self.dungeon.shooter.is_none() {
                    self.dungeon.reset_retaining_wish_jobs();
                }
                let mut steps = Vec::new();
                if self.together.enabled() {
                    steps.extend([Step::ShowWorld, Step::FocusStage]);
                }
                steps.extend([
                    Step::Redraw("Together mode changed"),
                    Step::Say(result.message),
                ]);
                steps
            }
            Err(error) => vec![Step::Say(format!("Together: {error}"))],
        }
    }

    fn village_command(&mut self) -> Vec<Step> {
        // The fleet read off the town — forge/heads/granary/apprentice,
        // every line a wired data source (the correspondence legend).
        vec![Step::Say(self.world.village_report())]
    }
}

/// `/world help` — every verb the Realm command answers to, in one message.
///
/// Deterministic text; the subcommand list is the contract the completion
/// table and `local_command::help_text` are checked against.
pub(crate) fn world_help_text() -> String {
    [
        "/world · open the Realm stage · verbs:",
        "  view [3d|dotmax] · the overworld map on the Realm, Dotmax 3D on Explore; older names also select Dotmax (/world 3d and v report the view)",
        "  zoom · cycle the map camera auto→wide→close",
        "  visit <place> · view garden, artisans, colosseum, tournament, village, round-table, or another landmark",
        "  follow · return the camera to the working knight",
        "  walk · step onto the map yourself: arrows or WASD walk, E or Enter goes in at a door, Esc leaves (or click the map, or p on it)",
        "  ride · saddle up; the road fills the glass",
        "  enter · step through the landmark's door",
        "  leave · step back under the open sky",
        "  crops [plot-id] · visit the graph garden and inspect exact chart data",
        "  weather · the realm's weather report",
        "  quest · the live adventure: region, danger, loot, party, waypoint",
        "  on|off · the visual backdrop (off = lean text mode)",
    ]
    .join("\n")
}
