//! The cave's ground as a seat sees it: rock, wall faces, floor and the
//! lamps' warm pools, at native size.
//!
//! Two layers. The plate holds what every world pixel is made of (an ink and
//! a kind); a cell of it is drawn once and kept until that cell, or one it
//! leans on, changes. The light pass turns the plate into colour each frame
//! from the lamps the knights carry, so a knight can cross a cell a pixel at
//! a time without the plate being drawn again.

use std::cell::RefCell;
use std::sync::OnceLock;

use super::light;
use crate::ink::{self, bayer, hash, vnoise, Img};
use crate::rules::caves::mines::{self, PROVE_AT};
use crate::rules::caves::world::{
    region_of, Cave, Cell, CELL, FL_SEEN, FLUID_ACID, FLUID_LAVA, FLUID_WATER, MAP_H, MAP_W, NATIVE_H,
    NATIVE_W,
};

/// A wall's south face, in pixels, and the shadow at its foot.
pub(crate) const FACE_PX: i32 = 20;
const SHADOW_PX: i32 = 5;
/// Tops are drawn in their own cell square. The painter does not lift them.
const TOP_LIFT: i32 = 0;

/// The plate's size: the whole cave in pixels.
const PLATE_W: i32 = MAP_W * CELL;
const PLATE_H: i32 = MAP_H * CELL;

pub(crate) fn cell_px(x: i32, y: i32) -> (i32, i32) {
    (x * CELL + CELL / 2, y * CELL + CELL / 2)
}

/// The view's top-left world pixel when it is centred on `centre`, held inside the cave.
pub(crate) fn camera(centre: (i32, i32)) -> (i32, i32) {
    let x = (centre.0 - NATIVE_W / 2).clamp(0, PLATE_W - NATIVE_W);
    let y = (centre.1 - NATIVE_H / 2).clamp(0, PLATE_H - NATIVE_H);
    (x, y)
}

// ---------------------------------------------------------------------------
// Lamps
// ---------------------------------------------------------------------------

/// A light a knight carries, or the lamp that marks the way out.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Lamp {
    /// Where it shines from, in world pixels.
    pub px: (i32, i32),
    /// The cell its line of sight is cast from.
    pub cell: (i32, i32),
    /// The way-out lamp: half a knight's reach.
    pub small: bool,
}

/// A knight's light reaches this many cells, where nothing stands in its way.
const KNIGHT_CELLS: i32 = 20;
const SMALL_CELLS: i32 = 10;
/// At most this many lamps light one frame: a bit each in `Light::los`.
const LAMPS: usize = 8;

/// The flat heart of a lamp's pool, then the three bands' outer edges, in pixels.
const HEART: i32 = 28;
const BAND_EDGE: [i32; 3] = [52, 104, 156];
/// The light level of the heart and of each band beyond it.
const BAND_LIT: [u8; 4] = [8, 6, 4, 0];
/// The farthest a pool reaches on one axis: the last edge and its wander.
const REACH: i32 = 164;
const RING_SIDE: i32 = REACH * 2 + 1;

const WANDER: [[i32; 64]; 3] = [
    [2, 3, 3, 4, 4, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 5, 5, 4, 4, 3, 3, 2, 2, 1, 1, 0, -1, -1, -2, -2, -3, -3, -4, -4, -5, -5, -5, -6, -6, -6, -6, -6, -6, -6, -6, -6, -5, -5, -5, -4, -4, -3, -3, -2, -2, -1, -1, 0, 1, 1, 2],
    [5, 5, 4, 4, 4, 3, 3, 2, 2, 1, 0, 0, -1, -1, -2, -3, -3, -4, -4, -4, -5, -5, -5, -6, -6, -6, -6, -6, -6, -6, -6, -5, -5, -5, -4, -4, -4, -3, -3, -2, -2, -1, 0, 0, 1, 1, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 5],
    [-5, -5, -5, -5, -6, -6, -6, -6, -6, -6, -6, -6, -5, -5, -5, -4, -4, -3, -3, -2, -2, -1, -1, 0, 0, 1, 2, 2, 3, 3, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 5, 5, 5, 4, 4, 3, 3, 2, 2, 1, 1, 0, 0, -1, -2, -2, -3, -3, -4, -4],
];

