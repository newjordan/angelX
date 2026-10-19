//! Offline review using native simulation frames and the actual cockpit cell painter.
#[cfg(test)]
#[path = "../../cockpit/src/stage/world_viz/art/cells.rs"]
mod cells;

#[cfg(test)]
mod production;

#[cfg(test)]
mod contrast;

#[cfg(test)]
mod snapshots;

#[cfg(test)]
mod bard;

#[cfg(test)]
mod peasant;

#[cfg(test)]
mod tests {
    use angel_delve::render::overworld::arena::{self, art_v2::{self, testing}};
    use angel_delve::settings::{self, Settings};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    use std::path::Path;

    #[test]
    #[ignore = "exports the existing rage-ring stages over the library body"]
    fn write_warning_probe() {
        let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_WARNING_SHOTS").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let registry:serde_json::Value=serde_json::from_str(include_str!("../boss-library-registry.json")).unwrap();
        for id in registry.as_object().unwrap().keys() {
        let art=angel_delve::rules::bosses::Art::for_id(id).unwrap();
        for age in 0..5usize {
            let mut run=testing::boss(art.id());
            run.works.clouds.clear();
            run.tick=96+age as u64*4;
            let e=&mut run.enemies[0];
            e.x=24.0;e.y=15.0;e.hp=e.max_hp*3/10;e.seen_hp=e.hp;e.dread=[1,10,19,28,36][age];
            for (seat,hero) in &mut run.players {hero.x=if *seat==1 {24.0}else if *seat==2 {6.0}else {42.0};hero.y=24.0;}
            let mut shot:angel_delve::rules::Projectile=serde_json::from_value(serde_json::json!({"x":8.0,"y":18.0,"vx":4.0,"vy":0.0,"hostile":true,"kind":"ember","damage":1,"pierce":0,"last_hit":null,"ttl":100})).unwrap();
            shot.look=art.look();run.projectiles=vec![shot];
            run.feel.impacts=vec![angel_delve::rules::feel::Impact{x:38.0,y:18.0,kind:angel_delve::rules::Shot::Ember,ttl:[6,4,3,2,1][age],boss_art:Some(art)}];
            for modern in [false,true] {
                let name=format!("{id}-effects-{age}-v{}",if modern {2}else{1});
                capture(&dir,&name,&run,modern);
                let mut background=run.clone();background.projectiles.clear();background.feel.impacts.clear();background.ravages.clear();background.enemies[0].dread=0;
                capture(&dir,&format!("{name}-background"),&background,modern);
            }
        }
        }
        settings::set(Default::default());
    }

