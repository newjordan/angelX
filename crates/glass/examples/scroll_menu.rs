//! Offline, self-closing visual verifier for the real foreground scroll renderer.
//! ANGEL_SCROLL_OUT=/absolute/workspace/path cargo run --example scroll_menu
use angel_glass::scroll::{Motion, Page};
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    terminal::Terminal,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() {
    let out = std::env::var_os("ANGEL_SCROLL_OUT")
        .map(PathBuf::from)
        .expect("ANGEL_SCROLL_OUT is required (capture directory)");
    std::fs::create_dir_all(&out).unwrap();
    let mut options = angel_glass::Options::from_environment();
    options.columns = 118;
    options.rows = 44;
    options.font_size = 14.0;
    let result = angel_glass::run(options, move |host| {
        let mut terminal = Terminal::new(host.backend()).unwrap();
        let mut motion = Motion::default();
        let start = Instant::now();
        motion.target(true, start);
        for frame in 0..150 {
            let now = start + Duration::from_secs_f64(frame as f64 / 60.0);
            if frame == 74 {
                motion.target(false, now);
            }
            if frame == 86 {
                motion.target(true, now);
            }
            if frame == 114 {
                motion.target(false, now);
            }
            let pose = motion.advance(now);
            let size = host.grid_size();
            let w = size.cols.saturating_sub(12).min(96);
            let h = size.rows.saturating_sub(6).min(34);
            let area = Rect::new((size.cols - w) / 2, (size.rows - h) / 2, w, h);
            let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
            let ink = Style::default().fg(Color::Rgb(62, 40, 22));
            let title = ink.fg(Color::Rgb(128, 34, 24)).add_modifier(Modifier::BOLD);
            let copy = [
                "CONNECT ANGEL TO AN AI MODEL",
                "",
                "Your workspace stays here. Bring a model to the table.",
                "",
                "Choose how you would like to connect:",
                "",
                "  1  Sign in with ChatGPT",
                "  2  Use a model on this computer",
                "  3  Connect with an API key",
                "  4  Find a model on your network",
                "",
                "  5  Back to the cockpit",
                "",
                "No account settings are changed by this preview.",
            ];
            for (i, line) in copy.iter().enumerate() {
                if 4 + i as u16 >= h.saturating_sub(4) {
                    break;
                }
                let mut style = if i == 0 { title } else { ink };
                if i == 6 {
                    style = ink
                        .bg(Color::Rgb(207, 178, 125))
                        .add_modifier(Modifier::BOLD);
                }
                buf.set_stringn(
                    5,
                    4 + i as u16,
                    *line,
                    usize::from(w.saturating_sub(10)),
                    style,
                );
            }
            if h > 7 {
                buf.set_stringn(
                    5,
                    h - 5,
                    "Esc  Back      Enter  Choose      ↑ ↓  Navigate",
                    usize::from(w.saturating_sub(10)),
                    ink,
                );
            }
            let page = if motion.hidden() {
                None
            } else {
                Page::new(area, buf, pose)
            };
            host.set_scroll(page);
            terminal.draw(|f| {
                let r=f.area();
                let b=f.buffer_mut();
                for yy in 0..r.height {
                    let label=if yy==1 {"  ANGEL   /   WORKSPACE                                      ENGINEERING COCKPIT"}
                        else if yy==4 {"  RESEARCH TRACE                         ARTIFACTS / EVIDENCE"}
                        else if yy==r.height.saturating_sub(3) {"  > Write a message...                                         /setup  /table"}
                        else if yy>6 && yy<r.height.saturating_sub(5) {"  │ retain the evidence                   │  verified source  ·  native UI         │"}
                        else {"  ───────────────────────────────────────────────────────────────────────────────"};
                    b.set_stringn(0,yy,label,usize::from(r.width),Style::default().fg(Color::Rgb(125,152,164))
                        .bg(Color::Rgb(17,24,33)));
                }
            }).unwrap();
            if frame % 5 == 0 {
                let path = out.join(format!("frame-{:03}.png", frame / 5));
                host.capture(&path);
                let mut complete = false;
                for _ in 0..3000 {
                    // PNG IEND is the completion marker; file creation alone races writes.
                    if std::fs::read(&path).is_ok_and(|bytes| {
                        bytes.ends_with(&[0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130])
                    }) {
                        complete = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                assert!(complete, "capture timed out before PNG completion: {}", path.display());
            }
        }
        host.set_scroll(None);
    });
    if let Err(error) = result {
        eprintln!("scroll preview: {error}");
        std::process::exit(1);
    }
}
