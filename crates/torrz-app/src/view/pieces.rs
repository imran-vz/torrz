//! The piece map, dithered: each cell is a 4×4 ordered (Bayer) dither of the torrent's colour,
//! so a cell half there is a half-lit pattern, as the app icon is drawn. Two kinds of motion
//! play on the renderer's own clock, never by redrawing the view:
//!
//! - **In flight.** In a cell part-way there, the dots still to come pulse in their dither
//!   order, so the cell looks to be filling in.
//! - **Arrived.** Dots that lit since the last snapshot flash white and fade into the colour.
//!
//! Under Reduce Motion both hold still (the pulse rests invisible; the flash lands at once).

use rune::{Beat, Color, Cycle, El, Easing, LoopSpec, Path, Sequence, Shape, canvas, el, row};

use crate::theme as t;

/// The 4×4 Bayer matrix: the order a cell's 16 dots light in as it fills.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/// One dither dot, in points.
const DOT: f32 = 2.0;
const CELL: f32 = DOT * 4.0;
const GAP: f32 = 2.0;
/// Cells a tile holds. Tiles wrap, so the map fits any width.
const TILE: usize = 10;

/// Whether dot (`x`, `y`) of a cell `level` (0 to 255) full is lit.
pub fn lit(level: u8, x: usize, y: usize) -> bool {
    level as u32 * 16 > (BAYER[y][x] as u32 * 2 + 1) * 255 / 2
}

/// The pulse each in-flight group plays, a quarter period apart, so the not-yet dots sparkle in
/// the order they will light.
const PULSE: &[(f32, f32)] = &[(0.0, 0.0), (0.12, 0.85), (0.5, 0.0), (1.0, 0.0)];
const FLASH: &[Beat] = &[Beat::new(0.0).opacity(1.0), Beat::new(0.9).opacity(0.0).easing(Easing::EASE_OUT)];

/// Each cell's dots that `keep` selects, as one path of rectangles (a row's neighbouring dots
/// merge into one).
fn dots(levels: &[u8], keep: impl Fn(usize, usize, usize) -> bool) -> Option<Path> {
    let mut path = Path::new();
    let mut any = false;
    for (c, _) in levels.iter().enumerate() {
        let x0 = c as f32 * (CELL + GAP);
        for y in 0..4 {
            let mut x = 0;
            while x < 4 {
                if !keep(c, x, y) {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < 4 && keep(c, x, y) {
                    x += 1;
                }
                let (l, r, top, bottom) = (x0 + start as f32 * DOT, x0 + x as f32 * DOT, y as f32 * DOT, (y + 1) as f32 * DOT);
                path = path.move_to([l, top]).line_to([r, top]).line_to([r, bottom]).line_to([l, bottom]).close();
                any = true;
            }
        }
    }
    any.then_some(path)
}

fn layer(levels: &[u8], color: Color, keep: impl Fn(usize, usize, usize) -> bool) -> Option<El> {
    let w = levels.len() as f32 * (CELL + GAP) - GAP;
    dots(levels, keep).map(|p| canvas(Shape::new().fill_path(&p, color)).size(w, CELL).pin(Some(0.0), Some(0.0), None, None))
}

/// The map, `map` as it is and `before` as it was a snapshot ago (same length, or empty for
/// nothing new). `generation` changes with each snapshot, which replays the arrival flash.
pub fn piece_map(map: &[u8], before: &[u8], ink: Color, generation: u64, animate: bool) -> El {
    let mut grid = row().key("piece-map").w_full().flex_wrap().gaps(GAP, GAP);
    for (n, levels) in map.chunks(TILE).enumerate() {
        let w = levels.len() as f32 * (CELL + GAP) - GAP;
        let base = levels.iter().enumerate().fold(Shape::new(), |s, (c, _)| {
            let x = c as f32 * (CELL + GAP);
            s.fill(vec![[x, 0.0], [x + CELL, 0.0], [x + CELL, CELL], [x, CELL]], t::TRACK)
        });
        let mut tile = el().key(format!("piece-tile-{n}")).size(w, CELL).child(canvas(base).size(w, CELL).pin(Some(0.0), Some(0.0), None, None));
        tile = tile.children(layer(levels, ink, |c, x, y| lit(levels[c], x, y)));
        if animate {
            let in_flight = |c: usize| levels[c] > 0 && levels[c] < 255;
            if levels.iter().enumerate().any(|(c, _)| in_flight(c)) {
                for g in 0..4u8 {
                    let pulse = layer(levels, ink.alpha(0.7), |c, x, y| in_flight(c) && !lit(levels[c], x, y) && BAYER[y][x] / 4 == g).map(|l| {
                        l.key(format!("piece-pulse-{n}-{g}")).cycle(Cycle { period: 1.6, phase: g as f32 / 4.0, stops: PULSE, still: 0.0, fps: 15.0 })
                    });
                    tile = tile.children(pulse);
                }
            }
            if let Some(was) = before.get(n * TILE..n * TILE + levels.len()) {
                let fresh = layer(levels, t::WHITE, |c, x, y| lit(levels[c], x, y) && !lit(was[c], x, y));
                tile = tile.children(fresh.map(|l| l.key(format!("piece-new-{generation}-{n}")).looping(LoopSpec::sequence(Sequence::new(FLASH)))));
            }
        }
        grid = grid.child(tile);
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_lights_as_many_dots_as_it_is_full() {
        let count = |level| (0..4).flat_map(|y| (0..4).map(move |x| (x, y))).filter(|&(x, y)| lit(level, x, y)).count();
        assert_eq!(count(0), 0);
        assert_eq!(count(255), 16);
        assert_eq!(count(128), 8);
        assert_eq!(count(64), 4);
        // Dots only ever come on as a cell fills.
        for level in 1..=255u8 {
            for (x, y) in (0..4).flat_map(|y| (0..4).map(move |x| (x, y))) {
                assert!(!lit(level - 1, x, y) || lit(level, x, y), "dot {x},{y} went out between {} and {level}", level - 1);
            }
        }
    }
}
