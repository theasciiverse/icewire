//! The arena: a tile map, rovers as discs on ice, and fixed-step physics.
//! Every rover's wiring runs first, then all of them move together, so no
//! rover gains by going first. Same rovers, same result, every time.

use crate::brain::{Brain, Drive};
use crate::challenge::{self, Challenge, Goal};
use crate::design::Design;
use crate::geom::{deg, dir, heading_of, rad, wrap};
use crate::parts::{Kind, Mount, RANGER_MAX};
use std::collections::VecDeque;

pub const TICKS_PER_SEC: u32 = 60;
pub const DT: f64 = 1.0 / TICKS_PER_SEC as f64;
/// Rover radius in metres; a tile is a metre.
pub const RADIUS: f64 = 0.35;
/// Acceleration from one track at full power, m/s^2.
const THRUST: f64 = 1.6;
/// Drag along and across the rover's heading (ice is slippery sideways).
const DRAG_FWD: f64 = 1.0;
const DRAG_SIDE: f64 = 3.0;
/// Speed kept per second while scraping along a wall.
const SCRAPE: f64 = 0.8;
/// Turn rates at full power, rad/s.
const TURN_TRACKS: f64 = 2.4;
const TURN_GYRO: f64 = 3.0;
const TURN_RESPONSE: f64 = 10.0;
const BOUNCE: f64 = 0.15;
const TRAIL: usize = 160;

pub struct Arena {
    pub w: i32,
    pub h: i32,
    solid: Vec<bool>,
    /// Checkpoint number (1-based) of each tile, 0 for none.
    pub cp: Vec<u8>,
    pub exit: Vec<bool>,
    /// Centre of each checkpoint, in order.
    pub cp_centres: Vec<(f64, f64)>,
    pub exit_centre: (f64, f64),
    pub starts: [(f64, f64); 2],
    pub ring: Option<(f64, f64, f64)>,
}

