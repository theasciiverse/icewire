//! Arranges a rover's parts on the bench by its signal flow, the layered
//! way (after Sugiyama): sources on the left, each logic part one column
//! right of the parts that feed it, drive and weapons in the last column;
//! then each column ordered to cross as few wires as it can, by sweeping
//! back and forth and moving every part toward the average height of the
//! parts it is wired to. The wires are routed afterwards as always.
//!
//! A wire that closes a loop (loops read last tick's value) does not push
//! a part further right, so every rover has an arrangement.

use std::collections::HashMap;

use crate::design::{BENCH_H, BENCH_W, Design};
use crate::parts::{BOX_H, BOX_W, Category, Kind};

/// Where the arranged columns start, and the room between them.
const LEFT: i32 = 4;
const TOP: i32 = 2;
const COL_GAP: i32 = 6;
const ROW_STEP: i32 = BOX_H + 2;
const SWEEPS: usize = 12;

/// The order drive and weapon parts take in the last column.
fn actuator_rank(k: Kind) -> usize {
    match k {
        Kind::TrackLeft => 0,
        Kind::TrackRight => 1,
        Kind::Gyro => 2,
        Kind::PelletGun => 3,
        Kind::Lobber => 4,
        _ => 5,
    }
}

fn actuator(k: Kind) -> bool {
    matches!(k.category(), Category::Drive | Category::Weapon)
}