/// Fixed 64 by 64 blue-noise ranks. Looked up by world pixel so the band does not swim.
const LAMP_NOISE: [u8; 4096] = [
    123, 146, 66, 251, 102, 137, 64, 185, 108, 246, 64, 173, 118, 192, 82, 244, 75, 220, 84, 159, 75, 184, 95, 138, 106, 178, 82, 221, 70, 195, 72, 160, 122, 254, 98, 221, 103, 178, 81, 195, 89, 140, 72, 135, 119, 173, 108, 198, 76, 195, 123, 247, 64, 179, 118, 234, 109, 158, 80, 216, 114, 230, 71, 212,
    205, 2, 144, 38, 142, 12, 186, 54, 208, 1, 188, 35, 178, 13, 209, 52, 132, 2, 172, 36, 198, 15, 244, 43, 175, 1, 234, 48, 141, 9, 214, 40, 128, 3, 187, 32, 127, 15, 212, 32, 137, 1, 220, 37, 167, 9, 130, 53, 142, 3, 150, 61, 254, 8, 239, 60, 167, 1, 216, 56, 139, 8, 219, 34,
    72, 141, 85, 190, 70, 147, 87, 215, 98, 174, 123, 204, 66, 249, 114, 180, 72, 193, 92, 232, 122, 227, 92, 213, 66, 218, 83, 153, 103, 167, 104, 234, 103, 147, 101, 230, 89, 137, 74, 141, 101, 188, 82, 225, 126, 201, 74, 151, 79, 203, 119, 147, 116, 241, 84, 191, 83, 138, 74, 205, 92, 136, 91, 127,
    189, 39, 142, 20, 167, 47, 132, 17, 134, 33, 233, 19, 152, 53, 150, 29, 178, 62, 173, 20, 236, 39, 161, 22, 207, 49, 212, 21, 231, 60, 199, 28, 148, 51, 143, 31, 155, 51, 170, 25, 173, 60, 242, 30, 163, 39, 173, 26, 215, 56, 168, 26, 247, 42, 232, 18, 143, 42, 240, 26, 142, 33, 140, 17,
    105, 166, 74, 169, 70, 157, 103, 230, 73, 216, 122, 206, 104, 146, 83, 160, 121, 242, 107, 176, 100, 243, 105, 229, 83, 207, 90, 244, 78, 146, 118, 168, 103, 129, 108, 212, 77, 136, 87, 246, 106, 209, 75, 216, 78, 231, 78, 224, 98, 227, 74, 176, 91, 229, 85, 134, 97, 182, 126, 214, 79, 197, 116, 134,
    229, 9, 200, 46, 166, 7, 206, 61, 236, 9, 247, 62, 145, 5, 208, 60, 165, 10, 134, 53, 250, 6, 222, 46, 203, 12, 193, 40, 149, 7, 223, 63, 223, 15, 241, 52, 223, 4, 238, 62, 144, 13, 148, 60, 211, 7, 160, 34, 139, 10, 207, 61, 154, 4, 130, 41, 237, 14, 189, 49, 209, 6, 137, 62,
    109, 185, 117, 177, 111, 228, 113, 183, 86, 254, 95, 146, 110, 240, 120, 240, 101, 251, 100, 217, 89, 149, 109, 248, 80, 225, 66, 179, 68, 137, 105, 219, 124, 153, 109, 140, 105, 170, 67, 215, 122, 130, 79, 212, 67, 186, 101, 128, 84, 160, 110, 175, 88, 221, 120, 138, 120, 171, 121, 159, 117, 246, 112, 247,
    252, 41, 144, 24, 133, 57, 229, 29, 163, 34, 146, 21, 209, 55, 154, 25, 194, 53, 159, 24, 130, 40, 205, 29, 215, 55, 237, 18, 139, 57, 201, 31, 139, 52, 211, 19, 158, 51, 193, 20, 179, 48, 215, 29, 131, 38, 166, 20, 253, 44, 181, 30, 152, 46, 138, 26, 225, 37, 146, 28, 224, 34, 213, 28,
    80, 234, 126, 190, 65, 233, 74, 225, 77, 134, 100, 155, 125, 142, 70, 129, 75, 180, 109, 140, 107, 198, 67, 127, 68, 153, 90, 141, 125, 162, 68, 178, 104, 196, 100, 161, 108, 248, 123, 240, 107, 149, 89, 213, 103, 248, 106, 136, 89, 244, 97, 246, 125, 218, 65, 139, 84, 249, 75, 165, 96, 250, 109, 191,
    244, 0, 195, 56, 134, 11, 167, 44, 147, 2, 181, 36, 241, 14, 132, 41, 131, 0, 135, 41, 237, 13, 205, 44, 182, 3, 253, 36, 244, 13, 213, 32, 164, 0, 224, 40, 192, 13, 235, 32, 242, 3, 130, 58, 233, 12, 209, 53, 140, 0, 205, 37, 197, 8, 150, 51, 197, 2, 132, 55, 203, 15, 207, 52,
    117, 155, 127, 197, 87, 175, 64, 209, 67, 194, 80, 154, 69, 227, 74, 231, 97, 240, 114, 239, 94, 193, 93, 170, 92, 162, 111, 167, 112, 237, 126, 140, 113, 153, 125, 213, 111, 152, 97, 134, 69, 238, 106, 235, 85, 187, 109, 241, 71, 245, 120, 246, 72, 151, 89, 164, 91, 220, 68, 171, 83, 237, 95, 181,
    168, 57, 192, 19, 175, 34, 146, 18, 212, 47, 225, 26, 211, 63, 142, 19, 146, 47, 214, 21, 206, 55, 243, 29, 211, 54, 240, 22, 166, 34, 163, 21, 191, 58, 203, 19, 237, 58, 193, 17, 195, 47, 182, 30, 145, 43, 136, 29, 161, 43, 133, 16, 184, 32, 254, 19, 192, 33, 208, 24, 157, 36, 174, 22,
    114, 170, 114, 232, 114, 190, 72, 196, 105, 209, 127, 183, 108, 169, 103, 223, 107, 175, 126, 169, 67, 206, 71, 241, 66, 221, 125, 181, 105, 228, 109, 128, 89, 179, 121, 156, 107, 218, 80, 252, 66, 145, 77, 250, 124, 213, 93, 152, 103, 131, 79, 195, 85, 198, 83, 246, 126, 217, 126, 221, 102, 186, 97, 236,
    130, 9, 240, 35, 162, 7, 200, 52, 202, 10, 248, 45, 215, 4, 158, 57, 222, 8, 235, 45, 207, 3, 134, 45, 241, 8, 181, 60, 164, 7, 235, 42, 146, 15, 226, 53, 222, 7, 197, 37, 242, 15, 180, 60, 150, 4, 171, 45, 157, 12, 157, 62, 254, 4, 198, 59, 160, 10, 187, 61, 243, 5, 136, 45,
    88, 211, 108, 168, 64, 231, 83, 199, 88, 242, 90, 253, 92, 225, 82, 242, 111, 247, 93, 197, 84, 250, 90, 160, 78, 141, 88, 253, 112, 181, 103, 167, 85, 183, 76, 230, 70, 218, 99, 159, 87, 179, 97, 143, 125, 239, 99, 159, 101, 214, 121, 233, 82, 161, 114, 207, 120, 235, 125, 151, 98, 238, 73, 128,
    189, 56, 250, 28, 204, 58, 160, 22, 236, 44, 225, 21, 196, 49, 140, 27, 132, 44, 181, 25, 145, 59, 144, 27, 144, 48, 218, 19, 218, 46, 153, 25, 129, 39, 230, 23, 243, 53, 155, 28, 217, 50, 186, 24, 226, 59, 241, 18, 154, 61, 229, 27, 147, 45, 243, 24, 148, 44, 184, 17, 146, 62, 200, 26,
    102, 237, 123, 217, 94, 174, 96, 168, 121, 219, 101, 155, 96, 131, 74, 155, 72, 165, 75, 174, 99, 175, 78, 245, 125, 131, 68, 211, 123, 140, 98, 246, 67, 211, 118, 195, 115, 214, 101, 161, 108, 243, 67, 194, 111, 136, 118, 193, 64, 229, 120, 210, 106, 221, 98, 235, 69, 245, 114, 231, 118, 240, 115, 133,
    142, 3, 201, 56, 203, 14, 161, 53, 185, 1, 206, 50, 144, 13, 141, 33, 179, 1, 254, 42, 254, 11, 191, 57, 152, 1, 179, 36, 235, 11, 214, 42, 218, 3, 238, 40, 187, 13, 195, 61, 169, 1, 145, 39, 239, 11, 169, 39, 192, 3, 181, 50, 178, 12, 250, 51, 209, 1, 252, 55, 189, 11, 135, 56,
    84, 234, 77, 174, 115, 154, 76, 147, 75, 243, 83, 171, 127, 228, 73, 243, 72, 224, 63, 215, 79, 235, 95, 225, 111, 188, 84, 178, 97, 231, 73, 147, 86, 200, 79, 238, 73, 225, 113, 136, 65, 173, 86, 218, 122, 250, 66, 190, 72, 190, 110, 221, 78, 180, 119, 163, 69, 161, 67, 251, 91, 216, 94, 209,
    202, 55, 236, 25, 232, 33, 172, 20, 143, 42, 223, 19, 214, 57, 247, 22, 232, 41, 178, 19, 220, 34, 233, 21, 189, 34, 145, 29, 232, 41, 129, 17, 166, 38, 215, 19, 197, 61, 200, 19, 246, 57, 243, 25, 170, 32, 151, 31, 137, 37, 252, 28, 184, 40, 248, 24, 146, 36, 206, 30, 206, 42, 151, 18,
    67, 208, 118, 233, 114, 245, 99, 254, 120, 151, 88, 186, 70, 171, 119, 211, 88, 131, 70, 139, 74, 157, 71, 196, 69, 184, 109, 219, 111, 247, 84, 193, 122, 251, 109, 232, 77, 227, 70, 211, 122, 130, 123, 231, 120, 212, 97, 158, 86, 236, 75, 224, 102, 133, 80, 166, 68, 144, 86, 128, 86, 170, 84, 238,
    200, 14, 180, 55, 235, 5, 173, 58, 143, 14, 139, 40, 239, 4, 130, 49, 215, 15, 214, 49, 148, 4, 154, 49, 215, 8, 177, 35, 152, 7, 156, 60, 160, 15, 233, 62, 153, 4, 234, 52, 156, 15, 159, 53, 143, 6, 175, 35, 226, 8, 249, 39, 162, 4, 138, 35, 245, 12, 232, 35, 219, 6, 135, 35,
    105, 180, 89, 129, 94, 128, 89, 213, 94, 241, 75, 174, 118, 177, 76, 226, 71, 226, 98, 168, 100, 238, 70, 227, 88, 204, 91, 222, 93, 238, 103, 165, 90, 150, 102, 180, 122, 147, 102, 156, 125, 252, 104, 172, 94, 246, 116, 254, 76, 154, 124, 246, 119, 234, 115, 172, 93, 136, 119, 188, 75, 154, 113, 210,
    221, 53, 215, 27, 156, 56, 129, 20, 177, 57, 159, 24, 163, 44, 239, 23, 225, 41, 129, 31, 127, 49, 194, 17, 168, 50, 159, 25, 158, 34, 240, 16, 136, 49, 188, 27, 182, 40, 146, 16, 132, 35, 147, 31, 193, 47, 210, 23, 147, 55, 152, 17, 169, 41, 142, 29, 177, 56, 153, 28, 181, 49, 131, 25,
    84, 212, 108, 251, 94, 250, 104, 139, 88, 172, 93, 160, 115, 252, 88, 186, 68, 182, 88, 188, 75, 247, 78, 165, 68, 240, 108, 176, 96, 155, 126, 182, 90, 248, 71, 233, 80, 169, 79, 185, 83, 225, 123, 159, 70, 127, 103, 214, 108, 216, 67, 218, 87, 143, 100, 165, 114, 233, 123, 142, 102, 187, 77, 135,
    231, 0, 211, 38, 140, 11, 171, 52, 162, 2, 228, 54, 147, 11, 177, 58, 165, 0, 210, 33, 243, 8, 245, 54, 165, 2, 248, 39, 135, 13, 152, 47, 163, 0, 181, 59, 232, 15, 170, 53, 188, 2, 244, 58, 149, 10, 148, 48, 207, 0, 161, 43, 197, 11, 134, 57, 210, 2, 183, 32, 144, 11, 250, 40,
    92, 153, 86, 236, 79, 170, 106, 242, 103, 202, 87, 172, 78, 164, 72, 226, 118, 224, 82, 205, 125, 210, 104, 171, 74, 182, 76, 191, 114, 138, 107, 164, 116, 165, 82, 154, 95, 175, 100, 251, 73, 210, 97, 228, 87, 141, 74, 250, 64, 187, 73, 165, 85, 192, 101, 150, 120, 237, 102, 246, 112, 216, 69, 223,
    231, 46, 136, 17, 210, 61, 219, 31, 199, 49, 250, 18, 164, 63, 237, 23, 194, 56, 164, 26, 196, 34, 151, 31, 227, 60, 176, 21, 242, 58, 171, 18, 137, 50, 183, 17, 138, 57, 220, 23, 244, 61, 249, 30, 163, 35, 130, 25, 235, 39, 217, 26, 178, 38, 238, 16, 227, 33, 154, 30, 212, 37, 207, 15,
    99, 192, 69, 185, 102, 190, 83, 198, 81, 234, 112, 198, 64, 248, 76, 200, 75, 212, 116, 148, 99, 153, 117, 192, 102, 159, 68, 197, 121, 137, 106, 210, 115, 128, 102, 210, 94, 226, 104, 180, 84, 226, 108, 242, 100, 219, 111, 189, 99, 156, 75, 135, 87, 219, 64, 165, 81, 243, 127, 198, 113, 245, 99, 160,
    130, 11, 148, 62, 224, 5, 196, 36, 234, 12, 240, 47, 252, 7, 143, 35, 205, 13, 203, 63, 239, 6, 130, 45, 186, 15, 196, 59, 219, 7, 227, 40, 128, 9, 174, 47, 238, 6, 202, 56, 156, 10, 240, 42, 159, 4, 142, 52, 220, 12, 222, 57, 143, 5, 161, 58, 255, 8, 133, 38, 191, 5, 242, 63,
    90, 169, 69, 223, 73, 156, 119, 251, 81, 154, 107, 252, 112, 202, 89, 211, 73, 228, 84, 236, 63, 176, 82, 203, 116, 138, 70, 143, 105, 154, 93, 153, 96, 156, 108, 184, 88, 228, 114, 194, 101, 211, 98, 213, 108, 235, 71, 173, 77, 177, 63, 135, 71, 219, 82, 219, 124, 142, 65, 189, 73, 182, 110, 187,
    247, 63, 213, 22, 245, 39, 220, 28, 229, 45, 204, 25, 169, 49, 153, 24, 141, 43, 194, 30, 143, 34, 222, 20, 169, 47, 209, 26, 139, 42, 229, 19, 216, 47, 177, 27, 226, 37, 130, 22, 131, 40, 163, 24, 158, 59, 249, 24, 192, 59, 188, 23, 228, 52, 231, 25, 208, 38, 238, 21, 209, 44, 199, 23,
    85, 154, 85, 195, 107, 185, 126, 186, 117, 163, 126, 140, 115, 236, 83, 152, 98, 179, 96, 213, 123, 172, 74, 189, 70, 238, 106, 131, 77, 133, 82, 197, 64, 222, 80, 253, 109, 214, 99, 189, 95, 249, 88, 147, 123, 252, 118, 199, 86, 148, 108, 204, 120, 243, 112, 249, 76, 167, 126, 173, 69, 170, 81, 226,
    221, 2, 155, 40, 129, 13, 207, 56, 186, 0, 197, 55, 148, 9, 194, 62, 228, 3, 176, 58, 194, 14, 133, 59, 169, 1, 244, 32, 148, 13, 253, 46, 205, 3, 133, 41, 127, 12, 149, 50, 234, 1, 207, 33, 213, 9, 244, 63, 236, 2, 185, 57, 189, 14, 171, 51, 214, 1, 155, 32, 132, 12, 156, 43,
    126, 220, 96, 194, 80, 187, 110, 212, 118, 227, 78, 154, 84, 249, 76, 239, 68, 251, 80, 149, 77, 212, 78, 244, 89, 193, 122, 197, 77, 249, 114, 134, 101, 202, 87, 164, 88, 199, 87, 232, 114, 142, 120, 188, 95, 163, 101, 144, 85, 240, 117, 216, 110, 161, 105, 166, 110, 151, 102, 241, 100, 179, 119, 223,
    131, 56, 142, 30, 233, 33, 150, 26, 219, 61, 202, 26, 222, 49, 168, 27, 145, 46, 190, 19, 221, 34, 171, 27, 146, 48, 197, 18, 194, 53, 168, 21, 222, 37, 221, 27, 148, 54, 236, 30, 245, 34, 132, 16, 163, 41, 174, 21, 170, 62, 191, 20, 146, 37, 203, 16, 204, 61, 206, 22, 199, 35, 227, 29,
    76, 174, 86, 159, 127, 220, 124, 145, 111, 230, 81, 237, 105, 205, 113, 199, 117, 245, 66, 247, 68, 135, 63, 138, 66, 183, 64, 244, 109, 233, 103, 182, 124, 252, 75, 230, 100, 248, 102, 229, 65, 216, 124, 142, 90, 180, 65, 184, 90, 148, 94, 198, 104, 235, 77, 190, 89, 233, 115, 141, 125, 248, 98, 145,
    243, 12, 175, 60, 226, 7, 188, 53, 165, 12, 180, 46, 218, 6, 211, 55, 173, 9, 217, 61, 190, 4, 128, 52, 158, 14, 221, 59, 239, 5, 136, 50, 187, 8, 229, 36, 234, 6, 188, 45, 191, 10, 133, 38, 129, 5, 148, 52, 132, 14, 150, 49, 160, 4, 178, 38, 201, 8, 163, 34, 129, 7, 131, 39,
    98, 170, 100, 205, 73, 190, 92, 177, 107, 128, 101, 143, 85, 173, 112, 228, 101, 129, 109, 144, 124, 129, 98, 201, 85, 212, 108, 204, 111, 249, 107, 138, 113, 162, 101, 181, 67, 136, 96, 183, 89, 230, 97, 190, 72, 156, 120, 239, 77, 218, 78, 168, 124, 169, 105, 230, 86, 174, 121, 216, 75, 190, 102, 182,
    222, 49, 174, 28, 234, 51, 128, 30, 153, 42, 181, 16, 205, 35, 157, 17, 152, 37, 206, 25, 194, 54, 196, 18, 206, 39, 149, 24, 135, 52, 171, 28, 164, 43, 231, 31, 221, 31, 237, 30, 231, 60, 212, 28, 138, 43, 165, 18, 157, 60, 139, 23, 249, 51, 245, 27, 202, 43, 150, 18, 212, 32, 157, 31,
    116, 224, 91, 167, 111, 220, 78, 194, 66, 179, 120, 160, 90, 172, 95, 200, 87, 234, 112, 136, 88, 149, 91, 148, 92, 162, 80, 154, 121, 176, 69, 155, 102, 226, 110, 232, 112, 231, 69, 231, 126, 213, 66, 186, 94, 209, 110, 162, 117, 157, 71, 217, 119, 157, 118, 146, 117, 225, 76, 250, 116, 173, 73, 221,
    195, 0, 213, 55, 247, 15, 145, 38, 241, 3, 251, 50, 208, 9, 202, 57, 195, 0, 222, 60, 179, 10, 228, 54, 201, 2, 172, 55, 201, 10, 224, 61, 128, 0, 158, 50, 177, 13, 215, 50, 207, 3, 251, 43, 171, 12, 183, 39, 140, 0, 206, 43, 241, 14, 216, 51, 134, 2, 222, 45, 221, 13, 144, 54,
    76, 167, 91, 232, 65, 141, 106, 227, 94, 199, 85, 206, 113, 236, 107, 182, 124, 178, 78, 161, 113, 250, 79, 142, 87, 208, 124, 246, 110, 152, 99, 233, 71, 147, 107, 184, 91, 254, 69, 249, 65, 189, 125, 171, 94, 163, 95, 232, 86, 220, 104, 231, 102, 199, 92, 175, 82, 251, 89, 225, 95, 253, 99, 183,
    194, 58, 197, 24, 128, 36, 204, 18, 174, 33, 201, 21, 239, 53, 215, 23, 228, 33, 192, 23, 198, 41, 224, 17, 205, 44, 202, 21, 248, 59, 203, 27, 137, 39, 164, 27, 150, 46, 151, 22, 254, 45, 191, 21, 189, 53, 218, 16, 133, 36, 176, 18, 219, 34, 254, 20, 167, 48, 189, 23, 238, 32, 134, 30,
    106, 181, 111, 191, 116, 166, 90, 217, 82, 149, 99, 163, 95, 175, 112, 162, 65, 140, 67, 176, 92, 134, 96, 233, 91, 193, 101, 186, 113, 239, 92, 132, 65, 253, 97, 169, 93, 161, 117, 207, 87, 177, 117, 187, 118, 213, 85, 202, 76, 161, 99, 166, 69, 242, 96, 152, 121, 149, 88, 241, 82, 175, 117, 204,
    196, 9, 149, 57, 183, 5, 247, 36, 202, 8, 198, 51, 254, 6, 137, 44, 136, 14, 135, 48, 208, 5, 134, 38, 193, 8, 153, 48, 132, 7, 252, 46, 182, 10, 155, 33, 229, 5, 219, 43, 135, 13, 160, 52, 230, 7, 169, 41, 166, 15, 176, 60, 161, 5, 180, 33, 201, 14, 247, 35, 136, 5, 170, 59,
    119, 230, 70, 156, 123, 220, 76, 188, 104, 193, 115, 252, 109, 199, 79, 145, 76, 162, 67, 252, 81, 168, 100, 214, 80, 205, 113, 233, 116, 145, 115, 174, 86, 176, 64, 220, 125, 131, 104, 217, 112, 166, 121, 156, 111, 224, 104, 151, 101, 225, 72, 208, 80, 152, 94, 197, 93, 223, 123, 246, 120, 200, 71, 178,
    240, 37, 185, 20, 163, 60, 141, 29, 248, 35, 184, 20, 162, 35, 159, 17, 160, 43, 164, 16, 211, 50, 151, 28, 149, 50, 188, 29, 243, 40, 252, 31, 159, 61, 200, 23, 202, 62, 234, 17, 141, 37, 157, 22, 194, 32, 203, 16, 184, 59, 211, 29, 228, 37, 172, 16, 145, 42, 170, 22, 200, 54, 174, 16,
    112, 133, 65, 153, 125, 143, 72, 247, 81, 158, 96, 173, 90, 191, 85, 158, 73, 254, 73, 178, 118, 204, 85, 193, 107, 130, 117, 143, 116, 138, 122, 209, 90, 224, 127, 217, 84, 178, 64, 245, 115, 144, 87, 237, 109, 177, 66, 226, 83, 185, 114, 210, 93, 144, 99, 205, 65, 230, 97, 230, 90, 211, 94, 199,
    202, 2, 243, 36, 239, 9, 150, 32, 137, 1, 137, 59, 190, 8, 181, 44, 198, 2, 205, 51, 150, 9, 222, 44, 213, 1, 224, 42, 227, 9, 184, 34, 200, 3, 217, 57, 226, 14, 173, 32, 152, 1, 185, 40, 131, 7, 167, 46, 157, 3, 135, 55, 201, 10, 145, 47, 242, 1, 236, 37, 225, 13, 245, 54,
    87, 236, 122, 223, 86, 149, 97, 215, 95, 174, 78, 170, 104, 251, 124, 200, 91, 184, 74, 157, 81, 251, 81, 158, 116, 138, 92, 164, 85, 157, 119, 155, 106, 201, 122, 135, 95, 200, 79, 201, 66, 133, 112, 190, 69, 151, 98, 191, 119, 187, 92, 190, 96, 129, 79, 236, 121, 166, 71, 215, 90, 183, 95, 143,
    187, 58, 137, 22, 131, 51, 244, 27, 242, 49, 141, 21, 230, 41, 150, 30, 198, 55, 164, 24, 229, 54, 182, 30, 204, 49, 182, 17, 138, 38, 235, 18, 177, 51, 195, 27, 180, 42, 188, 23, 183, 34, 128, 25, 232, 63, 183, 26, 209, 63, 206, 17, 142, 36, 133, 30, 140, 48, 192, 24, 155, 39, 200, 25,
    98, 229, 99, 175, 123, 196, 84, 171, 106, 185, 80, 194, 110, 179, 72, 144, 124, 179, 68, 253, 95, 223, 75, 185, 110, 150, 86, 235, 68, 167, 123, 198, 124, 160, 76, 149, 106, 218, 84, 241, 106, 166, 96, 186, 74, 189, 119, 205, 106, 254, 83, 189, 83, 178, 126, 128, 65, 192, 116, 249, 122, 239, 117, 130,
    168, 12, 201, 54, 253, 6, 147, 56, 148, 11, 253, 37, 198, 6, 156, 63, 128, 10, 201, 55, 229, 7, 240, 47, 185, 9, 148, 62, 245, 7, 176, 37, 235, 8, 253, 45, 210, 6, 218, 47, 217, 13, 139, 57, 223, 5, 234, 50, 218, 11, 139, 52, 131, 6, 132, 59, 184, 14, 183, 58, 249, 5, 134, 63,
    97, 165, 120, 177, 117, 217, 119, 231, 105, 227, 96, 196, 96, 190, 111, 138, 112, 228, 79, 223, 65, 208, 66, 248, 83, 147, 111, 129, 86, 193, 105, 167, 119, 168, 124, 179, 81, 179, 103, 221, 93, 169, 72, 254, 113, 183, 121, 223, 109, 184, 120, 249, 82, 186, 125, 238, 77, 129, 77, 195, 77, 193, 118, 251,
    143, 43, 250, 20, 229, 46, 153, 24, 195, 38, 210, 28, 228, 51, 235, 27, 226, 47, 220, 19, 152, 54, 132, 21, 158, 42, 197, 20, 204, 40, 242, 23, 156, 55, 149, 29, 239, 33, 201, 22, 166, 36, 253, 30, 192, 61, 208, 31, 185, 44, 127, 21, 150, 44, 238, 18, 175, 56, 141, 16, 214, 50, 165, 17,
    68, 234, 115, 206, 121, 172, 97, 140, 122, 207, 87, 127, 94, 246, 71, 187, 70, 252, 96, 172, 92, 237, 65, 187, 70, 185, 92, 152, 64, 144, 69, 212, 85, 165, 107, 216, 102, 164, 116, 248, 95, 145, 88, 164, 78, 219, 91, 146, 64, 181, 92, 139, 112, 193, 65, 248, 119, 130, 91, 180, 100, 191, 104, 173,
    199, 0, 139, 51, 176, 14, 181, 46, 168, 3, 207, 35, 159, 10, 132, 36, 170, 0, 182, 43, 164, 10, 201, 48, 168, 2, 180, 45, 225, 9, 136, 43, 155, 0, 187, 58, 170, 10, 230, 56, 226, 2, 230, 59, 139, 11, 172, 51, 192, 0, 131, 46, 180, 10, 151, 50, 175, 3, 162, 61, 137, 15, 213, 42,
    110, 162, 67, 243, 110, 160, 98, 233, 64, 167, 100, 247, 115, 207, 122, 237, 82, 155, 73, 252, 67, 184, 93, 222, 72, 199, 126, 170, 107, 169, 104, 153, 126, 216, 121, 210, 81, 207, 122, 217, 81, 191, 103, 180, 117, 204, 71, 137, 89, 186, 124, 158, 125, 188, 100, 224, 93, 161, 79, 200, 83, 227, 77, 186,
    214, 38, 241, 24, 196, 36, 237, 22, 210, 59, 192, 20, 157, 40, 251, 28, 153, 48, 203, 18, 244, 39, 154, 16, 206, 53, 202, 28, 208, 52, 216, 26, 244, 48, 223, 29, 171, 48, 188, 29, 177, 44, 189, 20, 227, 31, 176, 25, 253, 48, 130, 26, 224, 54, 187, 27, 133, 54, 204, 20, 240, 44, 129, 25,
    71, 196, 94, 203, 115, 195, 123, 199, 91, 224, 105, 238, 81, 203, 121, 241, 81, 202, 127, 196, 66, 138, 100, 166, 67, 132, 69, 140, 81, 147, 91, 185, 104, 245, 111, 174, 113, 137, 90, 203, 68, 191, 99, 251, 79, 187, 114, 214, 107, 219, 97, 236, 116, 250, 68, 182, 119, 237, 113, 176, 115, 162, 86, 167,
    157, 14, 176, 32, 150, 6, 229, 45, 242, 15, 208, 38, 144, 6, 196, 48, 241, 11, 133, 33, 217, 4, 232, 42, 177, 11, 248, 41, 162, 4, 222, 54, 141, 11, 199, 41, 134, 4, 132, 62, 168, 12, 151, 50, 136, 6, 228, 33, 166, 8, 242, 48, 195, 4, 163, 32, 246, 12, 238, 51, 203, 5, 156, 62,
    80, 203, 106, 151, 70, 216, 93, 159, 68, 155, 74, 204, 73, 198, 113, 253, 71, 184, 113, 246, 105, 161, 108, 232, 78, 135, 89, 222, 110, 206, 93, 250, 91, 244, 116, 158, 118, 172, 79, 196, 82, 145, 127, 247, 98, 218, 80, 204, 93, 209, 103, 173, 66, 172, 121, 135, 105, 158, 115, 204, 110, 221, 74, 171,
    175, 46, 249, 17, 133, 60, 162, 28, 227, 62, 245, 22, 149, 52, 141, 31, 179, 45, 172, 31, 220, 38, 208, 16, 253, 45, 217, 19, 214, 56, 183, 29, 215, 47, 210, 23, 208, 58, 158, 31, 192, 62, 186, 22, 140, 63, 191, 19, 239, 47, 220, 26, 139, 46, 178, 16, 209, 58, 223, 23, 151, 41, 208, 26,
];

