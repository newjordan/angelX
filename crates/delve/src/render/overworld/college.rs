//! Gate-side map landmark, separate from the Sloptomizer school.
use super::arena;
use super::light::Light;
use super::map::TILE;
use super::scene::Prop;

// Where the college stands is the world's.
pub use crate::world::college::{approach, lot, stand};

pub fn stage(tick: u32, props: &mut Vec<Prop>, lights: &mut Vec<Light>) {
    let lot = lot();
    let base = (lot.ty + lot.th) * TILE - 1;
    let hall = arena::college::building(tick);
    props.push(Prop {
        x: lot.tx * TILE,
        base,
        img: hall,
    });
    let tower = arena::college::tower(tick);
    let tower_x = (lot.tx + 4) * TILE;
    let tower_top = base - 9 - tower.h;
    let tower_w = tower.w;
    props.push(Prop {
        x: tower_x,
        base: base - 9,
        img: tower,
    });
    lights.push(Light {
        x: ((lot.tx + 2) * TILE) as f32,
        y: (base - 15) as f32,
        r: 30.0,
        s: 0.2,
        fire: false,
    });
    let mut plaque = super::ink::Img::new(107, 13);
    plaque.rect(0, 0, 107, 13, 's');
    plaque.frame(0, 0, 107, 13, 'u');
    plaque.text(4, 3, "Labyrinth College", 'i');
    // Above the tower, clear of the door. Knights stand on the name's old spot.
    props.push(Prop {
        x: tower_x + (tower_w - plaque.w) / 2,
        base: tower_top,
        img: plaque,
    });
}

pub fn approach_mark(wx: i32, wy: i32) -> Option<char> {
    use super::{ink::hash, map::Realm};
    let tile = (wx.div_euclid(TILE), wy.div_euclid(TILE));
    if !approach().contains(&tile) {
        return None;
    }
    let (x, y) = (wx.rem_euclid(TILE), wy.rem_euclid(TILE));
    if matches!(Realm::get().at(tile.0, tile.1), b'~' | b'w' | b'f') {
        if (2..14).contains(&x) {
            Some(if y % 5 == 0 {
                'O'
            } else if x == 2 || x == 13 {
                'r'
            } else {
                'P'
            })
        } else {
            None
        }
    } else {
        match hash(wx, wy, 41) % 23 {
            0..=2 => Some('P'),
            3 => Some('r'),
            _ => None,
        }
    }
}
