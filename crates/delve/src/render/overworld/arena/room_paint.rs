//! The room's paint list: every layer of the frame, in the order it goes
//! down, for one seat to draw (`DrawList`).

use super::*;

/// The room's paint list (`room_layer`).
pub(super) fn room_list(run: &Run, focus: Option<u32>, legible: bool) -> DrawList<'_> {
    // The seat's view of the room, for the marks that keep inside it.
    let view = view_origin(run, focus);
    let mut scene = DrawList::default();
    scene.add(Layer::Ground, move |cv| industry::deposits(cv, run, focus));
    scene.add(Layer::Ground, move |cv| hazards(cv, run));
    let fortune_hall = run.room().kind == RoomKind::Fortune;
    let in_yard = run.room().kind == RoomKind::Yard;
    let in_hall = run.room().kind == RoomKind::Trophies;
    let in_tavern = run.room().kind == RoomKind::Tavern;
    scene.add(Layer::Ground, move |cv| {
        if run.at_home_now() {
            if fortune_hall {
                fortune::figures(cv, run);
            } else if in_yard {
                yard::figures(cv, run);
                lessons::figures(cv, run);
            } else if crate::rules::squire_rooms::is_room(run.room().kind) {
                squire_rooms::figures(cv, run);
            } else if in_hall {
                trophies::figures(cv, run);
            } else if in_tavern {
                tavern::figures(cv, run);
            } else if run.room().kind == RoomKind::Home {
                home::figures(cv, run);
                rescues::residents(cv, run);
                hireling::at_the_stair(cv, run);
            }
            match run.room().kind {
                RoomKind::Stables => joust::stables(cv, run),
                RoomKind::Lists => joust::lists(cv, run, legible),
                RoomKind::Forest => forest::figures(cv, run),
                RoomKind::Eyrie => race3d::eyrie_figures(cv, run),
                RoomKind::LandOffice => holdings::office(cv, run),
                RoomKind::RealmHall => holdings::castle(cv, run),
                RoomKind::Alehouse => {
                    alehouse::figures(cv, run);
                    rite_wall::people(cv, run);
                }
                RoomKind::Gate => {
                    hall_voices::dragon(cv, run);
                    rite_wall::paint(cv, run);
                }
                RoomKind::MusterTent => muster_tent::figures(cv, run),
                RoomKind::Bazaar => bazaar::figures(cv, run),
                kind if crate::rules::world::bomrekedan::is_level(kind) => bomrekedan::figures(cv, run),
                _ => {}
            }
            countryside::figures(cv, run);
            colosseum::figures(cv, run);
            pastimes::figures(cv, run);
            cinder::sign(cv, run);
            latch::sign(cv, run);
            brasswake::sign(cv, run);
            rafterwake::sign(cv, run);
            pennon::sign(cv, run);
            mossbell::sign(cv, run);
            teachers::figures(cv, run);
            world::entrance_ring(cv, run);
        }
    });
    scene.add(Layer::Ground, move |cv| {
        if matches!(
            run.room().kind,
            RoomKind::MineHead | RoomKind::KingsHall | RoomKind::Gate | RoomKind::Forge
        ) {
            barony::plates(cv, run);
            reclaim::hall_plate(cv, run);
            reclaim::king_prompt(cv, run);
            barony::folk(cv, run);
        }
    });
    scene.add(Layer::Ground, move |cv| {
        if !run.at_home_now() {
            // A floor the realm holds: its banners, its steward's hall.
            holdings::start_banners(cv, run);
            if run.room().kind == RoomKind::Holding {
                holdings::hall(cv, run);
            }
            nooks::figures(cv, run);
            // A lost hall's keeper, and the plate where its rent is read.
            reclaim::plates(cv, run);
            reclaim::figures(cv, run);
            // The Deep: a village's folk and its star, the errands' signs.
            deep_village::figures(cv, run);
            deep_village::signs(cv, run);
            deep_village::meeting(cv, run);
        }
    });
    scene.add(Layer::Ground, move |cv| lane_squires::figures(cv, run, focus));
    scene.add(Layer::Ground, move |cv| college::figures(cv, run));
    scene.add(Layer::Ground, move |cv| ranch::figures_with(cv, run));
    scene.add(Layer::Ground, move |cv| works::under_with(cv, run));
    scene.add(Layer::Ground, move |cv| gauntlets::under(cv, run));
    scene.add(Layer::Ground, move |cv| merchant::figures(cv, run));
    scene.add(Layer::Ground, move |cv| ults::under(cv, run));
    scene.add(Layer::Ground, move |cv| minions::under(cv, run));
    scene.add(Layer::Ground, move |cv| foes_hazards(cv, run));
    scene.add(Layer::Ground, move |cv| boss_art::landings(cv, run));
    scene.add(Layer::Ground, move |cv| critters::world(cv, run));
    scene.add(Layer::Ground, move |cv| tide::ravages(cv, run));
    scene.add(Layer::Ground, move |cv| void::holes(cv, run));
    scene.add(Layer::Ground, move |cv| rescues::cage(cv, run));
    scene.add(Layer::Ground, move |cv| hand::ground_layer(cv, run));
    scene.add(Layer::Ground, move |cv| garden::flamingo_fallen(cv, run));
    scene.add(Layer::Ground, move |cv| gate::draw(cv, run));
    scene.add(Layer::Ground, move |cv| chronicle::under(cv, run));
    scene.add(Layer::Ground, move |cv| feel::under(cv, run));
    scene.add(Layer::Ground, move |cv| vfx::under_figures(cv, run));
    scene.add(Layer::Ground, move |cv| wishfx::under(cv, run));
    scene.add(Layer::Ground, move |cv| magic::under(cv, run));
    scene.add(Layer::Ground, move |cv| zones::under(cv, run));
    scene.add(Layer::Ground, move |cv| gore::under(cv, run));
    scene.add(Layer::Ground, move |cv| clay_pots(cv, run));
    scene.add(Layer::Ground, move |cv| cauldron::under(cv, run));
    scene.add(Layer::Ground, move |cv| hop_floor(cv, run));
    scene.add(Layer::Ground, move |cv| book_hop::floor(cv, run));
    scene.add(Layer::Ground, move |cv| wax_maze::floor(cv, run));
    scene.add(Layer::Ground, move |cv| belfry_shoot::floor(cv, run, view));
    scene.add(Layer::Ground, move |cv| powder_room::floor(cv, run, focus, view));
    scene.add(Layer::Ground, move |cv| signatures::under_with(cv, run));
    scene.add(Layer::Ground, move |cv| deep_village::dread_under(cv, run));
    scene.add(Layer::Ground, move |cv| altar::ground(cv, run));
    scene.add(Layer::Ground, move |cv| freedom::ground(cv, run));
    scene.add(Layer::Ground, move |cv| starfall::ground(cv, run));
    scene.add(Layer::Ground, move |cv| rewards::under(cv, run));
    scene.add(Layer::Ground, move |cv| factory::paint(cv, false));
    scene.add(Layer::Figures, move |cv| figures_with(cv, run, view, focus));
    scene.add(Layer::Ground, move |cv| { module_effects::draw(cv, run, crate::realm::effects::Layer::Ground); });
    scene.add(Layer::Over, move |cv| { module_effects::draw(cv, run, crate::realm::effects::Layer::Over); });
    scene.add(Layer::Over, move |cv| aerie::poop(cv, run));
    scene.add(Layer::Over, move |cv| {
        if legible {
            pitstop::board(cv, run);
            frogs::plates(cv, run);
        }
    });
    scene.add(Layer::Over, move |cv| wanted::nameplates(cv, run, view));
    scene.add(Layer::Over, move |cv| marks::take_prompts(cv, run, legible, view, focus));
    scene.add(Layer::Over, move |cv| marks::takes(cv, run, legible, view, focus));
    scene.add(Layer::Over, move |cv| marks::thief_cries(cv, run, legible, view));
    scene.add(Layer::Over, move |cv| tallow(cv, run));
    scene.add(Layer::Over, move |cv| hooks(cv, run));
    scene.add(Layer::Over, move |cv| sparks(cv, run));
    scene.add(Layer::Over, move |cv| orbits(cv, run));
    scene.add(Layer::Over, move |cv| summoned(cv, run));
    scene.add(Layer::Over, move |cv| flourishes(cv, run));
    scene.add(Layer::Over, move |cv| wishfx::over(cv, run));
    scene.add(Layer::Over, move |cv| magic::over(cv, run));
    scene.add(Layer::Over, move |cv| zones::over(cv, run));
    scene.add(Layer::Over, move |cv| chronicle::over(cv, run, legible));
    scene.add(Layer::Over, move |cv| feel::over(cv, run));
    scene.add(Layer::Over, move |cv| gore::over(cv, run));
    scene.add(Layer::Over, move |cv| works::over(cv, run));
    scene.add(Layer::Over, move |cv| gauntlets::over(cv, run));
    scene.add(Layer::Over, move |cv| signatures::over(cv, run));
    scene.add(Layer::Over, move |cv| garden::flamingo_over(cv, run));
    scene.add(Layer::Over, move |cv| wanted::over(cv, run, view));
    scene.add(Layer::Over, move |cv| ults::over(cv, run));
    scene.add(Layer::Over, move |cv| ailments::over(cv, run));
    scene.add(Layer::Over, move |cv| minions::over(cv, run));
    scene.add(Layer::Over, move |cv| wax_maze::wraiths(cv, run));
    scene.add(Layer::Over, move |cv| belfry_shoot::over(cv, run, focus, view));
    scene.add(Layer::Over, move |cv| book_hop::over(cv, run, focus));
    scene.add(Layer::Over, move |cv| powder_room::over(cv, run, focus, view));
    scene.add(Layer::Over, move |cv| cauldron::over(cv, run, legible, view));
    scene.add(Layer::Over, move |cv| altar::over(cv, run, legible, view, focus));
    scene.add(Layer::Over, move |cv| freedom::over(cv, run, legible, view, focus));
    scene.add(Layer::Over, move |cv| starfall::over(cv, run, legible, view, focus));
    scene.add(Layer::Over, move |cv| rewards::over(cv, run));
    scene.add(Layer::Over, move |cv| factory::paint(cv, true));
    scene.add(Layer::Over, move |cv| factory::attacks(cv, run));
    scene.add(Layer::Over, move |cv| garden::over(cv, run, legible));
    scene.add(Layer::Over, move |cv| squire_company::foot_tabs(cv, run));
    scene.add(Layer::Boards, move |cv| {
        if run.at_home_now() && legible {
            if fortune_hall {
                fortune::boards(cv, run, focus);
            } else if in_yard {
                yard::boards(cv, run, focus);
                squires::boards(cv, run, focus);
                lessons::boards(cv, run, focus);
            } else if crate::rules::squire_rooms::is_room(run.room().kind) {
                squire_rooms::boards(cv, run, focus);
            } else if in_hall {
                trophies::boards(cv, run);
                tavern::wing_ledger(cv, run);
            } else if in_tavern {
                tavern::boards(cv, run, focus);
            } else if run.room().kind == RoomKind::Home {
                home::boards(cv, run, focus);
            }
            if run.room().kind.in_world() || run.room().kind == RoomKind::Home {
                world::boards(cv, run);
            }
            match run.room().kind {
                RoomKind::Stables => joust::stables_boards(cv, run),
                RoomKind::Lists => joust::lists_boards(cv, run, focus),
                RoomKind::Forest => forest::boards(cv, run),
                RoomKind::LandOffice => holdings::office_boards(cv, run, focus),
                RoomKind::RealmHall => holdings::castle_boards(cv, run, focus),
                // Alehouse/bazaar prompts dock after the seat crop.
                _ => {}
            }
            colosseum::boards(cv, run);
            if reading(run) {
                countryside::boards(cv, run);
            }
        }
    });
    scene.add(Layer::Boards, move |cv| cauldron::boards(cv, run, focus, legible, view));
    scene.add(Layer::Boards, move |cv| {
        if legible {
            merchant::boards(cv, run, focus);
            if run.room().kind == RoomKind::Holding {
                holdings::hall_boards(cv, run, focus);
                holdings_folk::boards(cv, run, focus);
            }
            if !run.at_home_now() {
                nooks::boards(cv, run);
                reclaim::boards(cv, run);
            }
        }
    });
    scene.add(Layer::Boards, move |cv| {
        if legible
            && matches!(
                run.room().kind,
                RoomKind::MineHead | RoomKind::KingsHall | RoomKind::Forge
            )
        {
            barony::boards(cv, run);
            reclaim::hall_boards(cv, run);
        }
    });
    scene
}
