//! Pixel and wire proof from the preserved four-seat dungeon snapshots.
use angel_delve::{render::overworld::arena, rules::Run, settings::{self, Settings}};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

#[test]
#[ignore = "exports exact RGB and production cell bytes for preserved dungeon states"]
fn write_dungeon_snapshot_bytes() {
    let input=std::path::PathBuf::from(std::env::var_os("ANGEL_DUNGEON_SNAPSHOTS").unwrap());
    let out=std::path::PathBuf::from(std::env::var_os("ANGEL_DUNGEON_BYTES").unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let mut files=std::fs::read_dir(input).unwrap().map(|p|p.unwrap().path()).filter(|p|p.to_string_lossy().ends_with("-run.json")).collect::<Vec<_>>();
    files.sort();
    for path in files {
        let bytes=std::fs::read(&path).unwrap();
        let run:Run=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(run.players.len(),4);
        let state=serde_json::to_vec(&run).unwrap();
        let name=path.file_name().unwrap().to_str().unwrap().trim_end_matches("-run.json");
        let modern=name.ends_with("v2");
        settings::set(Settings{art_v2:modern,feel_ledger:false});
        let (w,h)=if modern {(768,448)}else{(384,224)};
        let image=arena::frame_for(&run,w,h,Some(1));
        std::fs::write(out.join(format!("{name}.rgb")),image.rgb_bytes()).unwrap();
        let mut terminal=Terminal::new(TestBackend::new(96,28)).unwrap();
        terminal.draw(|f| {
            if modern {
                let (base,fx)=arena::art_v2::terminal_layers(&run,Some(1)).unwrap();
                super::cells::paint_cells(f,Rect::new(0,0,96,28),&base,&fx);
            } else {
                let im=arena::frame_for(&run,96,56,Some(1));
                for y in 0..28u16 {for x in 0..96u16 {
                    let [r,g,b]=im.get(x as i32,y as i32*2).unwrap();
                    let [r1,g1,b1]=im.get(x as i32,y as i32*2+1).unwrap();
                    f.buffer_mut()[(x,y)].set_char('▀').set_fg(Color::Rgb(r,g,b)).set_bg(Color::Rgb(r1,g1,b1));
                }}
            }
        }).unwrap();
        let rgb=|c:Color|match c {Color::Rgb(r,g,b)=>[r,g,b],_=>[0;3]};
        let cells=terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"glyph":c.symbol(),"fg":rgb(c.fg),"bg":rgb(c.bg)})).collect::<Vec<_>>();
        std::fs::write(out.join(format!("{name}-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
        assert!(state==serde_json::to_vec(&run).unwrap(),"painting changed simulation: {name}");
    }
    settings::set(Default::default());
}
