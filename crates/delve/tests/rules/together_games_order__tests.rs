//! A game's index is its first-win bit and its mark in saves, so Siege —
//! the newest game — must sit at the end of `Game::ALL`, after every game
//! that existed before it. Pin the index of each of those games so a
//! future insertion in the middle cannot slip through.

use super::*;

#[test]
fn siege_is_last_and_every_older_game_keeps_its_index() {
    assert_eq!(Game::ALL.len(), 19);
    assert_eq!(Game::Siege.index(), 18, "Siege is the last game");
    let before_siege = [
        (Game::Broadsheet, 0usize),
        (Game::Barrel, 1),
        (Game::Turnip, 2),
        (Game::Washtub, 3),
        (Game::Cheese, 4),
        (Game::Hobbyhorse, 5),
        (Game::Sled, 6),
        (Game::Pot, 7),
        (Game::Cart, 8),
        (Game::Surf, 9),
        (Game::Volley, 10),
        (Game::Halfpipe, 11),
        (Game::Chariot, 12),
        (Game::Trencher, 13),
        (Game::DuckShoot, 14),
        (Game::Longbow, 15),
        (Game::Powder, 16),
        (Game::Gander, 17),
    ];
    for (game, index) in before_siege {
        assert_eq!(game.index(), index, "{game:?} keeps its save index");
    }
    // Every game is in ALL exactly once.
    for a in Game::ALL {
        assert_eq!(Game::ALL.iter().filter(|b| **b == a).count(), 1);
    }
}