    pub(super) fn capture(dir: &Path, name: &str, run: &angel_delve::rules::Run, modern: bool) {
        settings::set(Settings { art_v2:modern,feel_ledger:false });
        let (w,h)=if modern {(768,448)} else {(384,224)};
        let img=arena::frame_for(run,w,h,Some(1));
        image::save_buffer(dir.join(format!("{name}-rgb.png")),&img.rgb_bytes(),w as u32,h as u32,image::ColorType::Rgb8).unwrap();
        let mut terminal=Terminal::new(TestBackend::new(96,28)).unwrap();
        terminal.draw(|f| {
            if modern {
                if let Some((base,fx))=art_v2::terminal_layers(run,Some(1)) {
                    super::cells::paint_cells(f,Rect::new(0,0,96,28),&base,&fx);
                    return;
                }
            }
            // The cockpit's V1 paint_room route: frame_for(96,56), then paint_pixels.
            let im=arena::frame_for(run,96,56,Some(1));
            for y in 0..28u16 { for x in 0..96u16 {
                let [r,g,b]=im.get(x as i32,y as i32*2).unwrap_or([0;3]);
                let [r1,g1,b1]=im.get(x as i32,y as i32*2+1).unwrap_or([0;3]);
                f.buffer_mut()[(x,y)].set_char('▀').set_fg(Color::Rgb(r,g,b)).set_bg(Color::Rgb(r1,g1,b1));
            }}
        }).unwrap();
        let buf=terminal.backend().buffer();
        let colour=|c:Color| match c {Color::Rgb(r,g,b)=>[r,g,b],_=>[0;3]};
        let cells:Vec<_>=(0..28).flat_map(|y|(0..96).map(move|x| {
            let c=&buf[(x,y)];serde_json::json!({"glyph":c.symbol(),"fg":colour(c.fg),"bg":colour(c.bg)})
        })).collect();
        std::fs::write(dir.join(format!("{name}-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "exports display-size and kit-coverage evidence"]
    fn write_display_study() {
        let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_LIBRARY_STUDY").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let registry:serde_json::Value=serde_json::from_str(include_str!("../boss-library-registry.json")).unwrap();
        let mut records=Vec::new();
        let ids = std::env::var("ANGEL_LIBRARY_BOSSES").unwrap_or_else(|_| registry.as_object().unwrap().keys().cloned().collect::<Vec<_>>().join(","));
        let samples: u32 = std::env::var("ANGEL_LIBRARY_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        for sample in 0..=samples {
            #[cfg(feature = "library-trials")]
            arena::boss_library_review(sample);
            for id in ids.split(',') {
                for ticks in [0,133] {
                    let mut run=testing::boss(id);
                    for seat in 2..=4 {run.join_seat(seat,&format!("Knight {seat}"));}
                    for _ in 0..ticks {
                        for h in run.players.values_mut(){h.hp=h.max_hp;}
                        run.step(&Default::default());
                    }
                    let state=serde_json::to_value(&run).unwrap();
                    for modern in [false,true] {
                        let kit=if modern {"v2"}else{"v1"};
                        let index=(sample as usize).min(registry[id]["study"][kit].as_array().map_or(0, |v| v.len()));
                        let dims=if index==0{&registry[id][kit]}else{&registry[id]["study"][kit][index-1]};
                        let name=format!("{id}-sample-{sample}-{ticks}-{kit}");
                        capture(&dir,&name,&run,modern);
                        assert_eq!(state,serde_json::to_value(&run).unwrap(),"rendering changed simulation");
                        records.push(serde_json::json!({"name":name,"boss":id,"kit":kit,"sample":sample,"size":dims,"tick":run.tick,"enemy":run.enemies[0],"shots":run.projectiles.len(),"players":run.players.len()}));
                    }
                }
            }
        }
        #[cfg(feature = "library-trials")]
        arena::boss_library_review(0);
        for (name,mut run) in [("kingdom",testing::kingdom()),("crypt",testing::fight()),("hall",testing::hall()),("book-hop",testing::book_hop())] {
            for seat in 2..=4 {if !run.players.contains_key(&seat){run.join_seat(seat,&format!("Knight {seat}"));}}
            for modern in [false,true] {capture(&dir,&format!("coverage-{name}-v{}",if modern{2}else{1}),&run,modern);}
        }
        std::fs::write(dir.join("records.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
        settings::set(Default::default());
    }
}

#[cfg(test)]
mod effects_study {
    use super::tests::capture;
    use angel_delve::render::overworld::arena::{self,art_v2::testing};
    #[test]
    #[ignore = "writes four-seat effect display trials and unchanged simulation"]
    fn write_effect_study() {
        let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_EFFECT_STUDY").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let mut records=Vec::new();
        for id in ["iron-tusk-chief","cinderjaw","mother-of-spores","the-index","waxen-warden","the-bone-choir","crypt-tithe-marshal"] {
            let mut run=testing::boss(id);
            for seat in 2..=4 {run.join_seat(seat,&format!("Knight {seat}"));}
            for _ in 0..133 {for hero in run.players.values_mut(){hero.hp=hero.max_hp;}run.step(&Default::default());}
            let original=serde_json::to_value(&run).unwrap();
            for sample in 0..=3 {
                arena::boss_effect_review(sample);
                for modern in [false,true] {
                    let name=format!("{id}-trial-{sample}-v{}",if modern{2}else{1});
                    capture(&dir,&name,&run,modern);
                    assert_eq!(original,serde_json::to_value(&run).unwrap());
                    records.push(serde_json::json!({"name":name,"boss":id,"sample":sample,"kit":if modern{2}else{1},"players":run.players.len(),"shots":run.projectiles.len(),"tick":run.tick}));
                }
            }
        }
        arena::boss_effect_review(0);
        angel_delve::settings::set(Default::default());
        std::fs::write(dir.join("records.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
    }
}

#[cfg(test)]
mod hitstop;
