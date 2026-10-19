//! Four-seat entrance and existing door-state fixtures, using the cockpit painter.
#[allow(dead_code)]
mod painter { include!(concat!(env!("OUT_DIR"), "/painter.rs")); }
use angel_delve::{render::overworld::arena, rules::{Run, Input, Pack, Phase, EnemyKind}, settings::{self, Settings}};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use std::{collections::BTreeMap, path::Path};
fn capture(dir: &Path, name: &str, run: &Run, modern: bool) {
    settings::set(Settings {art_v2:modern,feel_ledger:false});
    let state=serde_json::to_vec(run).unwrap();
    std::fs::write(dir.join(format!("{name}-run.json")),&state).unwrap();
    let (w,h)=if modern {(768,448)} else {(384,224)};
    let im=arena::frame_for(run,w,h,Some(1));
    image::save_buffer(dir.join(format!("{name}.png")),&im.rgb_bytes(),w as u32,h as u32,image::ColorType::Rgb8).unwrap();
    let mut term=Terminal::new(TestBackend::new(96,28)).unwrap();
    term.draw(|f| {
        if modern && let Some((base,fx))=arena::art_v2::terminal_layers(run,Some(1)) {
            painter::paint_cells(f,Rect::new(0,0,96,28),&base,&fx);return;
        }
        let im=arena::frame_for(run,96,56,Some(1));
        for y in 0..28u16 {for x in 0..96u16 {
            let [r,g,b]=im.get(x as i32,y as i32*2).unwrap_or([0;3]);
            let [r1,g1,b1]=im.get(x as i32,y as i32*2+1).unwrap_or([0;3]);
            f.buffer_mut()[(x,y)].set_char('▀').set_fg(Color::Rgb(r,g,b)).set_bg(Color::Rgb(r1,g1,b1));
        }}
    }).unwrap();
    let rgb=|c|if let Color::Rgb(r,g,b)=c {[r,g,b]}else{[0;3]};
    let cells=term.backend().buffer().content.iter().map(|c|serde_json::json!({"glyph":c.symbol(),"fg":rgb(c.fg),"bg":rgb(c.bg)})).collect::<Vec<_>>();
    std::fs::write(dir.join(format!("{name}-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
    assert_eq!(state,serde_json::to_vec(run).unwrap(),"paint changed simulation");
}
fn main() {
    let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());std::fs::create_dir_all(&dir).unwrap();
    for (name,fixture) in [("cinder",Run::cinder_for_test as fn()->Run),("latch",Run::latch_for_test),("mossbell",Run::mossbell_for_test),("brasswake",Run::brasswake_for_test),("rafterwake",Run::rafterwake_for_test),("pennon",Run::pennon_for_test)] {
        let mut run=fixture();for id in 2..=4 {run.join_seat(id,&format!("Seat {id}"));}
        assert_eq!(run.players.len(),4);
        for modern in [false,true] {capture(&dir,&format!("{name}-v{}",if modern{2}else{1}),&run,modern);}
        // Every delivered entrance is also exercised by a real entry/exit walk.
        let snapshot=serde_json::to_vec(&run).unwrap();
        for _ in 0..120 {run.step(&BTreeMap::from([(1,Input{move_y:1,..Default::default()})]));}
        std::fs::write(dir.join(format!("{name}-walk.json")),serde_json::to_vec(&run).unwrap()).unwrap();
        std::fs::write(dir.join(format!("{name}-entry.json")),snapshot).unwrap();
    }
    for (name,pack) in [("crypt",Pack::Crypt),("mines",Pack::Cavern),("keep",Pack::Hellforge),("fungal",Pack::Fungal),("unknown",Pack::Unknown),("archive",Pack::Archive)] {
        let mut run=Run::new(17,1,None);run.dungeon=angel_delve::rules::crawl_floor(2,pack,17);
        let at=run.dungeon.rooms.iter().position(|r|r.kind==angel_delve::rules::RoomKind::Fight).unwrap();
        run.enter_for_test(at);run.calm_for_test();run.enemies.clear();run.projectiles.clear();run.waves=Default::default();run.phase=Phase::Fighting;
        for side in [1,3] {run.dungeon.rooms[at].open_door(side);}
        for id in 2..=4 {run.join_seat(id,&format!("Seat {id}"));}
        for (&id,h) in &mut run.players {h.x=19.0+id as f32*2.0;h.y=18.0;}
        for state in ["open","closed","locked"] {
            run.enemies.clear();run.waves=Default::default();
            if state=="closed" {run.spawn_at_for_test(EnemyKind::Imp,24.0,6.0);}
            if state=="locked" {run.waves.left=1;}
            for modern in [false,true] {capture(&dir,&format!("{name}-{state}-v{}",if modern{2}else{1}),&run,modern);}
        }
    }
    settings::set(Default::default());
}