fn isqrt(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

fn angle_bin(dx: i32, dy: i32) -> usize {
    let ax = dx.abs();
    let ay = dy.abs();
    let sum = ax + ay;
    let fine = if sum == 0 { 0 } else { ((ay * 16) / sum).min(15) };
    let quad = if dx >= 0 {
        if dy >= 0 { 0 } else { 3 }
    } else if dy >= 0 {
        1
    } else {
        2
    };
    quad * 16 + fine as usize
}

/// Where an offset from a lamp falls: the level on the near side of its band,
/// the level on the far side, and how far across the band it is (0 to 254).
/// Each edge wanders with the angle, so the pool is not a circle.
fn ring_parts(dx: i32, dy: i32) -> (u8, u8, u8) {
    let dist = isqrt(dx * dx + dy * dy);
    let bin = angle_bin(dx, dy);
    let wobble = [WANDER[0][bin], WANDER[1][bin], WANDER[2][bin]];
    let heart = HEART + wobble[0];
    if dist <= heart {
        return (BAND_LIT[0], BAND_LIT[0], 0);
    }
    let edge = [
        heart,
        BAND_EDGE[0] + wobble[0],
        BAND_EDGE[1] + wobble[1],
        BAND_EDGE[2] + wobble[2],
    ];
    for i in 0..3 {
        if dist < edge[i + 1] {
            let span = edge[i + 1] - edge[i];
            if span < 1 {
                return (BAND_LIT[i + 1], BAND_LIT[i + 1], 0);
            }
            let across = ((dist - edge[i]) * 255 / span) as u8;
            return (BAND_LIT[i], BAND_LIT[i + 1], across);
        }
    }
    (0, 0, 0)
}

/// `ring_parts` for every offset a pool reaches, worked out once: near level,
/// far level and the way across, packed in a word.
fn rings() -> &'static [u16] {
    static RINGS: OnceLock<Vec<u16>> = OnceLock::new();
    RINGS.get_or_init(|| {
        let mut out = Vec::with_capacity((RING_SIDE * RING_SIDE) as usize);
        for dy in -REACH..=REACH {
            for dx in -REACH..=REACH {
                let (near, far, across) = ring_parts(dx, dy);
                out.push((u16::from(near) << 12) | (u16::from(far) << 8) | u16::from(across));
            }
        }
        out
    })
}

/// The light one lamp gives a world pixel, 0 to 8. Between two bands the far
/// level is dithered in by a fixed noise looked up at the world pixel, so
/// the edge does not swim as the lamp moves.
#[inline]
fn lamp_level(rings: &[u16], lamp: &Lamp, wx: i32, wy: i32) -> u8 {
    let (mut dx, mut dy) = (wx - lamp.px.0, wy - lamp.px.1);
    if lamp.small {
        dx *= 2;
        dy *= 2;
    }
    if dx.abs() > REACH || dy.abs() > REACH {
        return 0;
    }
    let ring = rings[((dy + REACH) * RING_SIDE + dx + REACH) as usize];
    let noise = LAMP_NOISE[(((wy & 63) << 6) + (wx & 63)) as usize];
    if u16::from(noise) < ring & 0xff {
        ((ring >> 8) & 0x0f) as u8
    } else {
        (ring >> 12) as u8
    }
}

/// What the lamps reach this frame, for the cells in and just round the view.
pub(crate) struct Light {
    lamps: Vec<Lamp>,
    origin: (i32, i32),
    cols: i32,
    rows: i32,
    /// Per cell: the lamps with a clear line to it, a bit a lamp.
    los: Vec<u8>,
    /// Per cell: some light reaches it, a lamp's or a glow of the cave's own.
    lit: Vec<bool>,
}

impl Light {
    fn index(&self, cx: i32, cy: i32) -> Option<usize> {
        let sx = cx - self.origin.0;
        let sy = cy - self.origin.1;
        (sx >= 0 && sy >= 0 && sx < self.cols && sy < self.rows).then(|| (sy * self.cols + sx) as usize)
    }

    /// The light on a floor pixel: the brightest lamp that can see its cell.
    /// A cell lit only by the cave's own glow takes the first lamp's ring.
    #[inline]
    fn floor_level(&self, rings: &[u16], wx: i32, wy: i32) -> u8 {
        let Some(i) = self.index(wx.div_euclid(CELL), wy.div_euclid(CELL)) else {
            return 0;
        };
        if !self.lit[i] {
            return 0;
        }
        let bits = self.los[i];
        if bits == 0 {
            return self.lamps.first().map_or(0, |lamp| lamp_level(rings, lamp, wx, wy));
        }
        let mut best = 0;
        for (n, lamp) in self.lamps.iter().enumerate() {
            if bits & (1 << n) != 0 {
                best = best.max(lamp_level(rings, lamp, wx, wy));
            }
        }
        best
    }

    /// The light on rock: every lamp counts, seen or not, so a wall glows
    /// from the cavern behind it.
    #[inline]
    fn rock_level(&self, rings: &[u16], wx: i32, wy: i32) -> u8 {
        let mut best = 0;
        for lamp in &self.lamps {
            best = best.max(lamp_level(rings, lamp, wx, wy));
        }
        best
    }
}

/// Cast every lamp's line of sight over the cells in and round the view at
/// `cam`. Fire, lava and fungus glow a few cells of their own: they show the
/// ground without brightening it. The first lamp is the seat's own knight.
pub(crate) fn light_field(cave: &Cave, lamps: &[Lamp], cam: (i32, i32)) -> Light {
    let origin = (cam.0.div_euclid(CELL) - 1, cam.1.div_euclid(CELL) - 1);
    let cols = NATIVE_W / CELL + 3;
    let rows = NATIVE_H / CELL + 3;
    let mut light = Light {
        lamps: lamps.iter().take(LAMPS).copied().collect(),
        origin,
        cols,
        rows,
        los: vec![0; (cols * rows) as usize],
        lit: vec![false; (cols * rows) as usize],
    };
    // (cell, reach in cells, the lamp's bit)
    let mut sources: Vec<((i32, i32), i32, u8)> = light
        .lamps
        .iter()
        .enumerate()
        .map(|(n, lamp)| (lamp.cell, if lamp.small { SMALL_CELLS } else { KNIGHT_CELLS }, 1u8 << n))
        .collect();
    for y in (origin.1 - 8).max(0)..(origin.1 + rows + 8).min(MAP_H) {
        for x in (origin.0 - 8).max(0)..(origin.0 + cols + 8).min(MAP_W) {
            let Some(c) = cave.at(x, y) else { continue };
            if c.fire > 0 {
                sources.push(((x, y), 6, 0));
            } else if c.fluid == FLUID_LAVA && c.amount > 0 {
                sources.push(((x, y), 7, 0));
            } else if c.fungus() {
                sources.push(((x, y), 5, 0));
            }
        }
    }
    for (from, reach, bit) in sources {
        let y0 = (from.1 - reach).max(origin.1);
        let y1 = (from.1 + reach).min(origin.1 + rows - 1);
        let x0 = (from.0 - reach).max(origin.0);
        let x1 = (from.0 + reach).min(origin.0 + cols - 1);
        for wy in y0..=y1 {
            for wx in x0..=x1 {
                let (dx, dy) = (wx - from.0, wy - from.1);
                if dx * dx + dy * dy > reach * reach || blocked(cave, from, (wx, wy)) {
                    continue;
                }
                let i = ((wy - origin.1) * cols + (wx - origin.0)) as usize;
                light.lit[i] = true;
                light.los[i] |= bit;
            }
        }
    }
    light
}

