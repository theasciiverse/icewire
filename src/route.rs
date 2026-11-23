//! Lays wires out on the bench: each one finds its own way from an output
//! port to an input port round the part boxes, with as few bends as it
//! can, sharing a trunk with wires from the same source and only ever
//! crossing others square on.

use crate::design::{BENCH_H, BENCH_W, Design};
use crate::parts::{BOX_H, BOX_W};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const N: u8 = 1;
pub const E: u8 = 2;
pub const S: u8 = 4;
pub const W: u8 = 8;

const DIRS: [(i32, i32, u8); 4] = [(1, 0, E), (0, 1, S), (-1, 0, W), (0, -1, N)];

/// One drawn cell of wire.
#[derive(Clone, Copy, Default)]
pub struct Cell {
    pub mask: u8,
    /// The source part of the horizontal and the vertical run through it.
    pub h: Option<u32>,
    pub v: Option<u32>,
}

pub struct Layout {
    pub cells: Vec<Cell>,
    /// For each wire of the design, its cells from source to target
    /// (empty when no way was found).
    pub paths: Vec<Vec<(i32, i32)>>,
}

impl Layout {
    pub fn at(&self, x: i32, y: i32) -> Option<&Cell> {
        (x >= 0 && y >= 0 && x < BENCH_W && y < BENCH_H).then(|| &self.cells[(y * BENCH_W + x) as usize]).filter(|c| c.mask != 0)
    }

    /// The wire running through a cell, if any.
    pub fn wire_at(&self, x: i32, y: i32) -> Option<usize> {
        self.paths.iter().position(|p| p.contains(&(x, y)))
    }
}

pub fn glyph(mask: u8) -> char {
    match mask {
        m if m == E | W || m == E || m == W => '─',
        m if m == N | S || m == N || m == S => '│',
        m if m == E | S => '┌',
        m if m == W | S => '┐',
        m if m == N | E => '└',
        m if m == N | W => '┘',
        m if m == N | E | S => '├',
        m if m == N | W | S => '┤',
        m if m == E | W | S => '┬',
        m if m == E | W | N => '┴',
        _ => '┼',
    }
}

fn idx(x: i32, y: i32) -> usize {
    (y * BENCH_W + x) as usize
}

pub fn layout(d: &Design) -> Layout {
    let n = (BENCH_W * BENCH_H) as usize;
    let mut blocked = vec![false; n];
    // ports' outside cells: other wires keep off them
    let mut port = vec![false; n];
    for p in &d.parts {
        for y in p.y..p.y + BOX_H {
            for x in p.x..p.x + BOX_W {
                if x >= 0 && y >= 0 && x < BENCH_W && y < BENCH_H {
                    blocked[idx(x, y)] = true;
                }
            }
        }
        let mut mark = |(x, y): (i32, i32)| {
            if x >= 0 && y >= 0 && x < BENCH_W && y < BENCH_H {
                port[idx(x, y)] = true;
            }
        };
        if p.kind.has_output() {
            mark(p.out_cell());
        }
        for i in 0..p.kind.inputs().len() {
            mark(p.in_cell(i));
        }
    }
    let mut cells = vec![Cell::default(); n];
    let mut paths = Vec::with_capacity(d.wires.len());
    // shorter wires first, so they get the straight runs
    let mut order: Vec<usize> = (0..d.wires.len()).collect();
    let span = |i: usize| {
        let w = &d.wires[i];
        match (d.part(w.from), d.part(w.to)) {
            (Some(f), Some(t)) => {
                let (a, b) = (f.out_cell(), t.in_cell(w.port));
                (a.0 - b.0).abs() + (a.1 - b.1).abs()
            }
            _ => 0,
        }
    };
    order.sort_by_key(|&i| (span(i), i));
    let mut found: Vec<Vec<(i32, i32)>> = vec![Vec::new(); d.wires.len()];
    for i in order {
        let w = d.wires[i];
        let (Some(f), Some(t)) = (d.part(w.from), d.part(w.to)) else { continue };
        let (s, g) = (f.out_cell(), t.in_cell(w.port));
        if let Some(path) = search(s, g, w.from, &blocked, &port, &cells) {
            for (k, &(x, y)) in path.iter().enumerate() {
                let c = &mut cells[idx(x, y)];
                let mut m = 0;
                if k == 0 {
                    m |= W;
                }
                if k + 1 == path.len() {
                    m |= E;
                }
                for nb in [path.get(k.wrapping_sub(1)), path.get(k + 1)].into_iter().flatten() {
                    m |= match (nb.0 - x, nb.1 - y) {
                        (1, 0) => E,
                        (-1, 0) => W,
                        (0, 1) => S,
                        _ => N,
                    };
                }
                if m & (E | W) != 0 {
                    c.h = Some(w.from);
                }
                if m & (N | S) != 0 {
                    c.v = Some(w.from);
                }
                c.mask |= m;
            }
            found[i] = path;
        }
    }
    paths.extend(found);
    Layout { cells, paths }
}

