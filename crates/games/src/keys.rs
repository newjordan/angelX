use angel_net::input::button;
use crossterm::event::KeyCode;

pub fn standard(code: KeyCode) -> u32 {
    match code {
        KeyCode::Up | KeyCode::Char('w') => button::UP,
        KeyCode::Down | KeyCode::Char('s') => button::DOWN,
        KeyCode::Left | KeyCode::Char('a') => button::LEFT,
        KeyCode::Right | KeyCode::Char('d') => button::RIGHT,
        KeyCode::Char(' ') | KeyCode::Char('f') => button::FIRE,
        KeyCode::Enter | KeyCode::Char('e') => button::USE,
        KeyCode::Char('q') => button::ALT,
        KeyCode::Char('r') => button::JUMP,
        KeyCode::Tab => button::MAP,
        KeyCode::Char('1') => button::SLOT1,
        KeyCode::Char('2') => button::SLOT2,
        KeyCode::Char('3') => button::SLOT3,
        KeyCode::Char('4') => button::SLOT4,
        KeyCode::Char('i') => button::AIM_UP,
        KeyCode::Char('k') => button::AIM_DOWN,
        KeyCode::Char('j') => button::AIM_LEFT,
        KeyCode::Char('l') => button::AIM_RIGHT,
        _ => 0,
    }
}
