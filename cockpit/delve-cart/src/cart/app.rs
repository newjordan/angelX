use super::Delve;
use crate::houses;
use crate::world_viz::{self, World};
use angel_live::kernel::Resource;
use angel_live::{GamePolicy, RealmStyle};
use angel_plug::app::{AppWorld, RealmGame};
use angel_plug::screen::Console;
use angel_realm_host::host::GameHost;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

/// Read the repository's visible top-level directory names for the district
/// layer. Any filesystem error disables the layer rather than delaying startup.
pub fn scan_workspace_districts(root: &std::path::Path) -> Vec<String> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => return Vec::new(),
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => return Vec::new(),
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.')
            || matches!(
                name.as_str(),
                "target" | "node_modules" | "vendor" | "dist" | "build"
            )
        {
            continue;
        }
        names.push(name);
    }
    names.sort();
    names.truncate(12);
    names
}

impl AppWorld for Delve {
    fn raise(session_disabled: bool, workspace: &Path) -> Self {
        // Decode the miniviz location/rider sources away from both startup and
        // the first draw. The viewer deliberately leaves frame one on the
        // braille fallback, so this worker has a full fast-tick interval to
        // finish before illustrated world playback begins.
        world_viz::warm_cinematic_assets();
        let mut world = if session_disabled {
            World::new(7)
        } else {
            World::for_workspace(workspace)
        };
        world.enable_districts(scan_workspace_districts(workspace));
        Delve::new(GameHost::new(world))
    }

    fn install_services(
        ambient_stage_sim_allowed: fn() -> bool,
        processes: Box<dyn angel_plug::services::Processes>,
        console: &dyn Console,
    ) {
        crate::realm_services::install(processes);
        // And the cockpit's answers the realm host asks for (posture, stage,
        // repository identity, the village's HTTP).
        crate::realm_hooks::install(ambient_stage_sim_allowed, console);
    }

    fn retirement() -> impl Sized {
        crate::together_media::RetirementGuard
    }

    fn read_settings() {
        crate::delve_settings_from_env();
    }

    fn thread_sim() {
        crate::together_sim::set_threaded(
            std::env::var("ANGEL_DELVE_SIM_THREAD").map_or(true, |v| v != "0"),
        );
    }

    #[cfg(feature = "glass")]
    fn draw_in_window() {
        // Thrown things in the Delve fly in the window, lifted toward the camera,
        // instead of in the game's picture.
        crate::world_viz::overworld::arena::draw_lifted_outside(true);
        // The window draws the Delve's frames from their parts (a stage); only a
        // terminal is handed the finished picture.
        crate::world_viz::overworld::arena::draw_parts_outside(true);
    }

    fn restore_run(&mut self, workspace: &Path) {
        if let Err(error) = self.dungeon.restore(workspace) {
            self.dungeon.notice = format!("Could not restore delve: {error}");
            tracing::warn!(%error, "could not restore dungeon");
        }
    }

    fn completion_ceremony(&mut self, active: bool) {
        self.world.set_completion_ceremony_active(active);
    }

    fn terminal_resized(&mut self) {
        self.dungeon.controls_visible = false;
    }

    fn map_moving(&self) -> bool {
        self.world.in_motion()
    }

    fn scenery_animating(&self) -> bool {
        self.world.animating()
    }

    fn joined_to_host(&self) -> bool {
        self.dungeon.joined.is_some()
    }

    fn friends_seated(&self) -> bool {
        self.view().friends_seated()
    }

    fn game_entry(&self) -> Option<RealmGame> {
        Some(RealmGame {
            id: "delve",
            summary: "the realm's own Delve · /dungeon",
        })
    }

    fn house_family(&self, house: u8) -> Option<&'static str> {
        Some(houses::get(houses::HouseId(house)).family)
    }

    fn live_status(&self) -> Option<String> {
        Some(self.dungeon.shooter.system_status())
    }

    fn live_style(&mut self, style: &RealmStyle) {
        self.world.set_live_realm_style(style.clone());
    }

    fn live_rules(&mut self, rules: Option<&Arc<GamePolicy>>) {
        self.dungeon
            .shooter
            .set_live_rules(rules.cloned().unwrap_or_default());
    }

    fn live_characters(&mut self, resources: &BTreeMap<String, Resource>) {
        self.dungeon.shooter.set_live_characters(resources.values());
    }
}