fn blocked(cave: &Cave, from: (i32, i32), to: (i32, i32)) -> bool {
    let (mut x, mut y) = from;
    let (x1, y1) = to;
    let dx = (x1 - x).abs();
    let sx = (x1 - x).signum();
    let dy = -(y1 - y).abs();
    let sy = (y1 - y).signum();
    let mut err = dx + dy;
    loop {
        if (x, y) != from && (x, y) != to && cave.at(x, y).is_some_and(|c| c.solid()) {
            return true;
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// What the party has seen
// ---------------------------------------------------------------------------

/// How far into the rock a seen cavern shows: a band of wall this many cells
/// deep beyond the floor the light reached, and black past it.
const ROCK_BAND: u8 = 8;

struct Seen {
    seed: u32,
    tick: u32,
    /// 0 unseen. 1 a floor cell the light reached. 2 a wall the light reached
    /// or one touching such a floor; then one more for each cell deeper.
    ring: Vec<u8>,
}

thread_local! {
    static SEEN: RefCell<Seen> = const { RefCell::new(Seen { seed: 0, tick: 0, ring: Vec::new() }) };
}

/// Keep every cell the light reaches seen after the knights walk on, with a
/// band of the rock round it, and mark those cells on this frame's `cave`.
/// The memory is this draw thread's, for one seed; a tick that runs backwards
/// is a new game and starts it again.
pub(crate) fn remember(cave: &mut Cave, light: &Light, seed: u32, tick: u32) {
    SEEN.with(|slot| {
        let mut seen = slot.borrow_mut();
        let n = (MAP_W * MAP_H) as usize;
        if seen.seed != seed || seen.ring.len() != n || tick < seen.tick {
            seen.seed = seed;
            seen.ring = vec![0; n];
        }
        seen.tick = tick;
        let mut fresh: Vec<(i32, i32)> = Vec::new();
        for sy in 0..light.rows {
            for sx in 0..light.cols {
                if !light.lit[(sy * light.cols + sx) as usize] {
                    continue;
                }
                let (x, y) = (light.origin.0 + sx, light.origin.1 + sy);
                let Some(cell) = cave.at(x, y) else { continue };
                let ring = if cell.wall() { 2 } else { 1 };
                let i = (y * MAP_W + x) as usize;
                if seen.ring[i] == 0 || seen.ring[i] > ring {
                    seen.ring[i] = ring;
                    fresh.push((x, y));
                }
            }
        }
        while let Some((x, y)) = fresh.pop() {
            let ring = seen.ring[(y * MAP_W + x) as usize];
            if ring >= ROCK_BAND {
                continue;
            }
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if !cave.at(nx, ny).is_some_and(|c| c.wall()) {
                        continue;
                    }
                    let i = (ny * MAP_W + nx) as usize;
                    if seen.ring[i] == 0 || seen.ring[i] > ring + 1 {
                        seen.ring[i] = ring + 1;
                        fresh.push((nx, ny));
                    }
                }
            }
        }
        for y in (light.origin.1 - 6).max(0)..(light.origin.1 + light.rows + 2).min(MAP_H) {
            for x in (light.origin.0 - 2).max(0)..(light.origin.0 + light.cols + 2).min(MAP_W) {
                if seen.ring[(y * MAP_W + x) as usize] != 0 {
                    if let Some(c) = cave.at_mut(x, y) {
                        c.flags |= FL_SEEN;
                    }
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// The plate
// ---------------------------------------------------------------------------

const KIND_NONE: u8 = 0;
const KIND_FLOOR: u8 = 1;
const KIND_ROCK: u8 = 2;
const KIND_FACE: u8 = 3;
const KIND_CONTACT: u8 = 4;
const KIND_GLOW: u8 = 5;
const KIND_SHADOW: u8 = 6;
const KIND_ORE: u8 = 7;
const KIND_VEIN_FACE: u8 = 8;

/// A cell that changes shape can change the picture of the cells one to
/// either side, one above (a rounded corner, an ember) and five below (its
/// face and shadow fall that far, and frost drifts a little farther).
const LEAN_UP: i32 = 1;
const LEAN_DOWN: i32 = 5;

struct Plate {
    seed: u32,
    /// Per world pixel: the ink, and what kind of thing it is.
    ink: Vec<u8>,
    kind: Vec<u8>,
    /// Per world pixel: a rock top's speck (0 body, 1 dark, 2 mid, 3 pale),
    /// or how many pixels above a face or shadow pixel its wall's lip is.
    aux: Vec<u8>,
    /// Per world pixel, for the pixels whose neighbours are all in their own
    /// cell: `EDGE_KNOWN`, with `EDGE_LIP` on rock beside open ground and
    /// `EDGE_RIM` on floor against rock. A pixel on a cell's border asks its
    /// neighbours each frame instead, since they are drawn on their own.
    edge: Vec<u8>,
    /// Per cell: what its picture and its neighbours' depend on.
    shape: Vec<u32>,
    /// Per cell: what only its own picture depends on (seen, a fluid's ripple).
    look: Vec<u32>,
    stale: Vec<bool>,
}

thread_local! {
    static PLATE: RefCell<Plate> = const {
        RefCell::new(Plate {
            seed: 0,
            ink: Vec::new(),
            kind: Vec::new(),
            aux: Vec::new(),
            edge: Vec::new(),
            shape: Vec::new(),
            look: Vec::new(),
            stale: Vec::new(),
        })
    };
}

fn cell_shape(cell: &Cell) -> u32 {
    u32::from(cell.flags & !FL_SEEN)
        | (u32::from(cell.fluid) << 16)
        | (u32::from(cell.amount > 0) << 24)
        | (u32::from(cell.fire > 0) << 25)
        | (u32::from(cell.steam > 0) << 26)
}

fn cell_look(cell: &Cell, tick: u32) -> u32 {
    let ripple = if cell.fire > 0 || cell.fluid != 0 || cell.steam > 0 { tick / 4 } else { 0 };
    u32::from(cell.seen()) | (ripple << 1)
}

/// Bring the plate up to date for the cells `x0..=x1` by `y0..=y1`.
fn sync_plate(plate: &mut Plate, cave: &Cave, (x0, y0, x1, y1): (i32, i32, i32, i32), tick: u32) {
    let cells = (MAP_W * MAP_H) as usize;
    let pixels = (PLATE_W * PLATE_H) as usize;
    if plate.seed != cave.seed || plate.ink.len() != pixels {
        plate.seed = cave.seed;
        plate.ink = vec![0; pixels];
        plate.kind = vec![KIND_NONE; pixels];
        plate.aux = vec![0; pixels];
        plate.edge = vec![0; pixels];
        plate.shape = vec![u32::MAX; cells];
        plate.look = vec![u32::MAX; cells];
        plate.stale = vec![true; cells];
    }
    for cy in (y0 - LEAN_DOWN).max(0)..=(y1 + LEAN_UP).min(MAP_H - 1) {
        for cx in (x0 - 1).max(0)..=(x1 + 1).min(MAP_W - 1) {
            let Some(cell) = cave.at(cx, cy) else { continue };
            let ci = (cy * MAP_W + cx) as usize;
            let shape = cell_shape(cell);
            if plate.shape[ci] != shape {
                plate.shape[ci] = shape;
                for ny in (cy - LEAN_UP).max(0)..=(cy + LEAN_DOWN).min(MAP_H - 1) {
                    for nx in (cx - 1).max(0)..=(cx + 1).min(MAP_W - 1) {
                        plate.stale[(ny * MAP_W + nx) as usize] = true;
                    }
                }
            }
            let look = cell_look(cell, tick);
            if plate.look[ci] != look {
                plate.look[ci] = look;
                plate.stale[ci] = true;
            }
        }
    }
    let mut memo = false;
    for cy in y0.max(0)..=y1.min(MAP_H - 1) {
        for cx in x0.max(0)..=x1.min(MAP_W - 1) {
            let ci = (cy * MAP_W + cx) as usize;
            if !plate.stale[ci] {
                continue;
            }
            plate.stale[ci] = false;
            let Some(cell) = cave.at(cx, cy) else { continue };
            if !memo {
                begin_solid((x0 * CELL - 40, y0 * CELL - 40), ((x1 - x0 + 1) * CELL + 80, (y1 - y0 + 1) * CELL + 80));
                memo = true;
            }
            draw_cell(plate, cave, cx, cy, cell, tick);
        }
    }
    if memo {
        end_solid();
        let bake = std::time::Instant::now();
        stamp_veins(plate, cave, x0.max(0), x1.min(MAP_W - 1), y0.max(0), y1.min(MAP_H - 1));
        note_bake(bake.elapsed().as_micros() as u64);
    }
}

fn draw_cell(plate: &mut Plate, cave: &Cave, cx: i32, cy: i32, cell: &Cell, tick: u32) {
    for wy in cy * CELL..(cy + 1) * CELL {
        for wx in cx * CELL..(cx + 1) * CELL {
            let i = (wy * PLATE_W + wx) as usize;
            if !cell.seen() {
                plate.ink[i] = 0;
                plate.kind[i] = KIND_NONE;
                continue;
            }
            let solid = solid_pixel(cave, wx, wy);
            let ch = pixel_ink(cave, cx, cy, wx, wy, cell, solid, tick);
            let mut aux = 0;
            let kind = if glow_ink(ch, cell, solid) {
                KIND_GLOW
            } else if solid {
                aux = speck_role(ch, wx, wy);
                KIND_ROCK
            } else if let Some((drop, _)) = face_drop(cave, wx, wy) {
                aux = drop as u8;
                if drop + 1 == FACE_PX {
                    KIND_CONTACT
                } else if drop < FACE_PX {
                    KIND_FACE
                } else {
                    KIND_SHADOW
                }
            } else {
                KIND_FLOOR
            };
            plate.ink[i] = ch as u8;
            plate.kind[i] = kind;
            plate.aux[i] = aux;
        }
    }
    for wy in cy * CELL..(cy + 1) * CELL {
        for wx in cx * CELL..(cx + 1) * CELL {
            let i = (wy * PLATE_W + wx) as usize;
            let inner = (1..CELL - 1).contains(&(wx - cx * CELL)) && (1..CELL - 1).contains(&(wy - cy * CELL));
            plate.edge[i] = if !inner || !cell.seen() {
                0
            } else {
                EDGE_KNOWN
                    | match plate.kind[i] {
                        KIND_ROCK if rock_lip(&plate.kind, wx, wy) => EDGE_LIP,
                        KIND_FLOOR if floor_rim(&plate.kind, wx, wy) => EDGE_RIM,
                        _ => 0,
                    }
            };
        }
    }
}

const EDGE_KNOWN: u8 = 0x80;
const EDGE_LIP: u8 = 1;
const EDGE_RIM: u8 = 2;

/// The plate's kind at a world pixel; nothing outside the cave.
#[inline]
fn kind_at(kind: &[u8], wx: i32, wy: i32) -> u8 {
    if wx < 0 || wy < 0 || wx >= PLATE_W || wy >= PLATE_H {
        KIND_NONE
    } else {
        kind[(wy * PLATE_W + wx) as usize]
    }
}

const ROUND: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// A rock pixel beside open, seen ground: the wall's lip.
fn rock_lip(kind: &[u8], wx: i32, wy: i32) -> bool {
    ROUND.iter().any(|(dx, dy)| {
        let k = kind_at(kind, wx + dx, wy + dy);
        k != KIND_ROCK && k != KIND_ORE && k != KIND_NONE
    })
}

/// A pixel beside ground the party has not seen.
fn unseen_edge(kind: &[u8], wx: i32, wy: i32) -> bool {
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < PLATE_W && y < PLATE_H;
    ROUND
        .iter()
        .any(|(dx, dy)| inside(wx + dx, wy + dy) && kind_at(kind, wx + dx, wy + dy) == KIND_NONE)
}

/// A floor pixel against rock or a wall's face.
fn floor_rim(kind: &[u8], wx: i32, wy: i32) -> bool {
    [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
        let k = kind_at(kind, wx + dx, wy + dy);
        k == KIND_ROCK || k == KIND_FACE || k == KIND_CONTACT || k == KIND_ORE || k == KIND_VEIN_FACE
    })
}

/// A rock or face pixel with open floor on the side nearer `lamp`.
fn faces_lamp(kind: &[u8], wx: i32, wy: i32, lamp: (i32, i32)) -> bool {
    let here = (wx - lamp.0) * (wx - lamp.0) + (wy - lamp.1) * (wy - lamp.1);
    [(0, 1), (0, -1), (1, 0), (-1, 0)].iter().any(|(dx, dy)| {
        let (x, y) = (wx + dx, wy + dy);
        let k = kind_at(kind, x, y);
        (k == KIND_FLOOR || k == KIND_GLOW)
            && (x - lamp.0) * (x - lamp.0) + (y - lamp.1) * (y - lamp.1) < here
    })
}

// ---------------------------------------------------------------------------
// The light pass
// ---------------------------------------------------------------------------

struct InkSteps {
    down: [[[u8; 3]; 7]; 128],
    warm: [[[u8; 3]; 7]; 128],
}

fn ink_steps() -> &'static InkSteps {
    use std::sync::OnceLock;
    static TABLE: OnceLock<InkSteps> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut down = [[[0u8; 3]; 7]; 128];
        let mut warm = [[[0u8; 3]; 7]; 128];
        for ch in 0..128u8 {
            let Some(rgb) = ink::ink(ch as char) else { continue };
            let mut cur = rgb;
            for step in 0..7 {
                down[ch as usize][step] = cur;
                warm[ch as usize][step] = [
                    cur[0].saturating_add(6),
                    cur[1],
                    cur[2].saturating_sub(6),
                ];
                cur = light::step_down(cur, 1);
            }
        }
        InkSteps { down, warm }
    })
}

fn step_rgb(ch: char, steps: usize, warm: bool) -> ink::Rgb {
    let table = ink_steps();
    let i = (ch as usize).min(127);
    let mut n = steps.min(6);
    loop {
        let c = if warm { table.warm[i][n] } else { table.down[i][n] };
        if c != [0, 0, 0] || n == 0 {
            return if c == [0, 0, 0] { [11, 10, 8] } else { c };
        }
        n -= 1;
    }
}

fn lum(c: ink::Rgb) -> u32 {
    c[0] as u32 * 3 + c[1] as u32 * 6 + c[2] as u32
}

fn floor_steps(level: u8) -> usize {
    match level {
        8 | 7 => 0,
        6 | 5 => 1,
        4 | 3 => 2,
        _ => 3,
    }
}

fn sunk(c: ink::Rgb, n: usize) -> ink::Rgb {
    let d = light::step_down(c, n);
    if d == [0, 0, 0] { [11, 10, 8] } else { d }
}

/// A floor ink at a light level. Signal inks and fireflies keep their own
/// colour; the rest step down the ink ramp, warmed in the outer bands.
fn shade(ch: char, level: u8, tint: u8) -> ink::Rgb {
    let Some(base) = ink::ink(ch) else {
        return [0, 0, 0];
    };
    if level == 0 {
        return [0, 0, 0];
    }
    if ink::is_signal(base) || ch == 'Y' {
        return base;
    }
    let (steps, warm) = match level {
        8 | 7 => (0, false),
        6 | 5 => (1, tint != 0),
        4 | 3 => (2, true),
        _ => (3, false),
    };
    step_rgb(ch, steps, warm)
}

/// A floor ink under the light, worked out once for every ink, level and
/// warmth: `[sink][rim]` is the tone sunk that many steps (a dip in the
/// floor), then one more where the floor meets rock.
struct FloorTones {
    tone: Vec<[[ink::Rgb; 2]; 3]>,
}

fn floor_tones() -> &'static FloorTones {
    static TONES: OnceLock<FloorTones> = OnceLock::new();
    TONES.get_or_init(|| {
        let mut tone = Vec::with_capacity(128 * 9 * 2);
        for ch in 0..128u8 {
            for level in 0..9u8 {
                for tint in 0..2u8 {
                    let flat = shade(ch as char, level, tint);
                    let dips = [flat, sunk(flat, 1), sunk(flat, 2)];
                    tone.push(dips.map(|c| [c, sunk(c, 1)]));
                }
            }
        }
        FloorTones { tone }
    })
}

#[inline]
fn floor_tone(tones: &FloorTones, ch: u8, level: u8, warm: bool, sink: usize, rim: bool) -> ink::Rgb {
    let i = (usize::from(ch.min(127)) * 9 + usize::from(level.min(8))) * 2 + usize::from(warm);
    tones.tone[i][sink][usize::from(rim)]
}

fn beside_floor(cave: &Cave, cx: i32, cy: i32) -> Option<(i32, i32)> {
    let ox = cx * CELL + 4;
    let oy = cy * CELL + 4;
    for dist in 1..=8 {
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            let x = ox + dx * dist;
            let y = oy + dy * dist;
            let open = cave
                .at(x.div_euclid(CELL), y.div_euclid(CELL))
                .is_some_and(|c| !c.solid());
            if open {
                return Some((x, y));
            }
        }
    }
    None
}

fn glow_ink(ch: char, cell: &Cell, solid: bool) -> bool {
    // A pool is lit by the lamp like the floor, except lava's melt: molten rock makes its
    // own light in a dark cave, while its crust plates take the lamp like stone.
    if !pools_flat()
        && !solid
        && cell.amount > 0
        && matches!(cell.fluid, FLUID_LAVA | FLUID_WATER | FLUID_ACID)
    {
        return cell.fluid == FLUID_LAVA && matches!(ch, '4' | '5' | '7');
    }
    (ch == 'Y' && cell.fluid == FLUID_ACID)
        || ch == '@'
        || ch == '5'
        || ch == '6'
        || ch == '7'
        || ch == '8'
        || (!solid && cell.fire > 0)
        || (!solid && cell.fluid == FLUID_LAVA && cell.amount > 0)
}

/// role 0 body, 1 dark chip, 2 mid chip, 3 cap, 4 face, 5 contact, 6 shadow.
/// band 0 darkest explored, 1 dim, 2 mid, 3 full torch. Grey, not navy.
const MINE_STONE: [[[u8; 3]; 4]; 7] = [
    [[26, 25, 24], [28, 27, 26], [34, 32, 31], [46, 44, 43]],
    [[12, 11, 10], [14, 13, 12], [18, 17, 16], [24, 22, 20]],
    [[16, 14, 12], [20, 18, 16], [26, 23, 21], [36, 32, 28]],
    [[34, 32, 28], [42, 40, 38], [46, 44, 42], [56, 54, 52]],
    [[18, 17, 16], [20, 19, 18], [24, 23, 22], [32, 30, 29]],
    [[12, 11, 10], [14, 13, 12], [16, 15, 14], [22, 21, 20]],
    [[10, 9, 8], [10, 9, 8], [12, 11, 10], [16, 15, 14]],
];
const FOUNDRY_STONE: [[[u8; 3]; 4]; 7] = [
    [[26, 21, 18], [26, 22, 19], [32, 27, 24], [50, 42, 38]],
    [[12, 9, 7], [14, 11, 9], [18, 14, 12], [28, 22, 18]],
    [[18, 13, 10], [20, 16, 13], [24, 19, 16], [40, 32, 28]],
    [[40, 32, 24], [42, 34, 28], [46, 38, 32], [60, 52, 46]],
    [[16, 13, 11], [18, 15, 13], [22, 18, 15], [34, 28, 24]],
    [[10, 8, 7], [12, 10, 8], [16, 13, 11], [22, 18, 15]],
    [[8, 7, 6], [8, 7, 6], [12, 10, 8], [16, 13, 11]],
];
const FROST_STONE: [[[u8; 3]; 4]; 7] = [
    [[22, 24, 28], [24, 26, 30], [32, 34, 38], [44, 46, 50]],
    [[10, 12, 16], [12, 14, 18], [18, 20, 24], [26, 28, 34]],
    [[14, 16, 20], [18, 20, 24], [24, 26, 30], [34, 36, 42]],
    [[34, 36, 40], [38, 42, 48], [42, 46, 52], [54, 56, 60]],
    [[14, 16, 20], [16, 18, 22], [22, 24, 28], [30, 32, 36]],
    [[10, 11, 14], [12, 13, 16], [14, 16, 20], [20, 22, 26]],
    [[8, 9, 12], [8, 9, 12], [10, 12, 16], [14, 16, 20]],
];
const HOLLOW_STONE: [[[u8; 3]; 4]; 7] = [
    [[26, 28, 24], [24, 26, 22], [26, 30, 26], [32, 36, 30]],
    [[12, 14, 12], [12, 14, 12], [16, 18, 15], [20, 22, 18]],
    [[18, 16, 14], [18, 20, 16], [20, 24, 20], [26, 30, 24]],
    [[36, 34, 28], [40, 38, 32], [42, 40, 34], [48, 46, 40]],
    [[14, 16, 14], [16, 18, 15], [18, 20, 17], [22, 24, 21]],
    [[10, 12, 10], [10, 12, 10], [12, 14, 12], [16, 18, 15]],
    [[8, 10, 8], [8, 10, 8], [10, 12, 10], [12, 14, 12]],
];

fn light_band(level: u8) -> usize {
    match level {
        7 | 8 => 3,
        5 | 6 => 2,
        3 | 4 => 1,
        _ => 0,
    }
}

fn stone_rgb(region: u8, role: u8, level: u8) -> ink::Rgb {
    let band = light_band(level);
    let role = role.min(6) as usize;
    let table: &[[[u8; 3]; 4]; 7] = match region {
        1 => &HOLLOW_STONE,
        2 => &FOUNDRY_STONE,
        3 => &FROST_STONE,
        _ => &MINE_STONE,
    };
    table[role][band]
}

fn floor_beside(region: u8, level: u8) -> ink::Rgb {
    let floor_ch = match region {
        1 => 'E',
        3 => 'i',
        _ => 'P',
    };
    step_rgb(floor_ch, floor_steps(level), false)
}

fn rock_cap(region: u8, level: u8) -> ink::Rgb {
    let cap = stone_rgb(region, 3, level);
    let floor = floor_beside(region, level);
    if lum(cap) <= lum(floor) {
        cap
    } else {
        match region {
            1 => [44, 42, 36],
            2 => [52, 40, 30],
            3 => [46, 50, 56],
            _ => [48, 46, 40],
        }
    }
}

/// A rock top's speck: the ink's own where it names one, else scattered.
fn speck_role(ch: char, wx: i32, wy: i32) -> u8 {
    match ch {
        'K' => 1,
        'X' => 2,
        LOOSE_STONE => 0,
        LOOSE_LIGHT => 3,
        _ => {
            let h = hash(wx, wy, 17);
            if h % 4 == 0 {
                1
            } else if h % 6 == 0 {
                2
            } else {
                0
            }
        }
    }
}

/// Rock dimmed to the light on the floor beside it.
fn rock_rgb(ch: char, region: u8, level: u8) -> ink::Rgb {
    let role = match ch {
        'K' => 1,
        'X' => 2,
        _ => 0,
    };
    let mut rgb = stone_rgb(region, role, level);
    let floor = floor_beside(region, level);
    if lum(rgb) > lum(floor) {
        rgb = stone_rgb(region, role, 0);
    }
    if lum(rgb) > lum(floor) {
        rgb = stone_rgb(region, 1, 0);
    }
    rgb
}

fn fly_rgb(level: u8) -> ink::Rgb {
    match light_band(level) {
        3 => [63, 100, 40],
        2 => [51, 80, 34],
        1 => [32, 42, 22],
        _ => [28, 40, 22],
    }
}

/// Which lit thing warms the floor near a cell: 1 fire, 2 lava, 3 fungus.
fn warm_cells(cave: &Cave, light: &Light) -> Vec<bool> {
    let mut warm = vec![false; (light.cols * light.rows) as usize];
    let mut glows = Vec::new();
    for y in (light.origin.1 - 3).max(0)..(light.origin.1 + light.rows + 3).min(MAP_H) {
        for x in (light.origin.0 - 3).max(0)..(light.origin.0 + light.cols + 3).min(MAP_W) {
            let Some(c) = cave.at(x, y) else { continue };
            if c.fire > 0 || (c.fluid == FLUID_LAVA && c.amount > 0) || c.fungus() {
                glows.push((x, y));
            }
        }
    }
    for (x, y) in glows {
        for cy in y - 3..=y + 3 {
            for cx in x - 3..=x + 3 {
                if let Some(i) = light.index(cx, cy) {
                    warm[i] = true;
                }
            }
        }
    }
    warm
}

/// The cave's ground for the view at `cam`, lit by `light`. `cave` carries
/// what the party has seen (`remember`).
pub(crate) fn paint(
    cave: &Cave,
    cam: (i32, i32),
    light: &Light,
    tick: u32,
    heads: &[MineHead],
    sunk: u8,
) -> Img {
    settle_glow(cave, light, tick);
    let fade = glow_window(light);
    let rings = rings();
    let tones = floor_tones();
    let cells = (
        cam.0.div_euclid(CELL) - 1,
        cam.1.div_euclid(CELL) - 1,
        (cam.0 + NATIVE_W - 1).div_euclid(CELL) + 1,
        (cam.1 + NATIVE_H - 1).div_euclid(CELL) + 1,
    );
    let mut words = vec![0u32; (NATIVE_W * NATIVE_H) as usize];
    let own = light.lamps.first().map_or((0, 0), |lamp| lamp.px);
    let glint = std::time::Instant::now();
    let blinks = blink_cells(cave, sunk, tick);
    note_glint(glint.elapsed().as_micros() as u64);
    // Per cell of the light's window: the region, and for rock the light on
    // the open floor beside it (255 where there is none within a cell).
    let n = (light.cols * light.rows) as usize;
    let mut region = vec![0u8; n];
    let mut beside = vec![255u8; n];
    for sy in 0..light.rows {
        for sx in 0..light.cols {
            let (cx, cy) = (light.origin.0 + sx, light.origin.1 + sy);
            let i = (sy * light.cols + sx) as usize;
            region[i] = region_of(cx, cy).0;
            if cave.at(cx, cy).is_some_and(|c| c.solid()) {
                if let Some((fx, fy)) = beside_floor(cave, cx, cy) {
                    beside[i] = apply_fade(light.floor_level(rings, fx, fy), &fade, light, fx, fy);
                }
            }
        }
    }
    let warm = warm_cells(cave, light);
    PLATE.with(|slot| {
        let mut plate = slot.borrow_mut();
        sync_plate(&mut plate, cave, cells, tick);
        let kinds = &plate.kind;
        for py in 0..NATIVE_H {
            let wy = cam.1 + py;
            let cy = wy.div_euclid(CELL);
            for px in 0..NATIVE_W {
                let wx = cam.0 + px;
                let i = (wy * PLATE_W + wx) as usize;
                let kind = kinds[i];
                if kind == KIND_NONE {
                    continue;
                }
                let ch = plate.ink[i];
                let edge = plate.edge[i];
                let inner = edge & EDGE_KNOWN != 0;
                let cx = wx.div_euclid(CELL);
                let ci = light.index(cx, cy).unwrap_or(0);
                let region = region[ci];
                let rgb = match kind {
                    KIND_GLOW => step_rgb(ch as char, 0, false),
                    KIND_ROCK => {
                        let lip = if inner { edge & EDGE_LIP != 0 } else { rock_lip(kinds, wx, wy) };
                        let mut level = light.rock_level(rings, wx, wy);
                        if level >= 4 && lip && faces_lamp(kinds, wx, wy, own) {
                            level = (level + 1).min(8);
                        }
                        let lip = lip || (!inner && unseen_edge(kinds, wx, wy));
                        let role = plate.aux[i];
                        let mut rgb = if lip {
                            rock_cap(region, level)
                        } else {
                            let mut rgb = stone_rgb(region, role, level);
                            let floor = floor_beside(region, level);
                            if lum(rgb) > lum(floor) {
                                rgb = stone_rgb(region, role, 0);
                            }
                            if lum(rgb) > lum(floor) {
                                rgb = stone_rgb(region, 1, 0);
                            }
                            rgb
                        };
                        // Rock is never brighter than the floor it stands in.
                        let beside = beside[ci];
                        if beside != 255 {
                            let floor = floor_beside(region, beside);
                            if lum(rgb) > lum(floor) {
                                rgb = if lip {
                                    stone_rgb(region, 0, beside)
                                } else {
                                    rock_rgb(ch as char, region, beside)
                                };
                                if lum(rgb) > lum(floor) {
                                    rgb = stone_rgb(region, 1, 0);
                                }
                            }
                        }
                        rgb
                    }
                    KIND_FACE | KIND_CONTACT | KIND_SHADOW => {
                        // A face takes its wall's light, a column at a time:
                        // the light at the lip straight above this pixel.
                        let lip_y = if kind == KIND_CONTACT { wy } else { wy - i32::from(plate.aux[i]) - 1 };
                        let mut level = light.rock_level(rings, wx, lip_y);
                        if kind == KIND_FACE && level >= 4 && faces_lamp(kinds, wx, wy, own) {
                            level = (level + 1).min(8);
                        }
                        if kind == KIND_FACE && kind_at(kinds, wx, wy - 1) == KIND_ROCK {
                            rock_cap(region, level)
                        } else {
                            let role = match (kind, ch as char) {
                                (KIND_FACE, LOOSE_FACE) => 2,
                                (KIND_FACE, LOOSE_SEAM) => 1,
                                (KIND_FACE, _) => 4,
                                (KIND_CONTACT, _) => 5,
                                _ => 6,
                            };
                            let rgb = stone_rgb(region, role, level);
                            if lum(rgb) > lum(floor_beside(region, level)) {
                                stone_rgb(region, 6, level)
                            } else {
                                rgb
                            }
                        }
                    }
                    KIND_ORE | KIND_VEIN_FACE => {
                        let (level, vein) = if kind == KIND_ORE {
                            let lip = if inner { edge & EDGE_LIP != 0 } else { rock_lip(kinds, wx, wy) };
                            let mut level = light.rock_level(rings, wx, wy);
                            if level >= 4 && lip && faces_lamp(kinds, wx, wy, own) {
                                level = (level + 1).min(8);
                            }
                            (level, (cx, cy))
                        } else {
                            let drop = plate.aux[i] & 31;
                            let lip_y = wy - i32::from(drop) - 1;
                            let mut level = light.rock_level(rings, wx, lip_y);
                            if level >= 4 && faces_lamp(kinds, wx, wy, own) {
                                level = (level + 1).min(8);
                            }
                            (level, (wx.div_euclid(CELL), lip_y.div_euclid(CELL)))
                        };
                        let blink = ore_blinks(kind, plate.aux[i]) && blinks.iter().any(|c| *c == vein);
                        ore_rgb(ch, level, region, beside[ci], blink)
                    }
                    _ => {
                        let level = apply_fade(light.floor_level(rings, wx, wy), &fade, light, wx, wy);
                        let lit = if level == 0 { 2 } else { level };
                        if region == 1 && ch == b'Y' {
                            fly_rgb(lit)
                        } else if !inner && unseen_edge(kinds, wx, wy) {
                            UNSEEN_EDGE
                        } else {
                            let rim = if inner { edge & EDGE_RIM != 0 } else { floor_rim(kinds, wx, wy) };
                            // A dip in the mine's floor, a paler vein, a foundry joint.
                            let (ch, sink) = match (region, ch) {
                                (0, b'p') => (b'P', 2),
                                (0, b'r') => (b'o', 0),
                                (2, b'I') => (b'P', 1),
                                _ => (ch, 0),
                            };
                            floor_tone(tones, ch, lit, warm[ci], sink, rim)
                        }
                    }
                };
                words[(py * NATIVE_W + px) as usize] =
                    0xff00_0000 | (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]);
            }
        }
    });
    let mut im = Img::from_words(NATIVE_W, NATIVE_H, words);
    paint_cracks(&mut im, cave, cam, cells);
    stamp_adits(&mut im, cave, cam, light, heads);
    im
}