/// New positions for every part, as (id, x, y); None when the rover is
/// too big to arrange on the bench (it is then left as it is).
pub fn arrange(d: &Design) -> Option<Vec<(u32, i32, i32)>> {
    let ids: Vec<u32> = d.parts.iter().map(|p| p.id).collect();
    let kind: HashMap<u32, Kind> = d.parts.iter().map(|p| (p.id, p.kind)).collect();
    let pos: HashMap<u32, (i32, i32)> = d.parts.iter().map(|p| (p.id, (p.x, p.y))).collect();
    let edges: Vec<(u32, u32)> = d.wires.iter().map(|w| (w.from, w.to)).filter(|(f, t)| kind.contains_key(f) && kind.contains_key(t) && f != t).collect();
    // which input each wire goes into: a above b, fwd above back
    let port: HashMap<(u32, u32), usize> = d.wires.iter().map(|w| ((w.from, w.to), w.port)).collect();

    // the wires that close loops, found by a depth-first walk from the
    // sources in bench order, are left out of the layering
    let mut order_ids = ids.clone();
    order_ids.sort_by_key(|id| (pos[id].0, pos[id].1, *id));
    let succ = |id: u32| edges.iter().filter(move |e| e.0 == id).map(|e| e.1);
    let mut state: HashMap<u32, u8> = HashMap::new(); // 1 on the path, 2 done
    let mut back: Vec<(u32, u32)> = Vec::new();
    fn walk(id: u32, succ: &dyn Fn(u32) -> Vec<u32>, state: &mut HashMap<u32, u8>, back: &mut Vec<(u32, u32)>) {
        state.insert(id, 1);
        for s in succ(id) {
            match state.get(&s) {
                Some(1) => back.push((id, s)),
                Some(_) => {}
                None => walk(s, succ, state, back),
            }
        }
        state.insert(id, 2);
    }
    let succ_vec = |id: u32| succ(id).collect::<Vec<_>>();
    for &id in &order_ids {
        if !state.contains_key(&id) {
            walk(id, &succ_vec, &mut state, &mut back);
        }
    }
    let flow: Vec<(u32, u32)> = edges.iter().copied().filter(|e| !back.contains(e)).collect();

    // layers: longest path from a source, actuators all in the last one
    let mut layer: HashMap<u32, usize> = ids.iter().map(|&id| (id, 0)).collect();
    for _ in 0..ids.len() {
        let mut changed = false;
        for &(f, t) in &flow {
            if actuator(kind[&t]) {
                continue;
            }
            let want = layer[&f] + 1;
            if layer[&t] < want {
                layer.insert(t, want);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // a logic part nothing feeds and that feeds something waits beside it
    let last_inner = ids.iter().filter(|id| !actuator(kind[id])).map(|id| layer[id]).max().unwrap_or(0);
    let act_layer = if ids.iter().any(|id| actuator(kind[id])) { last_inner + 1 } else { last_inner };
    for &id in &ids {
        if actuator(kind[&id]) {
            layer.insert(id, act_layer);
        }
    }
    let nl = act_layer + 1;

    // the columns, first in bench order (the player's own sense of up and
    // down), the last in the actuators' fixed order
    let mut cols: Vec<Vec<u32>> = vec![Vec::new(); nl];
    let mut by_y = ids.clone();
    by_y.sort_by_key(|id| (pos[id].1, pos[id].0, *id));
    for id in by_y {
        cols[layer[&id]].push(id);
    }
    if let Some(last) = cols.last_mut() {
        last.sort_by_key(|id| (actuator_rank(kind[id]), pos[id].1, *id));
    }

    // order within columns: barycentre sweeps, keeping the best seen
    let index = |cols: &Vec<Vec<u32>>| -> HashMap<u32, (usize, f64)> {
        let mut m = HashMap::new();
        for (l, c) in cols.iter().enumerate() {
            let n = c.len().max(1) as f64;
            for (i, id) in c.iter().enumerate() {
                // spread each column over the same height so columns of
                // different lengths compare fairly
                m.insert(*id, (l, (i as f64 + 0.5) / n));
            }
        }
        m
    };
    let crossings = |cols: &Vec<Vec<u32>>| -> usize {
        let at = index(cols);
        // every wire as a straight run from its left column to its right
        // one; two runs cross when, over the columns both span, their
        // heights swap order
        let runs: Vec<(f64, f64, f64, f64)> = edges
            .iter()
            .filter_map(|e| {
                // a wire ends on its input's row, a little below the part's top
                let row = port.get(e).copied().unwrap_or(0) as f64 * 1e-3;
                let (a, mut b) = (at[&e.0], at[&e.1]);
                b.1 += row;
                let (a, b) = if a.0 <= b.0 { (a, b) } else { (b, a) };
                (a.0 != b.0).then_some((a.0 as f64, a.1, b.0 as f64, b.1))
            })
            .collect();
        let y_at = |r: &(f64, f64, f64, f64), x: f64| r.1 + (r.3 - r.1) * (x - r.0) / (r.2 - r.0);
        let mut n = 0;
        for (i, a) in runs.iter().enumerate() {
            for b in &runs[i + 1..] {
                let (lo, hi) = (a.0.max(b.0), a.2.min(b.2));
                if lo >= hi {
                    continue;
                }
                if (y_at(a, lo) - y_at(b, lo)) * (y_at(a, hi) - y_at(b, hi)) < 0.0 {
                    n += 1;
                }
            }
        }
        n
    };
    let mut best = cols.clone();
    let mut best_x = crossings(&cols);
    for sweep in 0..SWEEPS {
        let down = sweep % 2 == 0;
        let ls: Vec<usize> = if down { (1..nl).collect() } else { (0..nl.saturating_sub(1)).rev().collect() };
        for l in ls {
            if l == nl - 1 && act_layer == l && ids.iter().any(|id| actuator(kind[id])) {
                continue; // the actuators keep their fixed order
            }
            let at = index(&cols);
            let mut keyed: Vec<(f64, usize, u32)> = cols[l]
                .iter()
                .enumerate()
                .map(|(i, &id)| {
                    let nb: Vec<f64> = edges
                        .iter()
                        .filter_map(|&(f, t)| {
                            let other = if t == id { f } else if f == id { t } else { return None };
                            let (ol, oy) = at[&other];
                            let toward = if down { ol < l } else { ol > l };
                            toward.then_some(oy)
                        })
                        .collect();
                    let own = at[&id].1;
                    let k = if nb.is_empty() { own } else { nb.iter().sum::<f64>() / nb.len() as f64 };
                    (k, i, id)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
            cols[l] = keyed.into_iter().map(|k| k.2).collect();
        }
        let x = crossings(&cols);
        if x < best_x {
            best_x = x;
            best = cols.clone();
        }
    }
    // then neighbours swap places wherever that crosses fewer wires (the
    // sweeps alone cannot break a tie between two parts that pull the
    // same way)
    let mut cols = best;
    let fixed = ids.iter().any(|id| actuator(kind[id])).then_some(act_layer);
    for _ in 0..20 {
        let mut better = false;
        for l in 0..cols.len() {
            if Some(l) == fixed {
                continue;
            }
            for i in 0..cols[l].len().saturating_sub(1) {
                cols[l].swap(i, i + 1);
                let x = crossings(&cols);
                if x < best_x {
                    best_x = x;
                    better = true;
                } else {
                    cols[l].swap(i, i + 1);
                }
            }
        }
        if !better {
            break;
        }
    }

    // columns too tall for the bench split into neighbours
    let max_rows = ((BENCH_H - TOP - BOX_H) / ROW_STEP + 1).max(1) as usize;
    let mut placed: Vec<Vec<u32>> = Vec::new();
    for c in cols {
        if c.is_empty() {
            continue;
        }
        for chunk in c.chunks(max_rows) {
            placed.push(chunk.to_vec());
        }
    }
    // spacing: the usual gap, tighter when there are many columns; a
    // long chain wraps onto a band below, like a line of text
    let n = placed.len().max(1) as i32;
    let room = BENCH_W - LEFT - BOX_W - 2;
    let min_step = BOX_W + 3;
    let per_band = (room / min_step + 1).max(1);
    let in_band = n.min(per_band);
    let step = if in_band > 1 { (BOX_W + COL_GAP).min(room / (in_band - 1)).max(min_step) } else { 0 };
    let tallest = placed.iter().map(|c| c.len()).max().unwrap_or(0) as i32;
    let band_h = tallest * ROW_STEP + 2;
    let bands = (n + per_band - 1) / per_band;
    if TOP + bands * band_h - 2 > BENCH_H {
        return None;
    }
    let mut out = Vec::new();
    for (ci, c) in placed.iter().enumerate() {
        let (band, col) = (ci as i32 / per_band, ci as i32 % per_band);
        // each column centred on the tallest, on whole rows
        let off = (tallest - c.len() as i32) / 2 * ROW_STEP;
        for (ri, &id) in c.iter().enumerate() {
            out.push((id, LEFT + col * step, TOP + band * band_h + off + ri as i32 * ROW_STEP));
        }
    }
    Some(out)
}

/// Moves the parts to their arranged places; how many columns they stand
/// in, or None when the rover is too big to arrange.
pub fn apply(d: &mut Design) -> Option<usize> {
    let at = arrange(d)?;
    for (id, x, y) in at {
        if let Some(p) = d.part_mut(id) {
            p.x = x;
            p.y = y;
        }
    }
    Some(at_columns(d))
}

/// How many columns the parts stand in.
pub fn at_columns(d: &Design) -> usize {
    let mut xs: Vec<i32> = d.parts.iter().map(|p| p.x).collect();
    xs.sort();
    xs.dedup();
    xs.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design;
    use crate::route;

    fn check(src: &str) -> Design {
        let mut d = Design::from_text(src).unwrap();
        apply(&mut d).expect("arranged");
        // every part fits where it was put, apart from all the others
        for p in &d.parts {
            assert!(d.fits(p.x, p.y, Some(p.id)), "{} at {},{} overlaps in {}", p.kind.name(), p.x, p.y, d.name);
        }
        // every wire still finds its way
        let l = route::layout(&d);
        assert!(l.paths.iter().all(|p| !p.is_empty()), "a wire found no way in {}", d.name);
        // signal flows left to right: sources left of drive
        let x = |k: Kind| d.parts.iter().filter(|p| p.kind == k).map(|p| p.x).min();
        if let (Some(c), Some(t)) = (x(Kind::Compass), x(Kind::TrackLeft)) {
            assert!(c < t, "compass left of the tracks in {}", d.name);
        }
        d
    }

    #[test]
    fn every_built_in_rover_arranges_cleanly() {
        for src in [design::STARTER, design::TUMBLER, design::COMET, design::SPROCKET, design::KETTLE, design::NIGHTJAR, design::SLEET] {
            check(src);
        }
    }

    #[test]
    fn logic_stands_between_what_feeds_it_and_the_drive() {
        let d = check(design::COMET);
        let x = |k: Kind| d.parts.iter().find(|p| p.kind == k).unwrap().x;
        assert!(x(Kind::Compare) > x(Kind::Compass));
        assert!(x(Kind::Or) > x(Kind::Compare));
        assert!(x(Kind::TrackLeft) > x(Kind::Or));
        // the drive column in its fixed order, top to bottom
        let y = |k: Kind| d.parts.iter().find(|p| p.kind == k).unwrap().y;
        assert!(y(Kind::TrackLeft) < y(Kind::TrackRight) && y(Kind::TrackRight) < y(Kind::Gyro));
    }

    /// Cells where two different sources' wires cross square on.
    fn crossing_cells(d: &Design) -> usize {
        let l = route::layout(d);
        l.cells.iter().filter(|c| c.mask == route::N | route::E | route::S | route::W && c.h.is_some() && c.v.is_some() && c.h != c.v).count()
    }

    #[test]
    fn a_scattered_rover_comes_out_with_no_crossings() {
        // Comet's wiring with every part dropped anywhere
        let src = "icewire rover 1\nname Messy\npart 1 compass at 40 20 mode bearing\npart 2 compass at 4 26 mode distance\npart 3 compare at 60 2 mode > value 3\npart 4 number at 22 14 value 0.4\npart 5 or at 4 2\npart 6 track-left at 22 26 value 100\npart 7 track-right at 60 14 value 100\npart 8 gyro at 40 8 value 8\nwire 1 to 8 right\nwire 2 to 3 a\nwire 3 to 5 a\nwire 4 to 5 b\nwire 5 to 6 fwd\nwire 5 to 7 fwd\n";
        let before = crossing_cells(&Design::from_text(src).unwrap());
        let d = check(src);
        assert_eq!(crossing_cells(&d), 0, "was {before} crossings before");
        assert!(before > 0, "the scattered rover did cross before");
    }

    #[test]
    fn arranging_twice_changes_nothing_more() {
        let mut d = Design::from_text(design::NIGHTJAR).unwrap();
        apply(&mut d);
        let once = d.to_text();
        apply(&mut d);
        assert_eq!(d.to_text(), once);
    }

    #[test]
    fn a_loop_does_not_stop_it() {
        // a toggle fed by its own output through a NOT
        let src = "icewire rover 1\nname Loop\npart 1 toggle at 4 2\npart 2 not at 30 2\npart 3 track-left at 60 2\nwire 1 to 2 in\nwire 2 to 1 in\nwire 1 to 3 fwd\n";
        check(src);
    }

    #[test]
    fn a_full_logic_board_still_fits() {
        let mut src = String::from("icewire rover 1\nname Busy\npart 1 number at 4 2\n");
        for i in 0..30 {
            src.push_str(&format!("part {} not at {} {}\n", i + 2, 4 + (i % 6) * 20, 8 + (i / 6) * 8));
        }
        for i in 0..30 {
            let from = if i == 0 { 1 } else { i + 1 };
            src.push_str(&format!("wire {} to {} in\n", from, i + 2));
        }
        check(&src);
    }
}
