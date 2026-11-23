//! A rover's wiring: its parts, where they sit on the bench, and the wires
//! between them. Saved as a plain text file anyone can read and edit.

use crate::parts::{self, BOX_H, BOX_W, Category, Kind, Mount};

pub const BENCH_W: i32 = 150;
pub const BENCH_H: i32 = 60;
pub const FORMAT: &str = "icewire rover 1";

#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub id: u32,
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub mount: Mount,
    pub mode: usize,
    pub value: f64,
}

impl Part {
    pub fn new(id: u32, kind: Kind, x: i32, y: i32) -> Part {
        Part { id, kind, x, y, mount: kind.default_mount(), mode: 0, value: kind.value().map(|v| v.default).unwrap_or(0.0) }
    }

    /// The cell just right of the output port, where its wires start.
    pub fn out_cell(&self) -> (i32, i32) {
        (self.x + BOX_W, self.y + 1)
    }

    /// The cell just left of input port `port`, where a wire ends.
    pub fn in_cell(&self, port: usize) -> (i32, i32) {
        (self.x - 1, self.y + 1 + port as i32)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + BOX_W && y >= self.y && y < self.y + BOX_H
    }

    /// The settings in a few characters, for the box.
    pub fn label(&self, b_wired: bool) -> String {
        let val = || parts::fmt_num(self.value);
        match self.kind {
            Kind::Bumper | Kind::Ranger => self.mount.name().to_string(),
            Kind::Whisker => format!("{} {}m", self.mount.name(), val()),
            Kind::Compass => parts::COMPASS_MODES[self.mode].to_string(),
            Kind::Radar => parts::RADAR_MODES[self.mode].to_string(),
            Kind::Gauge => "charge".to_string(),
            Kind::Lobber => {
                if b_wired { "aimed".to_string() } else { format!("{}m", val()) }
            }
            Kind::Compare => {
                let op = parts::COMPARE_MODES[self.mode];
                if b_wired { format!("{op} b") } else { format!("{op} {}", val()) }
            }
            Kind::Timer => format!("{}s", val()),
            Kind::Number => val(),
            Kind::TrackLeft | Kind::TrackRight | Kind::Gyro => format!("x{}%", val()),
            _ => String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    pub from: u32,
    pub to: u32,
    pub port: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub name: String,
    pub parts: Vec<Part>,
    pub wires: Vec<Wire>,
}

impl Design {
    /// A bare Mule: just its drive, waiting for wiring.
    pub fn blank(name: &str) -> Design {
        let mut d = Design { name: name.to_string(), parts: Vec::new(), wires: Vec::new() };
        let _ = d.add(Kind::TrackLeft, 60, 2);
        let _ = d.add(Kind::TrackRight, 60, 8);
        let _ = d.add(Kind::Gyro, 60, 14);
        d
    }

    pub fn part(&self, id: u32) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }

    pub fn part_mut(&mut self, id: u32) -> Option<&mut Part> {
        self.parts.iter_mut().find(|p| p.id == id)
    }

    pub fn part_at(&self, x: i32, y: i32) -> Option<u32> {
        self.parts.iter().find(|p| p.contains(x, y)).map(|p| p.id)
    }

    fn next_id(&self) -> u32 {
        self.parts.iter().map(|p| p.id).max().unwrap_or(0) + 1
    }

    pub fn count(&self, cat: Category) -> usize {
        self.parts.iter().filter(|p| p.kind.category() == cat).count()
    }

    /// Room for one more of this kind on the chassis?
    pub fn room_for(&self, kind: Kind) -> Result<(), String> {
        match kind.category() {
            Category::Sensor if self.count(Category::Sensor) >= parts::SENSOR_SLOTS => {
                Err(format!("The Mule has {} sensor slots, all taken", parts::SENSOR_SLOTS))
            }
            Category::Logic if self.count(Category::Logic) >= parts::LOGIC_SLOTS => {
                Err(format!("The Mule's logic board holds {} parts, all taken", parts::LOGIC_SLOTS))
            }
            Category::Drive if self.parts.iter().any(|p| p.kind == kind) => Err(format!("The Mule has one {}, already fitted", kind.name().to_lowercase())),
            Category::Weapon if self.parts.iter().any(|p| p.kind == kind) => Err(format!("The Mule has one {} mount, already taken", kind.name().to_lowercase())),
            _ => Ok(()),
        }
    }

    /// Is there room on the bench for a box at x, y (ignoring part `skip`)?
    /// Boxes keep a column for wires on either side and a row between them.
    pub fn fits(&self, x: i32, y: i32, skip: Option<u32>) -> bool {
        if x < 1 || y < 0 || x + BOX_W + 1 > BENCH_W || y + BOX_H > BENCH_H {
            return false;
        }
        self.parts.iter().filter(|p| Some(p.id) != skip).all(|p| x + BOX_W + 2 <= p.x || p.x + BOX_W + 2 <= x || y + BOX_H < p.y || p.y + BOX_H < y)
    }

    /// The nearest free spot to x, y, searching outwards.
    pub fn free_spot(&self, x: i32, y: i32, skip: Option<u32>) -> Option<(i32, i32)> {
        for r in 0..BENCH_W {
            for dy in -r..=r {
                for dx in [-r, r] {
                    if self.fits(x + dx, y + dy, skip) {
                        return Some((x + dx, y + dy));
                    }
                }
                if dy == -r || dy == r {
                    for dx in -r..=r {
                        if self.fits(x + dx, y + dy, skip) {
                            return Some((x + dx, y + dy));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn add(&mut self, kind: Kind, x: i32, y: i32) -> Result<u32, String> {
        self.room_for(kind)?;
        let (x, y) = self.free_spot(x, y, None).ok_or("The bench is full")?;
        let id = self.next_id();
        self.parts.push(Part::new(id, kind, x, y));
        Ok(id)
    }

    pub fn remove(&mut self, id: u32) {
        self.parts.retain(|p| p.id != id);
        self.wires.retain(|w| w.from != id && w.to != id);
    }

    pub fn wire_into(&self, to: u32, port: usize) -> Option<usize> {
        self.wires.iter().position(|w| w.to == to && w.port == port)
    }

    /// Wires from's output into to's input `port`, replacing what was there.
    pub fn connect(&mut self, from: u32, to: u32, port: usize) -> Result<(), String> {
        let (Some(f), Some(t)) = (self.part(from), self.part(to)) else { return Err("No such part".into()) };
        if !f.kind.has_output() {
            return Err(format!("The {} has no output", f.kind.name().to_lowercase()));
        }
        if port >= t.kind.inputs().len() {
            return Err(format!("The {} has no input there", t.kind.name().to_lowercase()));
        }
        if from == to {
            return Err("A part cannot feed itself".into());
        }
        if let Some(i) = self.wire_into(to, port) {
            self.wires.remove(i);
        }
        self.wires.push(Wire { from, to, port });
        Ok(())
    }

    /// The text file.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str("# An Icewire rover. One part or wire per line; edit freely.\n");
        s.push_str("# part <id> <kind> at <x> <y> [mount <side>] [mode <mode>] [value <n>]\n");
        s.push_str("# wire <from id> to <to id> <input>\n");
        s.push_str(FORMAT);
        s.push('\n');
        s.push_str(&format!("name {}\n", self.name));
        for p in &self.parts {
            s.push_str(&format!("part {} {} at {} {}", p.id, p.kind.key(), p.x, p.y));
            if !p.kind.mounts().is_empty() {
                s.push_str(&format!(" mount {}", p.mount.name()));
            }
            if !p.kind.modes().is_empty() {
                s.push_str(&format!(" mode {}", p.kind.modes()[p.mode]));
            }
            if p.kind.value().is_some() {
                s.push_str(&format!(" value {}", p.value));
            }
            s.push('\n');
        }
        for w in &self.wires {
            let port = self.part(w.to).map(|p| p.kind.inputs()[w.port]).unwrap_or("in");
            s.push_str(&format!("wire {} to {} {}\n", w.from, w.to, port));
        }
        s
    }

    pub fn from_text(text: &str) -> Result<Design, String> {
        let mut d = Design { name: "Unnamed".into(), parts: Vec::new(), wires: Vec::new() };
        let mut seen_header = false;
        let mut wires = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            let err = |m: &str| format!("line {}: {m}", n + 1);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("icewire rover") {
                if line != FORMAT {
                    return Err(err("made by a newer Icewire"));
                }
                seen_header = true;
                continue;
            }
            let w: Vec<&str> = line.split_whitespace().collect();
            match w[0] {
                "name" => d.name = clean_name(line[4..].trim()),
                "part" => {
                    if w.len() < 6 || w[3] != "at" {
                        return Err(err("part <id> <kind> at <x> <y>"));
                    }
                    let id: u32 = w[1].parse().map_err(|_| err("bad part id"))?;
                    let kind = Kind::parse(w[2]).ok_or_else(|| err(&format!("no part called {}", w[2])))?;
                    let x: i32 = w[4].parse().map_err(|_| err("bad x"))?;
                    let y: i32 = w[5].parse().map_err(|_| err("bad y"))?;
                    if d.part(id).is_some() {
                        return Err(err("two parts with one id"));
                    }
                    let mut p = Part::new(id, kind, x.clamp(1, BENCH_W - BOX_W - 1), y.clamp(0, BENCH_H - BOX_H));
                    let mut i = 6;
                    while i + 1 < w.len() {
                        match w[i] {
                            "mount" => p.mount = Mount::parse(w[i + 1]).filter(|m| kind.mounts().contains(m)).ok_or_else(|| err("bad mount"))?,
                            "mode" => p.mode = kind.modes().iter().position(|m| *m == w[i + 1]).ok_or_else(|| err("bad mode"))?,
                            "value" => {
                                let v: f64 = w[i + 1].parse().map_err(|_| err("bad value"))?;
                                if let Some(spec) = kind.value() {
                                    p.value = if v.is_finite() { v.clamp(spec.min, spec.max) } else { spec.default };
                                }
                            }
                            other => return Err(err(&format!("unknown setting {other}"))),
                        }
                        i += 2;
                    }
                    d.parts.push(p);
                }
                "wire" => {
                    if w.len() != 5 || w[2] != "to" {
                        return Err(err("wire <from> to <to> <input>"));
                    }
                    let from: u32 = w[1].parse().map_err(|_| err("bad wire source"))?;
                    let to: u32 = w[3].parse().map_err(|_| err("bad wire target"))?;
                    wires.push((n + 1, from, to, w[4].to_string()));
                }
                other => return Err(err(&format!("unknown line {other}"))),
            }
        }
        if !seen_header {
            return Err(format!("not a rover file (no \"{FORMAT}\" line)"));
        }
        for (line, from, to, port) in wires {
            let p = d.part(to).ok_or(format!("line {line}: no part {to}"))?;
            let port = p.kind.inputs().iter().position(|i| *i == port).ok_or(format!("line {line}: the {} has no input {port}", p.kind.name()))?;
            d.connect(from, to, port).map_err(|e| format!("line {line}: {e}"))?;
        }
        Ok(d)
    }
}

/// Rover names: printable, short, no path characters.
pub fn clean_name(s: &str) -> String {
    let s: String = s.chars().filter(|c| c.is_alphanumeric() || " -_'.".contains(*c)).take(20).collect();
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() { "Unnamed".into() } else { s }
}

/// The rover new players find in their garage: it heads for the beacon
/// at a steady pace and nothing more.
pub const STARTER: &str = "icewire rover 1
name Homer
part 1 compass at 4 3 mode bearing
part 2 number at 4 10 value 1
part 3 track-left at 34 2 value 60
part 4 track-right at 34 8 value 60
part 5 gyro at 34 14 value 2
wire 2 to 3 fwd
wire 2 to 4 fwd
wire 1 to 5 right
";

/// Sami's rover for the Puck: turns to face the other rover and shoves.
pub const TUMBLER: &str = "icewire rover 1
name Tumbler
part 1 compass at 4 2 mode bearing
part 2 number at 4 9 value 1
part 3 track-left at 40 2 value 85
part 4 track-right at 40 9 value 85
part 5 gyro at 40 16 value 3
wire 1 to 5 right
wire 2 to 3 fwd
wire 2 to 4 fwd
";

/// Ruben's rover for the boiler room: wanders in wide circles, shies
/// from walls and fires whenever another rover crosses its nose.
pub const SPROCKET: &str = "icewire rover 1
name Sprocket
part 1 radar at 4 2 mode bearing
part 2 compare at 22 2 mode > value -12
part 3 compare at 22 9 mode < value 12
part 4 and at 40 5
part 5 pellet-gun at 58 2
part 6 number at 4 16 value 1
part 7 track-left at 58 9 value 70
part 8 track-right at 58 16 value 50
part 9 whisker at 4 23 mount nose value 1
part 10 gyro at 58 23 value 100
wire 1 to 2 a
wire 1 to 3 a
wire 2 to 4 a
wire 3 to 4 b
wire 4 to 5 fire
wire 6 to 7 fwd
wire 6 to 8 fwd
wire 9 to 10 left
";

/// Ines's rover for laundry night: turns to face, keeps three or four
/// metres away, fires whenever it is aimed and lobs when it is far.
pub const KETTLE: &str = "icewire rover 1
name Kettle
part 1 radar at 4 2 mode bearing
part 2 radar at 4 9 mode distance
part 3 compare at 22 9 mode > value 4
part 4 compare at 22 16 mode < value 3
part 5 compare at 22 2 mode > value -10
part 6 compare at 40 2 mode < value 10
part 7 and at 40 9
part 8 track-left at 58 2 value 60
part 9 track-right at 58 9 value 60
part 10 gyro at 58 16 value 3
part 11 pellet-gun at 58 23
part 12 and at 40 16
part 13 lobber at 58 30
wire 3 to 12 a
wire 7 to 12 b
wire 12 to 13 fire
wire 2 to 13 range
wire 1 to 10 right
wire 1 to 5 a
wire 1 to 6 a
wire 5 to 7 a
wire 6 to 7 b
wire 7 to 11 fire
wire 2 to 3 a
wire 2 to 4 a
wire 3 to 8 fwd
wire 3 to 9 fwd
wire 4 to 8 back
wire 4 to 9 back
";

/// The dome's mystery rover: fires whenever it is aimed, lobs from range,
/// presses in while its charge is high and backs away when it is low.
pub const NIGHTJAR: &str = "icewire rover 1
name Nightjar
part 1 radar at 4 2 mode bearing
part 2 radar at 4 9 mode distance
part 3 compare at 22 2 mode > value -8
part 4 compare at 40 2 mode < value 8
part 5 and at 58 2
part 6 compare at 22 9 mode > value 3
part 7 and at 40 9
part 10 gauge at 4 16
part 11 compare at 22 23 mode > value 40
part 12 compare at 22 30 mode > value 2.5
part 13 and at 40 23
part 14 not at 40 30
part 15 track-left at 76 2 value 70
part 16 track-right at 76 9 value 60
part 17 gyro at 76 16 value 4
part 18 pellet-gun at 76 23
part 19 lobber at 76 30
wire 1 to 17 right
wire 1 to 3 a
wire 1 to 4 a
wire 3 to 5 a
wire 4 to 5 b
wire 2 to 6 a
wire 6 to 7 a
wire 5 to 7 b
wire 7 to 19 fire
wire 2 to 19 range
wire 5 to 18 fire
wire 10 to 11 a
wire 2 to 12 a
wire 11 to 13 a
wire 12 to 13 b
wire 13 to 15 fwd
wire 13 to 16 fwd
wire 11 to 14 in
wire 14 to 15 back
wire 14 to 16 back
";

/// The demo's fighter: heads for the other rover firing pellets, and lobs
/// slush at whatever distance the compass gives.
pub const SLEET: &str = "icewire rover 1
name Sleet
part 1 compass at 4 2 mode bearing
part 2 number at 4 9 value 1
part 3 compass at 4 16 mode distance
part 4 track-left at 34 2 value 70
part 5 track-right at 34 8 value 70
part 6 gyro at 34 14 value 3
part 7 pellet-gun at 34 20
part 8 lobber at 34 26
wire 2 to 4 fwd
wire 2 to 5 fwd
wire 1 to 6 right
wire 2 to 7 fire
wire 2 to 8 fire
wire 3 to 8 range
";

/// The demo's rover: steers for the beacon at full power and eases off
/// near it, so it makes the corners.
pub const COMET: &str = "icewire rover 1
name Comet
part 1 compass at 4 2 mode bearing
part 2 compass at 4 9 mode distance
part 3 compare at 22 9 mode > value 3
part 4 number at 4 16 value 0.4
part 5 or at 40 9
part 6 track-left at 58 2 value 100
part 7 track-right at 58 9 value 100
part 8 gyro at 58 16 value 8
wire 1 to 8 right
wire 2 to 3 a
wire 3 to 5 a
wire 4 to 5 b
wire 5 to 6 fwd
wire 5 to 7 fwd
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips() {
        for src in [STARTER, TUMBLER, COMET, SPROCKET, KETTLE, NIGHTJAR] {
            let d = Design::from_text(src).unwrap();
            let again = Design::from_text(&d.to_text()).unwrap();
            assert_eq!(d, again);
        }
    }

    #[test]
    fn errors_name_the_line() {
        let e = Design::from_text("icewire rover 1\npart 1 laser at 3 3\n").unwrap_err();
        assert!(e.starts_with("line 2"), "{e}");
        assert!(Design::from_text("part 1 not at 3 3\n").is_err());
    }

    #[test]
    fn chassis_limits() {
        let mut d = Design::blank("t");
        assert!(d.add(Kind::Gyro, 0, 0).is_err());
        for _ in 0..parts::SENSOR_SLOTS {
            d.add(Kind::Bumper, 2, 2).unwrap();
        }
        assert!(d.add(Kind::Ranger, 2, 2).is_err());
    }

    #[test]
    fn one_wire_per_input() {
        let mut d = Design::from_text(STARTER).unwrap();
        d.connect(1, 3, 0).unwrap();
        assert_eq!(d.wires.iter().filter(|w| w.to == 3).count(), 1);
        assert!(d.connect(3, 4, 0).is_err());
        assert!(d.connect(1, 1, 0).is_err());
    }

    #[test]
    fn names_are_clean() {
        assert_eq!(clean_name("../../etc"), "etc");
        assert_eq!(clean_name("  "), "Unnamed");
        assert_eq!(clean_name("Ice Mule 2"), "Ice Mule 2");
    }
}