/// Where the floor meets ground nobody has seen: a dark warm edge.
const UNSEEN_EDGE: ink::Rgb = [36, 14, 6];

/// Draw the plate and the lamp's rings for the view a knight will arrive in,
/// before they arrive. Draw-thread work, never a simulation tick's.
pub(crate) fn warm(cave: &mut Cave, lamps: &[Lamp], cam: (i32, i32), seed: u32, tick: u32) {
    let light = light_field(cave, lamps, cam);
    remember(cave, &light, seed, tick);
    let _ = paint(cave, cam, &light, tick, &[], 0);
}

// ---------------------------------------------------------------------------
// A cell's light fades in and out over six ticks, so a corner does not pop
// when the lamp's line of sight changes cell. A cell seen for the first time
// snaps, so the first frame inside is not a fade from black.
// ---------------------------------------------------------------------------

const GLOW_FULL: u8 = 6;

struct Glow {
    seed: u32,
    tick: u32,
    level: Vec<u8>,
    /// 0 unseen, 1 was dark, 2 was lit.
    seen: Vec<u8>,
}

thread_local! {
    static GLOW: RefCell<Glow> = RefCell::new(Glow {
        seed: 0,
        tick: 0,
        level: Vec::new(),
        seen: Vec::new(),
    });
}