impl Arena {
    pub fn parse(ch: &Challenge) -> Arena {
        let rows: Vec<&str> = ch.map.lines().filter(|l| !l.is_empty()).collect();
        let h = rows.len() as i32;
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let n = (w * h) as usize;
        let mut a = Arena { w, h, solid: vec![false; n], cp: vec![0; n], exit: vec![false; n], cp_centres: Vec::new(), exit_centre: (0.0, 0.0), starts: [(1.5, 1.5); 2], ring: None };
        let mut sums: Vec<(f64, f64, f64)> = Vec::new();
        let mut exit = (0.0, 0.0, 0.0);
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                let i = y * w as usize + x;
                let centre = (x as f64 + 0.5, y as f64 + 0.5);
                match c {
                    '#' => a.solid[i] = true,
                    '1'..='9' => {
                        let k = c as u8 - b'0';
                        a.cp[i] = k;
                        if sums.len() < k as usize {
                            sums.resize(k as usize, (0.0, 0.0, 0.0));
                        }
                        let s = &mut sums[k as usize - 1];
                        *s = (s.0 + centre.0, s.1 + centre.1, s.2 + 1.0);
                    }
                    'E' => {
                        a.exit[i] = true;
                        exit = (exit.0 + centre.0, exit.1 + centre.1, exit.2 + 1.0);
                    }
                    'S' => a.starts[0] = centre,
                    'T' => a.starts[1] = centre,
                    _ => {}
                }
            }
        }
        a.cp_centres = sums.iter().map(|s| (s.0 / s.2.max(1.0), s.1 / s.2.max(1.0))).collect();
        if exit.2 > 0.0 {
            a.exit_centre = (exit.0 / exit.2, exit.1 / exit.2);
        }
        a.ring = ch.ring.map(|r| (w as f64 / 2.0, h as f64 / 2.0, r));
        a
    }

    pub fn solid(&self, x: i32, y: i32) -> bool {
        x < 0 || y < 0 || x >= self.w || y >= self.h || self.solid[(y * self.w + x) as usize]
    }

    fn idx(&self, x: f64, y: f64) -> Option<usize> {
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        (tx >= 0 && ty >= 0 && tx < self.w && ty < self.h).then(|| (ty * self.w + tx) as usize)
    }

    pub fn cp_at(&self, x: f64, y: f64) -> u8 {
        self.idx(x, y).map(|i| self.cp[i]).unwrap_or(0)
    }

    pub fn exit_at(&self, x: f64, y: f64) -> bool {
        self.idx(x, y).map(|i| self.exit[i]).unwrap_or(false)
    }

    /// Distance along a ray to the first solid tile (grid traversal).
    fn ray_tiles(&self, x: f64, y: f64, dx: f64, dy: f64, max: f64) -> f64 {
        let (mut tx, mut ty) = (x.floor() as i32, y.floor() as i32);
        if self.solid(tx, ty) {
            return 0.0;
        }
        let big = f64::MAX;
        let (sx, sy) = (if dx > 0.0 { 1 } else { -1 }, if dy > 0.0 { 1 } else { -1 });
        let ddx = if dx != 0.0 { (1.0 / dx).abs() } else { big };
        let ddy = if dy != 0.0 { (1.0 / dy).abs() } else { big };
        let mut mx = if dx > 0.0 {
            ((tx + 1) as f64 - x) / dx
        } else if dx < 0.0 {
            (tx as f64 - x) / dx
        } else {
            big
        };
        let mut my = if dy > 0.0 {
            ((ty + 1) as f64 - y) / dy
        } else if dy < 0.0 {
            (ty as f64 - y) / dy
        } else {
            big
        };
        loop {
            let t;
            if mx < my {
                t = mx;
                mx += ddx;
                tx += sx;
            } else {
                t = my;
                my += ddy;
                ty += sy;
            }
            if t >= max {
                return max;
            }
            if self.solid(tx, ty) {
                return t;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Rover {
    pub x: f64,
    pub y: f64,
    /// Heading in radians, 0 up the map, clockwise.
    pub h: f64,
    pub vx: f64,
    pub vy: f64,
    pub spin: f64,
    pub drive: Drive,
    /// Touching something at nose, right, tail, left.
    pub bumps: [bool; 4],
    /// Fell off the ring, at this tick.
    pub fell: Option<u32>,
    /// Checkpoints passed.
    pub next_cp: usize,
    pub trail: VecDeque<(f64, f64)>,
}

impl Rover {
    fn new(x: f64, y: f64, heading_deg: f64) -> Rover {
        Rover { x, y, h: rad(heading_deg), vx: 0.0, vy: 0.0, spin: 0.0, drive: Drive::default(), bumps: [false; 4], fell: None, next_cp: 0, trail: VecDeque::new() }
    }

    pub fn speed(&self) -> f64 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }

    /// Heading in degrees, 0 to 360.
    pub fn heading_deg(&self) -> f64 {
        let d = deg(wrap(self.h));
        if d < 0.0 { d + 360.0 } else { d }
    }

    fn bump_toward(&mut self, nx: f64, ny: f64) {
        // nx, ny points from the rover to what it touched
        let rel = deg(wrap(heading_of(nx, ny) - self.h));
        let q = if rel.abs() <= 45.0 {
            0
        } else if rel > 45.0 && rel < 135.0 {
            1
        } else if rel < -45.0 && rel > -135.0 {
            3
        } else {
            2
        };
        self.bumps[q] = true;
    }
}

pub struct Entry {
    pub design: Design,
    pub brain: Brain,
    pub rover: Rover,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Running,
    /// Finished, at this tick.
    Won(u32),
    Lost(&'static str),
    Draw,
}

/// A sensor beam drawn in the arena: from, to, and whether it hit.
#[derive(Clone, Copy, Debug)]
pub struct Beam {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

pub struct Sim {
    pub ch: &'static Challenge,
    pub arena: Arena,
    pub rovers: Vec<Entry>,
    pub tick: u32,
    pub outcome: Outcome,
    /// Things worth a line in the feed, with the tick they happened.
    pub events: Vec<(u32, String)>,
    /// The player's sensor beams this tick.
    pub beams: Vec<Beam>,
}

impl Sim {
    pub fn new(ch_index: usize, player: Design) -> Sim {
        let ch = &challenge::ALL[ch_index];
        let arena = Arena::parse(ch);
        let mut rovers = Vec::new();
        let mut add = |d: Design, i: usize| {
            let (x, y) = arena.starts[i];
            rovers.push(Entry { brain: Brain::new(&d), design: d, rover: Rover::new(x, y, ch.headings[i]) });
        };
        add(player, 0);
        if let Some(src) = ch.opponent {
            add(Design::from_text(src).expect("built-in rover"), 1);
        }
        let events = vec![(0, format!("{}: go!", ch.name))];
        Sim { ch, arena, rovers, tick: 0, outcome: Outcome::Running, events, beams: Vec::new() }
    }

    pub fn secs(&self) -> f64 {
        self.tick as f64 / TICKS_PER_SEC as f64
    }

    /// Where rover `i`'s compass beacon is.
    pub fn beacon(&self, i: usize) -> (f64, f64) {
        match self.ch.goal {
            Goal::Race => {
                let r = &self.rovers[i].rover;
                let n = r.next_cp.min(self.arena.cp_centres.len().saturating_sub(1));
                self.arena.cp_centres.get(n).copied().unwrap_or((0.0, 0.0))
            }
            Goal::Maze => self.arena.exit_centre,
            Goal::Sumo => {
                let o = &self.rovers[(i + 1) % self.rovers.len()].rover;
                (o.x, o.y)
            }
        }
    }

    /// Distance from rover `i`'s hull along heading `a` to the first wall,
    /// rover or ring edge, at most `max`.
    fn cast(&self, i: usize, a: f64, max: f64) -> f64 {
        let r = &self.rovers[i].rover;
        let (dx, dy) = dir(a);
        let reach = max + RADIUS;
        let mut t = self.arena.ray_tiles(r.x, r.y, dx, dy, reach);
        for (j, o) in self.rovers.iter().enumerate() {
            let o = &o.rover;
            if j == i || o.fell.is_some() {
                continue;
            }
            // ray against a disc
            let (fx, fy) = (r.x - o.x, r.y - o.y);
            let b = fx * dx + fy * dy;
            let c = fx * fx + fy * fy - RADIUS * RADIUS;
            let disc = b * b - c;
            if disc >= 0.0 {
                let hit = -b - disc.sqrt();
                if hit >= 0.0 && hit < t {
                    t = hit;
                }
            }
        }
        if let Some((cx, cy, rr)) = self.arena.ring {
            let (fx, fy) = (r.x - cx, r.y - cy);
            let b = fx * dx + fy * dy;
            let c = fx * fx + fy * fy - rr * rr;
            let disc = b * b - c;
            if disc >= 0.0 {
                let hit = -b + disc.sqrt();
                if hit >= 0.0 && hit < t {
                    t = hit;
                }
            }
        }
        (t - RADIUS).clamp(0.0, max)
    }

    fn beam(&mut self, i: usize, a: f64, len: f64) {
        if i != 0 {
            return;
        }
        let r = &self.rovers[0].rover;
        let (dx, dy) = dir(a);
        self.beams.push(Beam { x0: r.x + dx * RADIUS, y0: r.y + dy * RADIUS, x1: r.x + dx * (RADIUS + len), y1: r.y + dy * (RADIUS + len) });
    }

    /// Reads sensor part `p` of rover `i`.
    fn sense(&mut self, i: usize, p: usize) -> f64 {
        let part = self.rovers[i].design.parts[p].clone();
        let r = &self.rovers[i].rover;
        let (rh, bumps, heading) = (r.h, r.bumps, r.heading_deg());
        let (rx, ry) = (r.x, r.y);
        let facing = rh + rad(part.mount.angle());
        match part.kind {
            Kind::Bumper => {
                let on = match part.mount {
                    Mount::All => bumps.iter().any(|b| *b),
                    m => bumps[Mount::SIDES.iter().position(|s| *s == m).unwrap_or(0)],
                };
                if on { 1.0 } else { 0.0 }
            }
            Kind::Whisker => {
                let mut hit = false;
                for off in [-30.0, 0.0, 30.0] {
                    let d = self.cast(i, facing + rad(off), part.value);
                    self.beam(i, facing + rad(off), d);
                    hit |= d < part.value;
                }
                if hit { 1.0 } else { 0.0 }
            }
            Kind::Ranger => {
                let d = self.cast(i, facing, RANGER_MAX);
                self.beam(i, facing, d);
                d
            }
            Kind::Compass => {
                let (bx, by) = self.beacon(i);
                let (dx, dy) = (bx - rx, by - ry);
                match part.mode {
                    0 => heading,
                    1 => deg(wrap(heading_of(dx, dy) - rh)),
                    _ => (dx * dx + dy * dy).sqrt(),
                }
            }
            _ => 0.0,
        }
    }

    /// One fixed step of the whole arena.
    pub fn step(&mut self) {
        if self.outcome != Outcome::Running {
            return;
        }
        self.beams.clear();
        // 1. every rover thinks
        for i in 0..self.rovers.len() {
            if self.rovers[i].rover.fell.is_some() {
                continue;
            }
            let mut brain = self.rovers[i].brain.clone();
            let design = self.rovers[i].design.clone();
            let drive = brain.step(&design, |p| self.sense(i, p));
            self.rovers[i].brain = brain;
            self.rovers[i].rover.drive = drive;
        }
        // 2. every rover moves
        for e in self.rovers.iter_mut() {
            let r = &mut e.rover;
            if r.fell.is_some() {
                continue;
            }
            let Drive { left, right, gyro } = r.drive;
            let want = (left - right) * TURN_TRACKS + gyro * TURN_GYRO;
            r.spin += (want - r.spin) * (TURN_RESPONSE * DT).min(1.0);
            r.h = wrap(r.h + r.spin * DT);
            let (fx, fy) = dir(r.h);
            let acc = (left + right) * THRUST;
            r.vx += fx * acc * DT;
            r.vy += fy * acc * DT;
            let along = r.vx * fx + r.vy * fy;
            let side = r.vx * -fy + r.vy * fx;
            let along = along * (1.0 - DRAG_FWD * DT);
            let side = side * (1.0 - DRAG_SIDE * DT);
            r.vx = fx * along - fy * side;
            r.vy = fy * along + fx * side;
            r.x += r.vx * DT;
            r.y += r.vy * DT;
            r.bumps = [false; 4];
        }
        // 3. rovers push each other
        for i in 0..self.rovers.len() {
            for j in i + 1..self.rovers.len() {
                if self.rovers[i].rover.fell.is_some() || self.rovers[j].rover.fell.is_some() {
                    continue;
                }
                let (a, b) = self.rovers.split_at_mut(j);
                let (a, b) = (&mut a[i].rover, &mut b[0].rover);
                let (dx, dy) = (a.x - b.x, a.y - b.y);
                let d2 = dx * dx + dy * dy;
                if d2 < 4.0 * RADIUS * RADIUS && d2 > 0.0 {
                    let d = d2.sqrt();
                    let (nx, ny) = (dx / d, dy / d);
                    let push = (2.0 * RADIUS - d) / 2.0;
                    a.x += nx * push;
                    a.y += ny * push;
                    b.x -= nx * push;
                    b.y -= ny * push;
                    let rel = (a.vx - b.vx) * nx + (a.vy - b.vy) * ny;
                    if rel < 0.0 {
                        let j = -(1.0 + BOUNCE) * rel / 2.0;
                        a.vx += nx * j;
                        a.vy += ny * j;
                        b.vx -= nx * j;
                        b.vy -= ny * j;
                    }
                    a.bump_toward(-nx, -ny);
                    b.bump_toward(nx, ny);
                }
            }
        }
        // 4. walls push rovers
        for e in self.rovers.iter_mut() {
            let r = &mut e.rover;
            if r.fell.is_some() {
                continue;
            }
            for _ in 0..2 {
                for ty in (r.y - RADIUS).floor() as i32..=(r.y + RADIUS).floor() as i32 {
                    for tx in (r.x - RADIUS).floor() as i32..=(r.x + RADIUS).floor() as i32 {
                        if !self.arena.solid(tx, ty) {
                            continue;
                        }
                        let px = r.x.clamp(tx as f64, tx as f64 + 1.0);
                        let py = r.y.clamp(ty as f64, ty as f64 + 1.0);
                        let (dx, dy) = (r.x - px, r.y - py);
                        let d2 = dx * dx + dy * dy;
                        if d2 >= RADIUS * RADIUS {
                            continue;
                        }
                        let (nx, ny, d) = if d2 > 0.0 {
                            let d = d2.sqrt();
                            (dx / d, dy / d, d)
                        } else {
                            // centre inside a wall: push back the way it came
                            let s = r.speed().max(1e-9);
                            (-r.vx / s, -r.vy / s, 0.0)
                        };
                        r.x += nx * (RADIUS - d);
                        r.y += ny * (RADIUS - d);
                        let vn = r.vx * nx + r.vy * ny;
                        if vn < 0.0 {
                            r.vx -= (1.0 + BOUNCE) * vn * nx;
                            r.vy -= (1.0 + BOUNCE) * vn * ny;
                        }
                        // scraping along the wall costs speed
                        let k = 1.0 - SCRAPE * DT;
                        r.vx *= k;
                        r.vy *= k;
                        r.bump_toward(-nx, -ny);
                    }
                }
            }
        }
        self.tick += 1;
        // 5. the rules
        for i in 0..self.rovers.len() {
            let tick = self.tick;
            let r = &mut self.rovers[i].rover;
            if r.fell.is_some() {
                continue;
            }
            if tick.is_multiple_of(4) {
                r.trail.push_back((r.x, r.y));
                if r.trail.len() > TRAIL {
                    r.trail.pop_front();
                }
            }
            if let Some((cx, cy, rr)) = self.arena.ring {
                let (dx, dy) = (r.x - cx, r.y - cy);
                if dx * dx + dy * dy > rr * rr {
                    r.fell = Some(tick);
                    let who = if i == 0 { "Your rover".to_string() } else { self.rovers[i].design.name.clone() };
                    self.events.push((tick, format!("{who} slides off the ring")));
                }
            }
        }
        let secs = self.secs();
        match self.ch.goal {
            Goal::Race => {
                let n = self.arena.cp_centres.len();
                let r = &mut self.rovers[0].rover;
                if r.next_cp < n && self.arena.cp_at(r.x, r.y) as usize == r.next_cp + 1 {
                    r.next_cp += 1;
                    let msg = if r.next_cp == n { format!("Finish! {secs:.2} s") } else { format!("Checkpoint {} of {n} at {secs:.1} s", r.next_cp) };
                    self.events.push((self.tick, msg));
                    if r.next_cp == n {
                        self.outcome = Outcome::Won(self.tick);
                    }
                }
            }
            Goal::Maze => {
                let r = &self.rovers[0].rover;
                if self.arena.exit_at(r.x, r.y) {
                    self.events.push((self.tick, format!("Through the hatch! {secs:.2} s")));
                    self.outcome = Outcome::Won(self.tick);
                }
            }
            Goal::Sumo => {
                let me = self.rovers[0].rover.fell.is_some();
                let them = self.rovers.get(1).map(|e| e.rover.fell.is_some()).unwrap_or(false);
                if me {
                    self.outcome = Outcome::Lost("pushed out of the ring");
                } else if them {
                    self.events.push((self.tick, format!("Tumbler is out! {secs:.2} s")));
                    self.outcome = Outcome::Won(self.tick);
                }
            }
        }
        if self.outcome == Outcome::Running && secs >= self.ch.limit {
            self.outcome = if self.ch.goal == Goal::Sumo { Outcome::Draw } else { Outcome::Lost("out of time") };
            self.events.push((self.tick, "Time!".to_string()));
        }
    }

    /// Runs until the challenge ends or `max_ticks` pass.
    #[cfg(test)]
    pub fn run(&mut self, max_ticks: u32) {
        for _ in 0..max_ticks {
            if self.outcome != Outcome::Running {
                break;
            }
            self.step();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ch: &str, src: &str) -> Sim {
        let d = Design::from_text(src).unwrap();
        let mut s = Sim::new(challenge::by_id(ch).unwrap(), d);
        s.run(u32::MAX);
        s
    }

    use crate::design::COMET as RACER;

    /// Follows the right-hand wall: turns left when the nose sees a wall,
    /// gently right when the right side opens up.
    const FOLLOWER: &str = "icewire rover 1
name Follower
part 1 ranger at 4 2 mount nose
part 2 compare at 20 2 mode < value 0.9
part 3 ranger at 4 9 mount right
part 4 compare at 20 9 mode > value 1.1
part 5 number at 4 16 value 1
part 6 not at 20 16
part 7 and at 36 9
part 8 track-left at 60 2 value 70
part 9 track-right at 60 8 value 70
part 10 gyro at 60 14 value 60
wire 1 to 2 a
wire 3 to 4 a
wire 2 to 6 in
wire 4 to 7 a
wire 6 to 7 b
wire 5 to 8 fwd
wire 5 to 9 fwd
wire 7 to 10 right
wire 2 to 10 left
";

    #[test]
    fn same_rover_same_run() {
        let a = run("ring-hall", RACER);
        let b = run("ring-hall", RACER);
        assert_eq!(a.tick, b.tick);
        let (ra, rb) = (&a.rovers[0].rover, &b.rovers[0].rover);
        assert_eq!((ra.x.to_bits(), ra.y.to_bits(), ra.h.to_bits()), (rb.x.to_bits(), rb.y.to_bits(), rb.h.to_bits()));
    }

    #[test]
    fn starter_finishes_the_race() {
        let s = run("ring-hall", crate::design::STARTER);
        assert!(matches!(s.outcome, Outcome::Won(_)), "{:?} at {:.1} s, cp {}", s.outcome, s.secs(), s.rovers[0].rover.next_cp);
        assert_eq!(s.ch.stars(s.secs()), 1, "Homer leaves room to improve");
        eprintln!("starter race {:.1} s", s.secs());
    }

    #[test]
    fn raw_power_earns_two_stars_not_three() {
        let brute = crate::design::STARTER.replace("value 60", "value 100").replace("gyro at 34 14 value 2", "gyro at 34 14 value 4");
        let s = run("ring-hall", &brute);
        assert!(matches!(s.outcome, Outcome::Won(_)));
        assert_eq!(s.ch.stars(s.secs()), 2, "{:.1} s", s.secs());
    }

    #[test]
    fn racer_beats_par() {
        let s = run("ring-hall", RACER);
        assert!(matches!(s.outcome, Outcome::Won(_)), "{:?} cp {}", s.outcome, s.rovers[0].rover.next_cp);
        assert_eq!(s.ch.stars(s.secs()), 3, "three stars are reachable");
        eprintln!("racer race {:.1} s", s.secs());
    }

    #[test]
    fn maze_can_be_solved() {
        let s = run("cold-stores", FOLLOWER);
        let r = &s.rovers[0].rover;
        assert!(matches!(s.outcome, Outcome::Won(_)), "{:?} at ({:.1},{:.1})", s.outcome, r.x, r.y);
        assert_eq!(s.ch.stars(s.secs()), 3, "three stars are reachable");
        eprintln!("follower maze {:.1} s", s.secs());
    }

    #[test]
    fn starter_loses_the_puck_and_a_stronger_rover_wins() {
        let s = run("puck", crate::design::STARTER);
        assert!(!matches!(s.outcome, Outcome::Won(_)), "{:?}", s.outcome);
        let strong = crate::design::STARTER.replace("value 60", "value 100").replace("gyro at 34 14 value 2", "gyro at 34 14 value 4");
        let s = run("puck", &strong);
        assert!(matches!(s.outcome, Outcome::Won(_)), "{:?}", s.outcome);
        assert_eq!(s.ch.stars(s.secs()), 3, "three stars are reachable");
        eprintln!("strong puck {:.1} s", s.secs());
    }
}

#[cfg(test)]
mod trace {
    use super::*;
    #[test]
    #[ignore]
    fn trace_maze() {
        let src = std::fs::read_to_string(std::env::var("ROVER").unwrap()).unwrap();
        let ch = std::env::var("CH").unwrap_or("cold-stores".into());
        let mut s = Sim::new(challenge::by_id(&ch).unwrap(), Design::from_text(&src).unwrap());
        for t in 0..u32::MAX {
            if s.outcome != Outcome::Running { break; }
            s.step();
            if t % 30 == 0 {
                let r = &s.rovers[0].rover;
                eprintln!("{:6.1}s ({:5.2},{:5.2}) h {:5.0} v {:4.2} drive {:?} out {:?}", s.secs(), r.x, r.y, r.heading_deg(), r.speed(), r.drive, s.rovers[0].brain.out);
            }
        }
        eprintln!("{:?} {:.1}", s.outcome, s.secs());
    }
}
