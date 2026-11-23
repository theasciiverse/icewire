//! Replays: what happened in a run, frame by frame, as plain text. Bots
//! need not be deterministic, so a replay keeps positions, not inputs.
//!
//! ```text
//! icewire replay 1
//! challenge boiler-room
//! rover 0 Dodger
//! rover 1 Sprocket
//! f 2                       a frame, at this tick
//! r 0 x y heading charge a  a rover: active, s(talled) or f(allen)
//! p x y vx vy owner         a pellet in flight
//! l x0 y0 x1 y1 t flight    a lob in flight
//! s x y tick 0|1            a splash (1 for a lob's)
//! e tick text               a line for the feed
//! result text
//! ```

use crate::challenge;
use crate::design::Design;
use crate::sim::{Lob, Pellet, Sim};

pub const FORMAT: &str = "icewire replay 1";
/// A frame every this many ticks.
pub const EVERY: u32 = 2;

fn f3(v: f64) -> String {
    format!("{:.3}", v)
}

/// Collects a run as it happens.
pub struct Recorder {
    text: String,
    events: usize,
}

impl Recorder {
    pub fn new(sim: &Sim) -> Recorder {
        let mut text = format!("{FORMAT}\nchallenge {}\n", sim.ch.id);
        for (i, e) in sim.rovers.iter().enumerate() {
            text.push_str(&format!("rover {i} {}\n", e.design.name));
        }
        let mut r = Recorder { text, events: 0 };
        r.frame(sim);
        r
    }

    /// Call after every step; keeps every EVERY-th tick.
    pub fn frame(&mut self, sim: &Sim) {
        while self.events < sim.events.len() {
            let (t, m) = &sim.events[self.events];
            self.text.push_str(&format!("e {t} {m}\n"));
            self.events += 1;
        }
        if !sim.tick.is_multiple_of(EVERY) && sim.outcome == crate::sim::Outcome::Running {
            return;
        }
        let t = &mut self.text;
        t.push_str(&format!("f {}\n", sim.tick));
        for (i, e) in sim.rovers.iter().enumerate() {
            let r = &e.rover;
            let state = if r.fell.is_some() {
                'f'
            } else if r.stalled.is_some() {
                's'
            } else {
                'a'
            };
            t.push_str(&format!("r {i} {} {} {:.4} {} {state}\n", f3(r.x), f3(r.y), r.h, f3(r.charge)));
        }
        for p in &sim.pellets {
            t.push_str(&format!("p {} {} {} {} {}\n", f3(p.x), f3(p.y), f3(p.vx), f3(p.vy), p.owner));
        }
        for l in &sim.lobs {
            t.push_str(&format!("l {} {} {} {} {} {}\n", f3(l.x0), f3(l.y0), f3(l.x1), f3(l.y1), l.t, l.flight));
        }
        for s in &sim.splashes {
            t.push_str(&format!("s {} {} {} {}\n", f3(s.0), f3(s.1), s.2, s.3 as u8));
        }
    }

    pub fn finish(mut self, result: &str) -> String {
        self.text.push_str(&format!("result {result}\n"));
        self.text
    }
}

#[derive(Clone, Default)]
pub struct Frame {
    pub tick: u32,
    pub rovers: Vec<(f64, f64, f64, f64, char)>,
    pub pellets: Vec<Pellet>,
    pub lobs: Vec<Lob>,
    pub splashes: Vec<(f64, f64, u32, bool)>,
}

pub struct Replay {
    pub challenge: usize,
    pub names: Vec<String>,
    pub frames: Vec<Frame>,
    pub events: Vec<(u32, String)>,
    pub result: String,
}