fn settle_glow(cave: &Cave, light: &Light, tick: u32) {
    GLOW.with(|slot| {
        let mut glow = slot.borrow_mut();
        let n = (MAP_W * MAP_H) as usize;
        if glow.seed != cave.seed || glow.level.len() != n || tick < glow.tick {
            glow.seed = cave.seed;
            glow.tick = tick;
            glow.level = vec![0; n];
            glow.seen = vec![0; n];
        }
        let step = if tick == glow.tick { 0 } else { (tick - glow.tick).min(6) as u8 };
        glow.tick = tick;
        for sy in 0..light.rows {
            for sx in 0..light.cols {
                let cx = light.origin.0 + sx;
                let cy = light.origin.1 + sy;
                if cx < 0 || cy < 0 || cx >= MAP_W || cy >= MAP_H {
                    continue;
                }
                let mi = (cy * MAP_W + cx) as usize;
                let i = (sy * light.cols + sx) as usize;
                let lit = light.lit[i] && light.los[i] != 0;
                let target = if lit { 2 } else { 1 };
                if glow.seen[mi] == 0 {
                    glow.seen[mi] = target;
                    glow.level[mi] = if lit { GLOW_FULL } else { 0 };
                    continue;
                }
                if glow.seen[mi] == target {
                    glow.level[mi] = if lit { GLOW_FULL } else { 0 };
                    continue;
                }
                if step == 0 {
                    continue;
                }
                if lit {
                    glow.level[mi] = glow.level[mi].saturating_add(step).min(GLOW_FULL);
                    if glow.level[mi] == GLOW_FULL {
                        glow.seen[mi] = 2;
                    }
                } else {
                    glow.level[mi] = glow.level[mi].saturating_sub(step);
                    if glow.level[mi] == 0 {
                        glow.seen[mi] = 1;
                    }
                }
            }
        }
    });
}

fn glow_window(light: &Light) -> Vec<u8> {
    GLOW.with(|slot| {
        let glow = slot.borrow();
        let mut out = vec![GLOW_FULL; (light.cols * light.rows) as usize];
        if glow.level.is_empty() {
            return out;
        }
        for sy in 0..light.rows {
            for sx in 0..light.cols {
                let cx = light.origin.0 + sx;
                let cy = light.origin.1 + sy;
                if cx < 0 || cy < 0 || cx >= MAP_W || cy >= MAP_H {
                    continue;
                }
                let mi = (cy * MAP_W + cx) as usize;
                out[(sy * light.cols + sx) as usize] = glow.level.get(mi).copied().unwrap_or(GLOW_FULL);
            }
        }
        out
    })
}

fn apply_fade(level: u8, fade: &[u8], light: &Light, wx: i32, wy: i32) -> u8 {
    if level == 0 {
        return 0;
    }
    let Some(i) = light.index(wx.div_euclid(CELL), wy.div_euclid(CELL)) else {
        return level;
    };
    let fade = fade.get(i).copied().unwrap_or(GLOW_FULL);
    if fade >= GLOW_FULL {
        return level;
    }
    (u16::from(level) * u16::from(fade) / u16::from(GLOW_FULL)) as u8
}

/// Darkest stone ink, and a lit edge that stays under the lamp's heart.
fn crack_inks(region: u8) -> (ink::Rgb, ink::Rgb) {
    let heart = floor_beside(region, 8);
    let split = stone_rgb(region, 6, 0);
    // A one-pixel lit edge: one step lighter than the darkest ink, never the
    // lamp's heart and never white.
    // Visible on dark stone at 1x, still under the lamp heart (about 169, 118, 57).
    let mut edge = [72, 54, 36];
    if lum(edge) >= lum(heart) {
        edge = [
            split[0].saturating_add(22),
            split[1].saturating_add(16),
            split[2].saturating_add(12),
        ];
    }
    if lum(edge) >= lum(heart) {
        edge = split;
    }
    (split, edge)
}

/// Screen rectangle the painter fills, in view pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl ScreenRect {
    pub(crate) fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// Where the painter draws one cave cell: its top, and the south face that
/// hangs from it. Both come from the painter's own numbers (`CELL`, `FACE_PX`,
/// the camera, and `TOP_LIFT`). A narrow pocket shortens the face to the lip
/// the painter actually fills. A cell with rock to its south has no face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CellDrawn {
    pub top: ScreenRect,
    pub face: Option<ScreenRect>,
}

pub(crate) fn cell_drawn(cave: &Cave, cell: (i32, i32), cam: (i32, i32)) -> CellDrawn {
    let (cx, cy) = cell;
    let top = ScreenRect {
        x: cx * CELL - cam.0,
        y: cy * CELL - cam.1 - TOP_LIFT,
        w: CELL,
        h: CELL,
    };
    let south_rock = cave.at(cx, cy + 1).is_some_and(|c| c.solid());
    let face = if south_rock {
        None
    } else {
        let h = if narrow_pocket(cave, cx, cy + 1) { POCKET_LIP } else { FACE_PX };
        Some(ScreenRect { x: top.x, y: top.y + top.h, w: CELL, h })
    };
    CellDrawn { top, face }
}

pub(crate) fn painted_stone(wx: i32, wy: i32) -> bool {
    PLATE.with(|slot| {
        let kind = kind_at(&slot.borrow().kind, wx, wy);
        kind == KIND_ROCK || kind == KIND_FACE || kind == KIND_CONTACT || kind == KIND_ORE || kind == KIND_VEIN_FACE
    })
}

/// A knight digging the cell to his north. A crack pixel is drawn only where
/// his sprite has no ink, on that cell's top or face, and only on painted
/// stone. After the first hit the same jag climbs the top of the wall north
/// of him; after the second, the wall north of that. The climb is one pixel
/// narrower, in the same crack inks.
pub(crate) fn paint_north_cracks(
    im: &mut Img,
    cave: &Cave,
    cam: (i32, i32),
    digs: &[(i32, i32)],
    covered: &dyn Fn(i32, i32) -> bool,
) {
    for &(cx, cy) in digs {
        let Some(cell) = cave.at(cx, cy) else { continue };
        if !cell.wall() || cell.swings == 0 || cell.swings >= 3 {
            continue;
        }
        let drawn = cell_drawn(cave, (cx, cy), cam);
        let deep = cell.swings == 1;
        let (split, edge) = crack_inks(region_of(cx, cy).0);
        let seed = hash(cx, cy, 77);
        let rows = if deep { CELL + FACE_PX - 1 } else { CELL + 12 };
        let on_dug = |sx: i32, sy: i32| {
            let on = drawn.top.contains(sx, sy)
                || drawn.face.is_some_and(|face| face.contains(sx, sy));
            on && !covered(sx, sy) && painted_stone(sx + cam.0, sy + cam.1)
        };
        for row in 0..rows {
            lay_crack(im, cx, cy, cam, row, seed, split, edge, deep, false, &on_dug);
        }
        let climb = if deep { 2 } else { 1 };
        for step in 1..=climb {
            let ncy = cy - step;
            let Some(north) = cave.at(cx, ncy) else { continue };
            if !north.wall() {
                continue;
            }
            let top = cell_drawn(cave, (cx, ncy), cam).top;
            for ly in 0..CELL {
                let wy = ncy * CELL + ly;
                let row = wy - cy * CELL;
                let on_top = |sx: i32, sy: i32| {
                    top.contains(sx, sy)
                        && !covered(sx, sy)
                        && painted_stone(sx + cam.0, sy + cam.1)
                };
                lay_crack(im, cx, cy, cam, row, seed, split, edge, deep, true, &on_top);
            }
        }
    }
}

fn lay_crack(
    im: &mut Img,
    cx: i32,
    cy: i32,
    cam: (i32, i32),
    row: i32,
    seed: u32,
    split: ink::Rgb,
    edge: ink::Rgb,
    deep: bool,
    narrow: bool,
    allow: &dyn Fn(i32, i32) -> bool,
) {
    let jag = CRACK_RUN[((row.rem_euclid(14) as u32 + seed) % 14) as usize];
    let x = cx * CELL + 2 + jag;
    let wy = cy * CELL + row;
    let sx = x - cam.0;
    let sy = wy - cam.1;
    let mut put = |dx: i32, dy: i32, rgb: ink::Rgb| {
        if allow(sx + dx, sy + dy) {
            im.set(sx + dx, sy + dy, rgb);
        }
    };
    // The climb drops the rightmost pixel of the dug crack, and grows no fork.
    if narrow {
        put(0, 0, split);
        if deep {
            put(-1, 0, split);
            put(1, 0, split);
            put(2, 0, edge);
        } else {
            put(1, 0, edge);
        }
        return;
    }
    put(0, 0, split);
    put(1, 0, split);
    put(2, 0, edge);
    if deep {
        put(-1, 0, split);
        put(3, 0, edge);
        if row % 8 == 3 {
            put(3, 0, split);
            put(4, 1, split);
            put(5, 1, edge);
        }
        if row % 8 == 6 {
            put(-2, 0, split);
            put(-3, 1, split);
            put(-4, 1, edge);
        }
    }
}

