//! Fingerposts drawn (`crate::world::signs`): a timber post with a board
//! for each arm, in the realm's dusk timber, standing in the landscape (the
//! cockpit's map and the Delve's kingdom both). The arms' names are
//! lettered beside the post, small and grey, only while a knight is near
//! ([`arms`]), so they read as you pass and never clutter the map.

use super::ink::{Img, text_width};
use super::scene::Prop;
use super::TILE;
use crate::world::signs::{self, Post, Way};

/// A fingerpost with a board pointing each of `ways`: 15 x 22 pixels, the
/// post up its middle on a stone footing, in the dusk timbers (nothing
/// brighter than `r`) so it reads on black paper without shining.
pub fn sprite(ways: &[Way]) -> Img {
    let mut im = Img::new(15, 22);
    im.rect(7, 3, 2, 17, 'P');
    im.rect(8, 3, 1, 17, 'B');
    im.rect(7, 2, 2, 1, 'B');
    im.rect(5, 19, 6, 2, 'G');
    im.rect(5, 21, 6, 1, 'X');
    for way in ways {
        match way {
            Way::East => {
                im.rect(9, 5, 4, 3, 'r');
                im.rect(9, 7, 4, 1, 'B');
                im.put(13, 6, 'r');
                im.put(14, 6, 'B');
            }
            Way::West => {
                im.rect(2, 9, 5, 3, 'r');
                im.rect(2, 11, 5, 1, 'B');
                im.put(1, 10, 'r');
                im.put(0, 10, 'B');
            }
            Way::North => {
                im.rect(4, 1, 2, 3, 'r');
                im.put(4, 0, 'B');
                im.put(5, 0, 'r');
            }
            Way::South => {
                im.rect(10, 12, 2, 3, 'r');
                im.put(10, 15, 'B');
                im.put(11, 15, 'r');
            }
        }
    }
    im
}

/// The world pixel the post's foot stands on.
fn foot(post: &Post) -> (i32, i32) {
    (post.at.0 * TILE + TILE / 2, post.at.1 * TILE + 14)
}

/// Every fingerpost as a prop in realm coordinates.
pub fn stage(props: &mut Vec<Prop>) {
    for post in signs::posts() {
        let ways: Vec<Way> = post.arms.iter().map(|a| a.0).collect();
        let img = sprite(&ways);
        let (x, base) = foot(post);
        props.push(Prop {
            x: x - img.w / 2,
            base,
            img,
        });
    }
}

/// Letter the arms of every post within [`signs::READ`] tiles of `near`
/// (tile units) onto `cv`, whose top-left is world pixel `origin`; returns
/// the rectangles they take.
pub fn arms(cv: &mut Img, origin: (i32, i32), near: (f32, f32)) -> Vec<(i32, i32, i32, i32)> {
    let mut taken = Vec::new();
    for post in signs::posts() {
        let (px, py) = (post.at.0 as f32 + 0.5, post.at.1 as f32 + 0.5);
        if (px - near.0).hypot(py - near.1) > signs::READ {
            continue;
        }
        let (fx, fy) = foot(post);
        let (x, y) = (fx - origin.0, fy - origin.1 - 22);
        for (way, name) in &post.arms {
            let text = name.to_uppercase();
            let w = text_width(&text);
            let (tx, ty) = match way {
                Way::East => (x + 10, y + 3),
                Way::West => (x - 10 - w, y + 7),
                Way::North => (x - w / 2, y - 11),
                Way::South => (x - w / 2, y + 26),
            };
            cv.rect(tx - 2, ty - 1, w + 4, 9, 'K');
            cv.text(tx, ty, &text, 'h');
            taken.push((tx - 2, ty - 1, w + 4, 9));
        }
    }
    taken
}
