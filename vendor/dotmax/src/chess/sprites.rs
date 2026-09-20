//! Braille sprites for chess pieces.

use shakmaty::{Color, Role};

/// An 8x8 dot mask for a chess piece.
pub struct PieceSprite {
    /// The bitmask representing the piece dots.
    pub mask: [u8; 8],
}

/// Returns the braille sprite mask for a given chess piece role and color.
pub fn get_sprite(role: Role, _color: Color) -> PieceSprite {
    match role {
        Role::Pawn => PieceSprite {
            mask: [
                0b00000000, 0b00000000, 0b00011000, 0b00111100, 0b00011000, 0b00111100, 0b01111110,
                0b00000000,
            ],
        },
        Role::Knight => PieceSprite {
            mask: [
                0b00000000, 0b00111100, 0b01101100, 0b00001100, 0b00011000, 0b00111100, 0b01111110,
                0b00000000,
            ],
        },
        Role::Bishop => PieceSprite {
            mask: [
                0b00011000, 0b00111100, 0b00100100, 0b01011010, 0b00111100, 0b00111100, 0b01111110,
                0b00000000,
            ],
        },
        Role::Rook => PieceSprite {
            mask: [
                0b00000000, 0b01011010, 0b01111110, 0b01111110, 0b01111110, 0b01111110, 0b01111110,
                0b00000000,
            ],
        },
        Role::Queen => PieceSprite {
            mask: [
                0b10101010, 0b11111111, 0b01111110, 0b01111110, 0b00111100, 0b00111100, 0b11111111,
                0b00000000,
            ],
        },
        Role::King => PieceSprite {
            mask: [
                0b00011000, 0b01111110, 0b00011000, 0b01111110, 0b11111111, 0b11111111, 0b01111110,
                0b00000000,
            ],
        },
    }
}