// ---------------------------------------------------------------------------
// Cracks and slides
// ---------------------------------------------------------------------------

/// How a crack runs down a wall: which of its columns it is in, row by row.
const CRACK_RUN: [i32; 14] = [0, 0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 1];

/// The cracks a shovel has left in loose rock that still stands: a thin one
/// after the first hit, a wide forked one after the second. They run down the
/// middle of the wall's top and on down its face, and only ever over that
/// wall's own stone.
fn paint_cracks(im: &mut Img, cave: &Cave, cam: (i32, i32), (x0, y0, x1, y1): (i32, i32, i32, i32)) {
    PLATE.with(|slot| {
        let plate = slot.borrow();
        for cy in (y0 - 3).max(0)..=y1.min(MAP_H - 1) {
            for cx in x0.max(0)..=x1.min(MAP_W - 1) {
                let Some(cell) = cave.at(cx, cy) else { continue };
                if !cell.wall() || !cell.seen() || cell.swings == 0 || cell.swings >= 3 {
                    continue;
                }
                let deep = cell.swings == 1;
                let (split, edge) = crack_inks(region_of(cx, cy).0);
                let seed = hash(cx, cy, 77);
                let drawn = cell_drawn(cave, (cx, cy), cam);
                let south = drawn.top.y + cam.1 + drawn.top.h;
                let mut mark = |wx: i32, wy: i32, rgb: ink::Rgb| {
                    let kind = kind_at(&plate.kind, wx, wy);
                    let own_top = wy < south && kind == KIND_ROCK;
                    if own_top {
                        im.set(wx - cam.0, wy - cam.1, rgb);
                    }
                };
                let rows = if deep { CELL + FACE_PX - 1 } else { CELL + 12 };
                for row in 1..rows {
                    let x = cx * CELL + 2 + CRACK_RUN[((row as u32 + seed) % 14) as usize];
                    let wy = cy * CELL + row;
                    // Two pixels of the stone's darkest ink, and a one-pixel lit edge.
                    mark(x, wy, split);
                    mark(x + 1, wy, split);
                    mark(x + 2, wy, edge);
                    if deep {
                        // The second hit is forked, and a pixel wider.
                        mark(x - 1, wy, split);
                        mark(x + 3, wy, edge);
                        if row % 8 == 3 {
                            mark(x + 3, wy, split);
                            mark(x + 4, wy + 1, split);
                            mark(x + 5, wy + 1, edge);
                        }
                        if row % 8 == 6 {
                            mark(x - 2, wy, split);
                            mark(x - 3, wy + 1, split);
                            mark(x - 4, wy + 1, edge);
                        }
                    }
                }
            }
        }
    });
}

