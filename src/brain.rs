//! Runs a rover's wiring one tick at a time. Parts are evaluated in wire
//! order (sources before what they feed); a loop of wires reads the value
//! its part had on the previous tick, so feedback works and stays
//! deterministic.

use crate::design::Design;
use crate::parts::{Category, Kind};
use crate::sim::TICKS_PER_SEC;

/// What the drive parts ask for, each from -1 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drive {
    pub left: f64,
    pub right: f64,
    pub gyro: f64,
}

#[derive(Clone, Debug)]
pub struct Brain {
    /// Evaluation order, as indexes into the design's parts.
    order: Vec<usize>,
    /// For each part and input, the index of the part feeding it.
    feeds: Vec<[Option<usize>; 2]>,
    /// Each part's output this tick.
    pub out: Vec<f64>,
    /// Each part's inputs this tick, for the live bench.
    pub ins: Vec<[f64; 2]>,
    last_in: Vec<f64>,
    timer: Vec<u32>,
    toggle: Vec<bool>,
}

fn on(v: f64) -> bool {
    v != 0.0
}

fn bit(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

impl Brain {
    pub fn new(d: &Design) -> Brain {
        let n = d.parts.len();
        let index = |id: u32| d.parts.iter().position(|p| p.id == id);
        let mut feeds = vec![[None, None]; n];
        for w in &d.wires {
            if let (Some(f), Some(t)) = (index(w.from), index(w.to))
                && w.port < 2
            {
                feeds[t][w.port] = Some(f);
            }
        }
        // Kahn's algorithm, lowest index first; what is left is in loops.
        let mut waiting: Vec<usize> = feeds.iter().map(|f| f.iter().flatten().count()).collect();
        let mut done = vec![false; n];
        let mut order = Vec::with_capacity(n);
        while let Some(i) = (0..n).find(|&i| !done[i] && waiting[i] == 0) {
            done[i] = true;
            order.push(i);
            for (t, f) in feeds.iter().enumerate() {
                for src in f.iter().flatten() {
                    if *src == i {
                        waiting[t] -= 1;
                    }
                }
            }
        }
        order.extend((0..n).filter(|&i| !done[i]));
        Brain { order, feeds, out: vec![0.0; n], ins: vec![[0.0; 2]; n], last_in: vec![0.0; n], timer: vec![0; n], toggle: vec![false; n] }
    }

    /// One tick. `sense` reads sensor part `i` from the world.
    pub fn step(&mut self, d: &Design, mut sense: impl FnMut(usize) -> f64) -> Drive {
        let mut drive = Drive::default();
        for k in 0..self.order.len() {
            let i = self.order[k];
            let p = &d.parts[i];
            let a = self.feeds[i][0].map(|s| self.out[s]).unwrap_or(0.0);
            let b_wired = self.feeds[i][1].is_some();
            let b = self.feeds[i][1].map(|s| self.out[s]).unwrap_or(if p.kind == Kind::Compare { p.value } else { 0.0 });
            self.ins[i] = [a, if b_wired || p.kind == Kind::Compare { b } else { 0.0 }];
            let rising = on(a) && !on(self.last_in[i]);
            self.last_in[i] = a;
            let v = match p.kind {
                k if k.category() == Category::Sensor => sense(i),
                // with 0s and 1s these are plain AND and OR; with other
                // numbers AND lets b through while a is on, and OR gives
                // a when it is on, else b
                Kind::And => {
                    if on(a) { b } else { 0.0 }
                }
                Kind::Or => {
                    if on(a) { a } else { b }
                }
                Kind::Not => bit(!on(a)),
                Kind::Compare => {
                    let (a1, b1) = ((a * 10.0).round(), (b * 10.0).round());
                    bit(match p.mode {
                        0 => a > b,
                        1 => a < b,
                        _ => a1 == b1,
                    })
                }
                Kind::Timer => {
                    if rising {
                        self.timer[i] = (p.value * TICKS_PER_SEC as f64).round().max(1.0) as u32;
                    }
                    if self.timer[i] > 0 {
                        self.timer[i] -= 1;
                        1.0
                    } else {
                        0.0
                    }
                }
                Kind::Toggle => {
                    if rising {
                        self.toggle[i] = !self.toggle[i];
                    }
                    bit(self.toggle[i])
                }
                Kind::Number => p.value,
                Kind::TrackLeft | Kind::TrackRight | Kind::Gyro => {
                    let v = ((a - b) * p.value / 100.0).clamp(-1.0, 1.0);
                    match p.kind {
                        Kind::TrackLeft => drive.left = v,
                        Kind::TrackRight => drive.right = v,
                        _ => drive.gyro = v,
                    }
                    v
                }
                _ => 0.0,
            };
            self.out[i] = if v.is_finite() { v } else { 0.0 };
        }
        drive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::Design;

    fn rover(src: &str) -> Design {
        Design::from_text(&format!("icewire rover 1\n{src}")).unwrap()
    }

    #[test]
    fn logic_parts() {
        let d = rover(
            "part 1 number at 2 2 value 1
part 2 number at 2 8 value 0
part 3 and at 20 2
part 4 or at 20 8
part 5 not at 20 14
part 6 compare at 20 20 mode > value 0.5
wire 1 to 3 a
wire 2 to 3 b
wire 1 to 4 a
wire 2 to 4 b
wire 2 to 5 in
wire 1 to 6 a
",
        );
        let mut b = Brain::new(&d);
        b.step(&d, |_| 0.0);
        assert_eq!(&b.out[2..6], &[0.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn timer_holds_and_toggle_flips() {
        let d = rover(
            "part 1 bumper at 2 2
part 2 timer at 20 2 value 0.1
part 3 toggle at 20 8
wire 1 to 2 in
wire 1 to 3 in
",
        );
        let mut b = Brain::new(&d);
        let pulses = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0];
        let mut timer = Vec::new();
        let mut toggle = Vec::new();
        for p in pulses {
            b.step(&d, |_| p);
            timer.push(b.out[1]);
            toggle.push(b.out[2]);
        }
        // 0.1 s is six ticks
        assert_eq!(timer, [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0]);
        assert_eq!(toggle, [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn drive_scales_and_clamps() {
        let d = Design::from_text(crate::design::STARTER).unwrap();
        let mut b = Brain::new(&d);
        let drive = b.step(&d, |_| 90.0);
        assert_eq!(drive, Drive { left: 0.6, right: 0.6, gyro: 1.0 });
    }

    #[test]
    fn loops_run() {
        // a NOT feeding itself through an OR blinks every tick
        let d = rover(
            "part 1 not at 2 2
part 2 or at 20 2
wire 2 to 1 in
wire 1 to 2 a
",
        );
        let mut b = Brain::new(&d);
        let mut seen = Vec::new();
        for _ in 0..4 {
            b.step(&d, |_| 0.0);
            seen.push(b.out[0]);
        }
        assert_eq!(seen, [1.0, 0.0, 1.0, 0.0]);
    }
}