/// A* over (cell, heading). Bends cost, parallel runs beside or over
/// another source's wire cost a lot, square crossings a little.
fn search(s: (i32, i32), g: (i32, i32), src: u32, blocked: &[bool], port: &[bool], cells: &[Cell]) -> Option<Vec<(i32, i32)>> {
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < BENCH_W && y < BENCH_H;
    if !inside(s.0, s.1) || !inside(g.0, g.1) {
        return None;
    }
    let n = (BENCH_W * BENCH_H) as usize * 4;
    let mut best = vec![u32::MAX; n];
    let mut came = vec![u32::MAX; n];
    let state = |x: i32, y: i32, d: usize| (idx(x, y) * 4 + d) as u32;
    let h = |x: i32, y: i32| ((x - g.0).abs() + (y - g.1).abs()) as u32;
    let mut open = BinaryHeap::new();
    // the wire leaves the output port heading east
    let s0 = state(s.0, s.1, 0);
    best[s0 as usize] = 0;
    open.push(Reverse((h(s.0, s.1), 0u32, s0)));
    while let Some(Reverse((_, cost, st))) = open.pop() {
        if cost > best[st as usize] {
            continue;
        }
        let cell = st as usize / 4;
        let d = st as usize % 4;
        let (x, y) = ((cell as i32) % BENCH_W, (cell as i32) / BENCH_W);
        if (x, y) == g {
            let mut path = vec![(x, y)];
            let mut cur = st;
            while came[cur as usize] != u32::MAX {
                cur = came[cur as usize];
                let c = cur as i32 / 4;
                path.push((c % BENCH_W, c / BENCH_W));
            }
            path.reverse();
            return Some(path);
        }
        for (nd, &(dx, dy, bit)) in DIRS.iter().enumerate() {
            if nd == (d + 2) % 4 {
                continue;
            }
            let (nx, ny) = (x + dx, y + dy);
            if !inside(nx, ny) {
                continue;
            }
            let ni = idx(nx, ny);
            if blocked[ni] {
                continue;
            }
            let mut step: u32 = 1;
            if nd != d {
                step += 3;
            }
            if port[ni] && (nx, ny) != g {
                step += 40;
            }
            let c = &cells[ni];
            let horizontal = bit & (E | W) != 0;
            let other = |o: Option<u32>| o.is_some_and(|o| o != src);
            if (horizontal && other(c.h)) || (!horizontal && other(c.v)) {
                step += 200; // running along another wire
            } else if other(c.h) || other(c.v) {
                step += 6; // crossing it
            }
            // turning inside a cell another wire runs through
            let cur = &cells[idx(x, y)];
            if nd != d && (other(cur.h) || other(cur.v)) {
                step += 200;
            }
            if (horizontal && c.h == Some(src)) || (!horizontal && c.v == Some(src)) {
                step = step.saturating_sub(1).max(1); // share the trunk
            }
            let ns = state(nx, ny, nd);
            let nc = cost + step;
            if nc < best[ns as usize] {
                best[ns as usize] = nc;
                came[ns as usize] = st;
                open.push(Reverse((nc + h(nx, ny), nc, ns)));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{STARTER, TUMBLER};

    #[test]
    fn every_wire_finds_a_way() {
        for src in [STARTER, TUMBLER] {
            let d = Design::from_text(src).unwrap();
            let l = layout(&d);
            for (w, p) in d.wires.iter().zip(&l.paths) {
                assert!(!p.is_empty(), "wire {w:?}");
                let f = d.part(w.from).unwrap();
                let t = d.part(w.to).unwrap();
                assert_eq!(p[0], f.out_cell());
                assert_eq!(*p.last().unwrap(), t.in_cell(w.port));
                for c in p {
                    assert!(d.part_at(c.0, c.1).is_none(), "wire through a box at {c:?}");
                }
            }
        }
    }

    #[test]
    fn glyphs() {
        assert_eq!(glyph(E | W), '─');
        assert_eq!(glyph(N | E | S), '├');
        assert_eq!(glyph(N | E | S | W), '┼');
    }
}