impl Replay {
    pub fn parse(text: &str) -> Result<Replay, String> {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(FORMAT) {
            return Err("not an Icewire replay".into());
        }
        let mut rp = Replay { challenge: 0, names: Vec::new(), frames: Vec::new(), events: Vec::new(), result: String::new() };
        let mut cur: Option<Frame> = None;
        for (n, line) in lines.enumerate() {
            let err = || format!("line {}: cannot read {line:?}", n + 2);
            let mut w = line.split_whitespace();
            let f = |w: &mut std::str::SplitWhitespace| -> Result<f64, String> { w.next().and_then(|x| x.parse().ok()).ok_or_else(err) };
            match w.next() {
                Some("challenge") => rp.challenge = w.next().and_then(challenge::by_id).ok_or_else(err)?,
                Some("rover") => {
                    w.next();
                    rp.names.push(w.collect::<Vec<_>>().join(" "));
                }
                Some("f") => {
                    if let Some(fr) = cur.take() {
                        rp.frames.push(fr);
                    }
                    cur = Some(Frame { tick: f(&mut w)? as u32, ..Frame::default() });
                }
                Some("r") => {
                    w.next();
                    let (x, y, h, c) = (f(&mut w)?, f(&mut w)?, f(&mut w)?, f(&mut w)?);
                    let s = w.next().and_then(|s| s.chars().next()).unwrap_or('a');
                    cur.as_mut().ok_or_else(err)?.rovers.push((x, y, h, c, s));
                }
                Some("p") => {
                    let p = Pellet { x: f(&mut w)?, y: f(&mut w)?, vx: f(&mut w)?, vy: f(&mut w)?, owner: f(&mut w)? as usize, flown: 0.0 };
                    cur.as_mut().ok_or_else(err)?.pellets.push(p);
                }
                Some("l") => {
                    let l = Lob { x0: f(&mut w)?, y0: f(&mut w)?, x1: f(&mut w)?, y1: f(&mut w)?, t: f(&mut w)? as u32, flight: f(&mut w)? as u32 };
                    cur.as_mut().ok_or_else(err)?.lobs.push(l);
                }
                Some("s") => {
                    let s = (f(&mut w)?, f(&mut w)?, f(&mut w)? as u32, f(&mut w)? != 0.0);
                    cur.as_mut().ok_or_else(err)?.splashes.push(s);
                }
                Some("e") => {
                    let t = f(&mut w)? as u32;
                    rp.events.push((t, w.collect::<Vec<_>>().join(" ")));
                }
                Some("result") => rp.result = w.collect::<Vec<_>>().join(" "),
                None => {}
                Some(_) => return Err(err()),
            }
        }
        if let Some(fr) = cur.take() {
            rp.frames.push(fr);
        }
        if rp.frames.is_empty() || rp.names.is_empty() {
            return Err("the replay is empty".into());
        }
        Ok(rp)
    }

    /// A sim to draw the replay with: the right arena and names, no brains.
    pub fn stage(&self) -> Sim {
        let names = self.names.iter().map(|n| Design::blank(n)).map(|mut d| {
            d.parts.clear();
            d
        });
        Sim::with(self.challenge, names.collect(), crate::sim::BOT_TIMEOUT)
    }

    /// Puts frame `i` on the stage.
    pub fn show(&self, i: usize, sim: &mut Sim) {
        let Some(fr) = self.frames.get(i) else { return };
        sim.tick = fr.tick;
        for (k, &(x, y, h, c, s)) in fr.rovers.iter().enumerate() {
            if let Some(e) = sim.rovers.get_mut(k) {
                let r = &mut e.rover;
                if i > 0 && (r.trail.back() != Some(&(x, y))) && fr.tick.is_multiple_of(4) {
                    r.trail.push_back((x, y));
                    if r.trail.len() > 160 {
                        r.trail.pop_front();
                    }
                }
                r.x = x;
                r.y = y;
                r.h = h;
                r.charge = c;
                r.stalled = (s == 's').then_some(fr.tick);
                r.fell = (s == 'f').then_some(fr.tick);
                r.drive = Default::default();
            }
        }
        sim.pellets = fr.pellets.clone();
        sim.lobs = fr.lobs.clone();
        sim.splashes = fr.splashes.clone();
        sim.events = self.events.iter().filter(|e| e.0 <= fr.tick).cloned().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{KETTLE, SLEET};

    #[test]
    fn a_recorded_battle_reads_back() {
        let mut s = Sim::with(challenge::by_id("laundry").unwrap(), vec![Design::from_text(SLEET).unwrap(), Design::from_text(KETTLE).unwrap()], crate::sim::BOT_TIMEOUT);
        let mut rec = Recorder::new(&s);
        while s.outcome == crate::sim::Outcome::Running {
            s.step();
            rec.frame(&s);
        }
        let text = rec.finish("Sleet wins");
        let rp = Replay::parse(&text).unwrap();
        assert_eq!(rp.names, ["Sleet", "Kettle"]);
        assert_eq!(rp.result, "Sleet wins");
        let last = rp.frames.last().unwrap();
        assert_eq!(last.tick, s.tick);
        assert!((last.rovers[1].3 - s.rovers[1].rover.charge).abs() < 1e-3);
        let mut stage = rp.stage();
        for i in 0..rp.frames.len() {
            rp.show(i, &mut stage);
        }
        assert_eq!(stage.tick, s.tick);
        assert!(stage.events.len() > 2);
        assert!(Replay::parse("hello").is_err());
    }
}