/// A slide's wake on the ice: two thin scores behind the boots, in the
/// floor's next lighter ink under the same light, drawing in as they age.
/// `at` is the knight's feet in world pixels, `(dx, dy)` the way it slid.
pub(crate) fn paint_skid(im: &mut Img, light: &Light, cam: (i32, i32), at: (i32, i32), (dx, dy): (i32, i32), age: u8) {
    const HOLD: i32 = 15;
    let left = HOLD - i32::from(age);
    if left <= 0 || (dx == 0 && dy == 0) {
        return;
    }
    let fade = glow_window(light);
    let rings = rings();
    let tones = floor_tones();
    let (bx, by) = (-dx.signum(), -dy.signum());
    let (sx, sy) = if dx != 0 { (0, 1) } else { (1, 0) };
    for side in [-2, 2] {
        for k in 3..(8 + left) {
            // Two pixels on, one off: a score, not a bar.
            if k % 3 == 2 {
                continue;
            }
            let wx = at.0 + bx * k + sx * side;
            let wy = at.1 + by * k + sy * side;
            let level = apply_fade(light.floor_level(rings, wx, wy), &fade, light, wx, wy);
            if level == 0 {
                continue;
            }
            let on_floor = PLATE.with(|slot| kind_at(&slot.borrow().kind, wx, wy) == KIND_FLOOR);
            if on_floor {
                im.set(wx - cam.0, wy - cam.1, floor_tone(tones, b'H', level, false, 0, false));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// What each pixel is made of
// ---------------------------------------------------------------------------

/// Plate inks for loose rock, the kind a shovel opens: a pale stone and its
/// lit corner on the earth of the top, a stone course and its seam on the
/// face. The light pass gives them the region's stone, as it does all rock.
const LOOSE_STONE: char = 'j';
const LOOSE_LIGHT: char = 'G';
const LOOSE_FACE: char = 'b';
const LOOSE_SEAM: char = 'n';

/// The top of loose rock: earth with small stones bedded in it.
fn loose_top(wx: i32, wy: i32, seed: u32) -> char {
    let h = hash(wx.div_euclid(4), wy.div_euclid(4), seed.wrapping_add(67));
    if h % 4 != 0 {
        let px = wx.rem_euclid(4) - ((h >> 3) % 2) as i32;
        let py = wy.rem_euclid(4) - ((h >> 5) % 2) as i32;
        if (0..2).contains(&px) && (0..2).contains(&py) {
            return if px == 0 && py == 0 { LOOSE_LIGHT } else { LOOSE_STONE };
        }
        if (0..2).contains(&px) && py == 2 {
            return 'K';
        }
    }
    'X'
}

/// The face of loose rock: packed earth and rounded stones of three sizes.
/// Centres are jittered and staggered, so no straight course appears.
fn loose_face(wx: i32, drop: i32) -> char {
    let col = wx.div_euclid(7);
    let row = drop.div_euclid(7);
    let mut hit = false;
    for sy in (row - 1)..=(row + 1) {
        for sx in (col - 1)..=(col + 1) {
            let size = 2 + (hash(sx, sy, 11) % 3) as i32;
            let jx = (hash(sx, sy, 13) % 5) as i32 - 2;
            let jy = (hash(sx, sy, 17) % 5) as i32 - 2;
            let stagger = if sx.rem_euclid(2) == 0 { 0 } else { 3 };
            let cx = sx * 7 + 3 + jx;
            let cy = sy * 7 + 2 + jy + stagger;
            let dx = wx - cx;
            let dy = drop - cy;
            // A round stone, not a squared block. Earth stays in the gaps.
            if dx * dx + dy * dy <= size * size - 1 {
                hit = true;
            }
        }
    }
    if !hit {
        return LOOSE_SEAM;
    }
    LOOSE_FACE
}

/// Whether a world pixel is rock. Asked many times over while a cell is
/// drawn, so the answers are kept for the stretch of plate being drawn.
fn solid_pixel(cave: &Cave, wx: i32, wy: i32) -> bool {
    if let Some(hit) = solid_cached(wx, wy) {
        return hit;
    }
    let hit = solid_raw(cave, wx, wy);
    solid_remember(wx, wy, hit);
    hit
}

struct SolidMap {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    /// 0 not asked yet, 1 rock, 2 open.
    bits: Vec<u8>,
    live: bool,
}

thread_local! {
    static SOLID: RefCell<SolidMap> = const {
        RefCell::new(SolidMap { x0: 0, y0: 0, w: 0, h: 0, bits: Vec::new(), live: false })
    };
}

fn begin_solid(origin: (i32, i32), size: (i32, i32)) {
    SOLID.with(|slot| {
        let mut map = slot.borrow_mut();
        map.x0 = origin.0;
        map.y0 = origin.1;
        map.w = size.0;
        map.h = size.1;
        let n = (size.0 * size.1) as usize;
        map.bits.clear();
        map.bits.resize(n, 0);
        map.live = true;
    });
}

fn end_solid() {
    SOLID.with(|slot| slot.borrow_mut().live = false);
}

fn solid_cached(wx: i32, wy: i32) -> Option<bool> {
    SOLID.with(|slot| {
        let map = slot.borrow();
        if !map.live {
            return None;
        }
        let x = wx - map.x0;
        let y = wy - map.y0;
        if x < 0 || y < 0 || x >= map.w || y >= map.h {
            return None;
        }
        match map.bits[(y * map.w + x) as usize] {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    })
}

fn solid_remember(wx: i32, wy: i32, hit: bool) {
    SOLID.with(|slot| {
        let mut map = slot.borrow_mut();
        if !map.live {
            return;
        }
        let x = wx - map.x0;
        let y = wy - map.y0;
        let w = map.w;
        let h = map.h;
        if x < 0 || y < 0 || x >= w || y >= h {
            return;
        }
        map.bits[(y * w + x) as usize] = if hit { 1 } else { 2 };
    });
}

fn solid_raw(cave: &Cave, wx: i32, wy: i32) -> bool {
    let cx = wx.div_euclid(CELL);
    let cy = wy.div_euclid(CELL);
    let Some(cell) = cave.at(cx, cy) else {
        return true;
    };
    let lx = wx - cx * CELL;
    let ly = wy - cy * CELL;
    let solid_at = |x: i32, y: i32| cave.at(x, y).is_some_and(|c| c.solid());
    if cell.solid() {
        // A corner is rounded off by two or three pixels: never four, which
        // would cut a lone cell of rock away to nothing.
        let cut = 2 + (hash(cx, cy, cave.seed.wrapping_add(41)) % 2) as i32;
        if lx < 4 || ly < 4 || lx >= CELL - 4 || ly >= CELL - 4 {
            let nw = (!solid_at(cx - 1, cy) && !solid_at(cx, cy - 1))
                || (solid_at(cx - 1, cy) && solid_at(cx, cy - 1) && !solid_at(cx - 1, cy - 1));
            if lx < cut && ly < cut && nw {
                return false;
            }
            let ne = (!solid_at(cx + 1, cy) && !solid_at(cx, cy - 1))
                || (solid_at(cx + 1, cy) && solid_at(cx, cy - 1) && !solid_at(cx + 1, cy - 1));
            if lx >= CELL - cut && ly < cut && ne {
                return false;
            }
            let sw = (!solid_at(cx - 1, cy) && !solid_at(cx, cy + 1))
                || (solid_at(cx - 1, cy) && solid_at(cx, cy + 1) && !solid_at(cx - 1, cy + 1));
            if lx < cut && ly >= CELL - cut && sw {
                return false;
            }
            let se = (!solid_at(cx + 1, cy) && !solid_at(cx, cy + 1))
                || (solid_at(cx + 1, cy) && solid_at(cx, cy + 1) && !solid_at(cx + 1, cy + 1));
            if lx >= CELL - cut && ly >= CELL - cut && se {
                return false;
            }
            if ly == 0 && !solid_at(cx, cy - 1) && solid_at(cx - 1, cy) && solid_at(cx + 1, cy) {
                let nick = hash(cx, cy, cave.seed.wrapping_add(53));
                if nick % 11 == 0 && lx >= 2 && lx <= 4 {
                    return false;
                }
            }
        }
        return true;
    }
    let bump = hash(cx, cy, cave.seed.wrapping_add(17));
    if bump % 19 == 0 && solid_at(cx, cy + 1) && ly >= 5 && lx >= 2 && lx <= 5 {
        return true;
    }
    if bump % 19 == 3 && solid_at(cx, cy - 1) && ly <= 2 && lx >= 2 && lx <= 5 {
        return true;
    }
    if bump % 19 == 6 && solid_at(cx + 1, cy) && lx >= 5 && ly >= 2 && ly <= 5 {
        return true;
    }
    if bump % 19 == 9 && solid_at(cx - 1, cy) && lx <= 2 && ly >= 2 && ly <= 5 {
        return true;
    }
    let rock = hash(cx, cy, cave.seed.wrapping_add(29));
    if rock % 23 == 0
        && (solid_at(cx, cy + 1) || solid_at(cx, cy - 1) || solid_at(cx + 1, cy) || solid_at(cx - 1, cy))
    {
        return matches!(
            (lx, ly),
            (2, 1) | (3, 1) | (4, 1) | (1, 2) | (2, 2) | (3, 2) | (4, 2) | (5, 2) | (2, 3) | (3, 3) | (4, 3) | (3, 4)
        );
    }
    false
}

/// A dug pocket is a narrow opening in the rock. Its floor must read as floor,
/// so only a short lip of the north face hangs into it. A wide cavern keeps
/// the full face.
const POCKET_LIP: i32 = 4;

fn narrow_pocket(cave: &Cave, cx: i32, cy: i32) -> bool {
    let rock = |x: i32, y: i32| cave.at(x, y).is_some_and(|c| c.solid());
    let west = rock(cx - 1, cy) || rock(cx - 2, cy);
    let east = rock(cx + 1, cy) || rock(cx + 2, cy);
    west && east
}

fn face_drop(cave: &Cave, wx: i32, wy: i32) -> Option<(i32, u8)> {
    let (drop, region) = south_drop(cave, wx, wy)?;
    if drop >= POCKET_LIP && narrow_pocket(cave, wx.div_euclid(CELL), wy.div_euclid(CELL)) {
        return None;
    }
    Some((drop, region))
}

fn south_drop(cave: &Cave, wx: i32, wy: i32) -> Option<(i32, u8)> {
    if solid_pixel(cave, wx, wy) {
        return None;
    }
    let mut y = wy - 1;
    let mut gap = 1i32;
    let limit = FACE_PX + SHADOW_PX;
    while gap <= limit {
        if solid_pixel(cave, wx, y) {
            let region = region_of(wx.div_euclid(CELL), y.div_euclid(CELL)).0;
            return Some((gap - 1, region));
        }
        y -= 1;
        gap += 1;
    }
    None
}

fn pixel_ink(cave: &Cave, cx: i32, cy: i32, wx: i32, wy: i32, cell: &Cell, solid: bool, tick: u32) -> char {
    let region = region_of(cx, cy).0;
    let lx = wx - cx * CELL;
    let ly = wy - cy * CELL;
    if let Some((drop, wall_region)) = face_drop(cave, wx, wy) {
        let col = wx.rem_euclid(12);
        let icicle = wall_region == 3
            && drop >= 3
            && (((4..7).contains(&col) && drop < 10) || (col == 5 && drop < 20));
        if icicle {
            return 'q';
        }
        if wall_region == 3 {
            let bank = 8 + (hash(wx / 18, 4, cave.seed) % 7) as i32;
            let foot = FACE_PX + SHADOW_PX;
            if drop + bank >= foot {
                return if drop + 3 >= foot { 'i' } else { 'H' };
            }
        }
        if drop < FACE_PX {
            let wall = (wx.div_euclid(CELL), (wy - drop - 1).div_euclid(CELL));
            if cave.at(wall.0, wall.1).is_some_and(|c| c.wall() && c.loose()) {
                return loose_face(wx, drop);
            }
            return face_ink(wall_region, wx, drop);
        }
        return shadow_ink(drop);
    }
    if cell.fire > 0 && !solid {
        return fire_ink(wx, wy, tick);
    }
    if solid {
        if cell.wall() && cell.loose() {
            return loose_top(wx, wy, cave.seed);
        }
        return top_ink(wx, wy, cave.seed);
    }
    if cell.wood() {
        return if lx < 2 || lx > 5 {
            'n'
        } else if ly < 2 {
            'P'
        } else {
            'I'
        };
    }
    if pools_flat() && cell.steam > 0 && lx < 3 && ly < 3 {
        return 'i';
    }
    if !pools_flat() {
        if let Some(ch) = pool_or_floor(cave, wx, wy, cell, tick) {
            return ch;
        }
    } else if !cell.wall() && !cell.sealed() && cell.amount > 0 {
        if cell.fluid == FLUID_LAVA {
            return lava_ink(wx, wy, tick);
        }
        if cell.fluid == FLUID_WATER {
            return water_ink(wx, ly, tick);
        }
        if cell.fluid == FLUID_ACID {
            return acid_ink(wx, wy, tick);
        }
    }
    if cell.ash() {
        return 'K';
    }
    if ember(cave, cx, cy, wx, wy, tick) {
        return '@';
    }
    floor_body(cave, region, cx, cy, wx, wy, cell)
}

fn fire_ink(wx: i32, wy: i32, tick: u32) -> char {
    let band = ((wx / 3) + (wy / 4) + (tick / 4) as i32).rem_euclid(5);
    match band {
        0 => '@',
        1 => '5',
        2 => '7',
        3 => '6',
        _ => 'p',
    }
}

fn lava_ink(wx: i32, wy: i32, tick: u32) -> char {
    let shift = (tick / 4) as i32;
    if (wy + shift).rem_euclid(6) == 0 && wx.rem_euclid(8) < 3 {
        '@'
    } else if wy.rem_euclid(5) == 2 {
        'Z'
    } else if wx.rem_euclid(4) == 0 {
        '8'
    } else {
        '7'
    }
}

fn water_ink(wx: i32, ly: i32, tick: u32) -> char {
    let shift = ((tick / 6) % 8) as i32;
    let hx = (wx + shift).rem_euclid(8);
    if ly <= 1 {
        '1'
    } else if ly == 2 {
        'q'
    } else if hx == 3 {
        '2'
    } else {
        '0'
    }
}

fn acid_ink(wx: i32, wy: i32, tick: u32) -> char {
    let col = (wx + (tick / 5) as i32).rem_euclid(9);
    if col == 4 && wy.rem_euclid(6) < 2 {
        'Y'
    } else if col < 3 {
        'E'
    } else {
        'A'
    }
}

fn ember(cave: &Cave, cx: i32, cy: i32, wx: i32, wy: i32, tick: u32) -> bool {
    cave.at(cx, cy + 1).is_some_and(|c| c.fire > 0)
        && wx.rem_euclid(11) == ((tick / 3) as i32).rem_euclid(11)
        && wy.rem_euclid(4) == 1
}

fn face_ink(region: u8, wx: i32, drop: i32) -> char {
    let band = match region {
        0 => {
            if drop < 4 {
                'h'
            } else if drop >= 14 {
                'K'
            } else {
                'g'
            }
        }
        1 => {
            if drop < 5 { 'E' } else { 'f' }
        }
        2 => {
            if drop < 4 {
                'J'
            } else if drop >= 12 {
                'X'
            } else {
                'g'
            }
        }
        _ => {
            if drop < 5 { 'Q' } else { 'K' }
        }
    };
    // A straight course would read as masonry. A pit or a chip breaks the line.
    let n = hash(wx, drop, 29);
    if n % 7 == 0 {
        return 'K';
    }
    if n % 9 == 0 {
        return 'h';
    }
    band
}

fn shadow_ink(drop: i32) -> char {
    if drop <= FACE_PX + 1 {
        'k'
    } else {
        'K'
    }
}

fn top_ink(wx: i32, wy: i32, seed: u32) -> char {
    let h = hash(wx, wy, seed.wrapping_add(19));
    if h % 83 == 0 {
        return 'K';
    }
    let bx = wx.div_euclid(9);
    let by = wy.div_euclid(5);
    if hash(bx, by, seed.wrapping_add(23)) % 13 == 0 {
        let y0 = (hash(bx, by, seed.wrapping_add(29)) % 4) as i32;
        if wy.rem_euclid(5) == y0 && wx.rem_euclid(9) < 3 {
            return 'K';
        }
    }
    if h % 61 == 0 {
        return 'X';
    }
    'g'
}

fn floor_blob(wx: i32, wy: i32, seed: u32) -> bool {
    const GRID: i32 = 28;
    const HALF: [i32; 17] = [8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16];
    let bx = wx.div_euclid(GRID);
    let by = wy.div_euclid(GRID);
    for oy in -1..=1 {
        for ox in -1..=1 {
            let h = hash(bx + ox, by + oy, seed);
            if h % 7 != 0 {
                continue;
            }
            let span = 16 + ((h >> 4) % 17) as i32;
            let half = HALF[(span - 16) as usize];
            let jx = (h >> 9) % 5;
            let jy = (h >> 12) % 5;
            let cx = (bx + ox) * GRID + 6 + jx as i32;
            let cy = (by + oy) * GRID + 6 + jy as i32;
            let dx = wx - cx;
            let dy = wy - cy;
            let wander = 1 + (hash(wx.div_euclid(2), wy.div_euclid(2), seed.wrapping_add(9)) % 3) as i32;
            let sign = if hash(wx, wy, seed.wrapping_add(5)) & 1 == 0 { -1 } else { 1 };
            let r = (half + sign * wander).max(4);
            if dx * dx + dy * dy <= r * r {
                return true;
            }
        }
    }
    false
}

fn ice_scratch(wx: i32, wy: i32) -> bool {
    let row = wy.div_euclid(36);
    let h = hash(row, 2, 29);
    if h % 4 != 0 {
        return false;
    }
    let y0 = row * 36 + 8 + (h % 20) as i32;
    if wy != y0 {
        return false;
    }
    let x0 = ((h >> 8) % 48) as i32;
    let len = 16 + ((h >> 4) % 17) as i32;
    let lx = wx.rem_euclid(96);
    lx >= x0 && lx < x0 + len
}

fn snow_drift(cave: &Cave, wx: i32, wy: i32) -> bool {
    let mut foot = false;
    for d in 22..=27 {
        if solid_pixel(cave, wx, wy - d) {
            foot = true;
            break;
        }
    }
    if !foot {
        return false;
    }
    hash(wx.div_euclid(3), wy, 17) % 3 != 0
}

fn floor_body(cave: &Cave, region: u8, cx: i32, cy: i32, wx: i32, wy: i32, cell: &Cell) -> char {
    let seed = cave.seed;
    let lx = wx - cx * CELL;
    let ly = wy - cy * CELL;
    if cell.ice() || region == 3 {
        return ice_floor(cave, cx, cy, lx, ly, wx, wy);
    }
    if cell.rubble() {
        if (1..7).contains(&lx) && (2..7).contains(&ly) {
            return 'I';
        }
        return 'P';
    }
    if ly == 7 && cave.at(cx, cy + 1).is_some_and(|c| c.solid()) {
        return 'n';
    }
    if let Some(ch) = ornament(region, wx, wy, seed) {
        return ch;
    }
    match region {
        1 => {
            if mushroom(cx, cy, lx, ly, seed) {
                return 'Y';
            }
            if moss_clump(wx, wy, seed) {
                return 'l';
            }
            'E'
        }
        2 => foundry_floor(wx, wy),
        _ => mine_floor(cx, cy, lx, ly, wx, wy, seed),
    }
}

fn ice_floor(cave: &Cave, cx: i32, cy: i32, lx: i32, ly: i32, wx: i32, wy: i32) -> char {
    if ice_rim(cave, cx, cy, lx, ly) {
        return '1';
    }
    if snow_drift(cave, wx, wy) {
        return 'H';
    }
    if ice_scratch(wx, wy) {
        return 'j';
    }
    'i'
}

fn ice_rim(cave: &Cave, cx: i32, cy: i32, lx: i32, ly: i32) -> bool {
    let (nx, ny) = if lx == 0 {
        (cx - 1, cy)
    } else if lx == 7 {
        (cx + 1, cy)
    } else if ly == 0 {
        (cx, cy - 1)
    } else if ly == 7 {
        (cx, cy + 1)
    } else {
        return false;
    };
    cave.at(nx, ny).is_some_and(|c| {
        !c.solid() && c.fluid == FLUID_WATER
    })
}

fn mine_floor(cx: i32, cy: i32, lx: i32, ly: i32, wx: i32, wy: i32, seed: u32) -> char {
    if (18..26).contains(&cx) && cy == 44 {
        if ly == 3 {
            return 'n';
        }
        if ly == 4 && lx.rem_euclid(8) < 2 {
            return 'K';
        }
    }
    if cy == 44 && (18..32).contains(&cx) && (ly == 3 || ly == 5) {
        return 'n';
    }
    let gx = wx.div_euclid(8);
    let gy = wy.div_euclid(8);
    let lx8 = wx.rem_euclid(8);
    let ly8 = wy.rem_euclid(8);
    let h = hash(gx, gy, 11);
    let x0 = (h % 6) as i32;
    let y0 = ((h / 6) % 6) as i32;
    let x1 = 1 + ((h / 36) % 5) as i32;
    let y1 = 1 + ((h / 180) % 5) as i32;
    if (lx8 == x0 && ly8 == y0) || (lx8 == x1 && ly8 == y1) || (lx8 == x1 + 1 && ly8 == y1) {
        return 'r';
    }
    if floor_blob(wx, wy, seed.wrapping_add(19)) {
        'p'
    } else {
        'P'
    }
}

fn foundry_floor(wx: i32, wy: i32) -> char {
    let cx = wx.div_euclid(CELL);
    let cy = wy.div_euclid(CELL);
    let lx = wx - cx * CELL;
    let ly = wy - cy * CELL;
    if (147..154).contains(&cx) && (27..31).contains(&cy) {
        return 'K';
    }
    if (145..156).contains(&cx) && (31..35).contains(&cy) && (1..7).contains(&lx) {
        return 'n';
    }
    if (148..153).contains(&cx) && (47..52).contains(&cy) && ly >= 2 {
        return 'K';
    }
    let row = wy.div_euclid(12);
    let off = if row % 2 == 0 { 0 } else { 6 };
    let bx = (wx + off).rem_euclid(12);
    let by = wy.rem_euclid(12);
    if bx == 0 || by == 0 { 'I' } else { 'P' }
}

fn moss_clump(wx: i32, wy: i32, seed: u32) -> bool {
    let gx = wx.div_euclid(24);
    let gy = wy.div_euclid(24);
    if hash(gx, gy, seed.wrapping_add(4)) % 5 != 0 {
        return false;
    }
    let lx = wx.rem_euclid(24);
    let ly = wy.rem_euclid(24);
    (8..18).contains(&lx) && (8..16).contains(&ly)
}

fn ornament(region: u8, wx: i32, wy: i32, seed: u32) -> Option<char> {
    let gx = wx.div_euclid(72);
    let gy = wy.div_euclid(72);
    if hash(gx, gy, seed.wrapping_add(region as u32)) % 8 != 0 {
        return None;
    }
    let lx = wx.rem_euclid(72);
    let ly = wy.rem_euclid(72);
    match region {
        0 => {
            if (8..22).contains(&lx) && (40..44).contains(&ly) {
                return Some('I');
            }
            if ly == 48 && (10..40).contains(&lx) {
                return Some('n');
            }
            if ly == 49 && (10..40).contains(&lx) && lx.rem_euclid(8) < 2 {
                return Some('K');
            }
            None
        }
        1 => {
            if (16..28).contains(&lx) && (20..28).contains(&ly) {
                let corner = (lx < 18 || lx > 25) && (ly < 22 || ly > 25);
                if !corner {
                    return Some('l');
                }
            }
            None
        }
        2 => {
            if (18..30).contains(&lx) && (22..30).contains(&ly) {
                if ly < 24 {
                    return Some('n');
                }
                return Some('X');
            }
            None
        }
        _ => {
            if (12..28).contains(&lx) && (36..42).contains(&ly) {
                return Some('w');
            }
            None
        }
    }
}

fn mushroom(cx: i32, cy: i32, lx: i32, ly: i32, seed: u32) -> bool {
    let wx = cx * CELL + lx;
    let wy = cy * CELL + ly;
    for oy in 0..2 {
        for ox in 0..2 {
            let h = hash(wx - ox, wy - oy, seed.wrapping_add(91));
            if h % 2112 == 0 {
                return true;
            }
        }
    }
    false
}

include!("cave_veins.rs");
include!("cave_pools.rs");
