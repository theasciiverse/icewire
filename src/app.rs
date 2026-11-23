//! What is on screen and what every key does there.

use crate::challenge::{self, Goal};
use crate::design::{self, BENCH_H, BENCH_W, Design};
use crate::parts::{self, BOX_H, BOX_W, Kind};
use crate::route::{self, Layout};
use crate::sim::{Outcome, Sim};
use crate::store::{self, Records, Settings, Stored};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Title,
    Garage,
    Challenges,
    Library,
    Help,
    Bench,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Edit,
    /// Moving a part; where it was, and where it was grabbed (for the mouse).
    Move { id: u32, from: (i32, i32), grab: (i32, i32), mouse: bool },
    /// Choosing where a new wire from this part goes.
    Wire { from: u32 },
}

#[derive(Clone, PartialEq, Debug)]
pub enum Popup {
    Add { sel: usize },
    Edit { id: u32, field: usize, typed: String },
    /// Naming: the current rover (rename) or a new one (garage).
    Name { buf: String, new: bool },
    Port { from: u32, to: u32, sel: usize },
    Delete { stem: String },
}

/// How a run ended, for the result card.
#[derive(Clone, Debug)]
pub struct RunResult {
    pub won: bool,
    pub secs: f64,
    pub stars: u8,
    pub best: bool,
    pub text: String,
}

pub const TITLE_MENU: [&str; 6] = ["Wire a rover", "Garage", "Challenges", "Parts library", "How to play", "Quit"];
pub const SPEEDS: [u32; 4] = [1, 2, 4, 8];

pub struct App {
    pub screen: Screen,
    pub back: Screen,
    pub design: Design,
    pub stem: Option<String>,
    pub cx: i32,
    pub cy: i32,
    /// The bench's top-left corner on screen.
    pub vx: i32,
    pub vy: i32,
    pub mode: Mode,
    pub undo: Vec<Design>,
    pub layout: Layout,
    pub ch: usize,
    pub sim: Option<Sim>,
    pub arena_view: bool,
    pub paused: bool,
    pub speed: usize,
    pub result: Option<RunResult>,
    pub records: Records,
    pub garage: Vec<Stored>,
    pub sel: usize,
    pub lib_sel: usize,
    pub help_scroll: u16,
    pub popup: Option<Popup>,
    pub msg: Option<(String, u32)>,
    pub exit: bool,
    /// Where the bench was last drawn, for the mouse.
    pub bench_area: Rect,
    /// No saving: the attract-mode tour.
    pub demo: bool,
    pub frame: u32,
}

/// Library order: sensors, logic, drive.
pub fn library() -> Vec<Kind> {
    parts::ALL.to_vec()
}

impl App {
    pub fn new(demo: bool) -> App {
        let settings = Settings::load();
        let garage = if demo { Vec::new() } else { store::garage() };
        let ch = settings.challenge.as_deref().and_then(challenge::by_id).unwrap_or(0);
        let mut app = App {
            screen: Screen::Title,
            back: Screen::Title,
            design: Design::from_text(design::STARTER).expect("starter"),
            stem: None,
            cx: 2,
            cy: 2,
            vx: 0,
            vy: 0,
            mode: Mode::Edit,
            undo: Vec::new(),
            layout: Layout { cells: Vec::new(), paths: Vec::new() },
            ch,
            sim: None,
            arena_view: true,
            paused: false,
            speed: 0,
            result: None,
            records: if demo { Records::default() } else { Records::load() },
            garage,
            sel: 0,
            lib_sel: 0,
            help_scroll: 0,
            popup: None,
            msg: None,
            exit: false,
            bench_area: Rect::default(),
            demo,
            frame: 0,
        };
        let pick = settings.rover.as_deref().and_then(|r| app.garage.iter().position(|g| g.stem == r && g.design.is_ok())).or_else(|| app.garage.iter().position(|g| g.design.is_ok()));
        if let Some(i) = pick {
            app.open(i);
        }
        app.relayout();
        app
    }

    pub fn say(&mut self, m: impl Into<String>) {
        let m = m.into();
        log::info!("message: {m}");
        self.msg = Some((m, 150));
    }

    fn open(&mut self, i: usize) {
        if let Some(Stored { stem, design: Ok(d) }) = self.garage.get(i) {
            let stem = stem.clone();
            self.design = d.clone();
            self.stem = Some(stem.clone());
            self.undo.clear();
            self.mode = Mode::Edit;
            if let Some(p) = self.design.parts.first() {
                self.cx = p.x + 1;
                self.cy = p.y + 1;
            }
            self.vx = 0;
            self.vy = 0;
            self.relayout();
            log::info!("opened rover {stem}");
        }
    }

    pub fn relayout(&mut self) {
        self.layout = route::layout(&self.design);
    }

    pub fn save(&mut self) {
        if self.demo {
            return;
        }
        if let Some(s) = store::save(&self.design, self.stem.as_deref()) {
            self.stem = Some(s);
        }
        Settings { rover: self.stem.clone(), challenge: Some(challenge::ALL[self.ch].id.to_string()) }.save();
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.design.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
    }

    fn changed(&mut self) {
        self.relayout();
        self.save();
    }

    fn go(&mut self, s: Screen) {
        log::info!("screen {:?} -> {s:?}", self.screen);
        if s == Screen::Help {
            self.help_scroll = 0;
        }
        if matches!(s, Screen::Library | Screen::Help) {
            self.back = self.screen;
        }
        if s == Screen::Garage {
            if !self.demo {
                self.garage = store::garage();
            }
            self.sel = self.garage.iter().position(|g| Some(&g.stem) == self.stem.as_ref()).unwrap_or(0);
        }
        if s == Screen::Challenges {
            self.sel = self.ch;
        }
        if s == Screen::Title {
            self.sel = 0;
        }
        self.screen = s;
    }

    /// The part under the bench cursor.
    pub fn under(&self) -> Option<u32> {
        self.design.part_at(self.cx, self.cy)
    }

    pub fn running(&self) -> bool {
        self.sim.is_some()
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        if let Some((_, t)) = &mut self.msg {
            *t = t.saturating_sub(1);
            if *t == 0 {
                self.msg = None;
            }
        }
        let Some(sim) = &mut self.sim else { return };
        if !self.paused && self.result.is_none() {
            for _ in 0..SPEEDS[self.speed] {
                sim.step();
            }
        }
        if sim.outcome != Outcome::Running && self.result.is_none() {
            self.finish();
        }
    }

    fn finish(&mut self) {
        let Some(sim) = &self.sim else { return };
        let ch = sim.ch;
        let secs = sim.secs();
        let r = match sim.outcome {
            Outcome::Won(_) => {
                let stars = ch.stars(secs);
                let best = !self.demo && self.records.offer(ch.id, secs, &self.design.name);
                let what = match ch.goal {
                    Goal::Race => "Lap done",
                    Goal::Maze => "Out through the hatch",
                    Goal::Sumo => "Tumbler is out of the ring",
                };
                RunResult { won: true, secs, stars, best, text: format!("{what} in {secs:.2} s") }
            }
            Outcome::Lost(why) => RunResult { won: false, secs, stars: 0, best: false, text: format!("Not this time: {why}") },
            Outcome::Draw => RunResult { won: false, secs, stars: 0, best: false, text: "Time! Both rovers still on the ice".into() },
            Outcome::Running => return,
        };
        log::info!("run over: {} ({:.2} s, {} stars)", r.text, r.secs, r.stars);
        self.result = Some(r);
        self.arena_view = true;
    }

    pub fn run(&mut self) {
        self.save();
        self.sim = Some(Sim::new(self.ch, self.design.clone()));
        self.result = None;
        self.paused = false;
        self.arena_view = true;
        self.mode = Mode::Edit;
        self.popup = None;
        log::info!("run {} with {:?}", challenge::ALL[self.ch].id, self.design.name);
    }

    fn stop(&mut self) {
        self.sim = None;
        self.result = None;
        self.paused = false;
        log::info!("run stopped");
    }

    // ---------------------------------------------------------- keys

    pub fn key(&mut self, k: KeyEvent) {
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.save();
            self.exit = true;
            return;
        }
        if self.popup.is_some() {
            self.popup_key(k);
            return;
        }
        match self.screen {
            Screen::Title => self.title_key(k),
            Screen::Garage => self.garage_key(k),
            Screen::Challenges => self.challenges_key(k),
            Screen::Library => self.library_key(k),
            Screen::Help => {
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => self.help_scroll = self.help_scroll.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => self.help_scroll = (self.help_scroll + 1).min(40),
                    KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(10),
                    KeyCode::PageDown | KeyCode::Char(' ') => self.help_scroll = (self.help_scroll + 10).min(40),
                    _ => {}
                }
                if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::F(1)) {
                    self.go(self.back);
                }
            }
            Screen::Bench => {
                if self.running() {
                    self.run_key(k)
                } else {
                    self.bench_key(k)
                }
            }
        }
    }

    fn list_nav(sel: &mut usize, len: usize, k: &KeyEvent) -> bool {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => *sel = (*sel + len.max(1) - 1) % len.max(1),
            KeyCode::Down | KeyCode::Char('j') => *sel = (*sel + 1) % len.max(1),
            KeyCode::Home => *sel = 0,
            KeyCode::End => *sel = len.saturating_sub(1),
            _ => return false,
        }
        true
    }

    fn title_key(&mut self, k: KeyEvent) {
        if Self::list_nav(&mut self.sel, TITLE_MENU.len(), &k) {
            return;
        }
        match k.code {
            KeyCode::Enter | KeyCode::Char(' ') => match self.sel {
                0 => self.go(Screen::Bench),
                1 => self.go(Screen::Garage),
                2 => self.go(Screen::Challenges),
                3 => self.go(Screen::Library),
                4 => self.go(Screen::Help),
                _ => self.exit = true,
            },
            KeyCode::Char('w') => self.go(Screen::Bench),
            KeyCode::Char('g') => self.go(Screen::Garage),
            KeyCode::Char('c') => self.go(Screen::Challenges),
            KeyCode::Char('l') => self.go(Screen::Library),
            KeyCode::Char('?') | KeyCode::F(1) => self.go(Screen::Help),
            KeyCode::Esc | KeyCode::Char('q') => self.exit = true,
            _ => {}
        }
    }

    fn garage_key(&mut self, k: KeyEvent) {
        let len = self.garage.len();
        if Self::list_nav(&mut self.sel, len, &k) {
            return;
        }
        match k.code {
            KeyCode::Enter => {
                if self.garage.get(self.sel).is_some_and(|g| g.design.is_ok()) {
                    self.open(self.sel);
                    self.save();
                    self.go(Screen::Bench);
                } else if !self.garage.is_empty() {
                    self.say("That file does not read as a rover; fix it in a text editor");
                }
            }
            KeyCode::Char('n') => self.popup = Some(Popup::Name { buf: String::new(), new: true }),
            KeyCode::Char('c') => {
                if let Some(Stored { design: Ok(d), .. }) = self.garage.get(self.sel) {
                    let mut d = d.clone();
                    d.name = design::clean_name(&format!("{} copy", d.name));
                    if !self.demo {
                        store::save(&d, None);
                        self.garage = store::garage();
                    }
                    self.say(format!("Copied as {}", d.name));
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(g) = self.garage.get(self.sel) {
                    self.popup = Some(Popup::Delete { stem: g.stem.clone() });
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => self.go(Screen::Title),
            _ => {}
        }
    }

    fn challenges_key(&mut self, k: KeyEvent) {
        if Self::list_nav(&mut self.sel, challenge::ALL.len(), &k) {
            return;
        }
        match k.code {
            KeyCode::Enter => {
                self.ch = self.sel;
                self.save();
                self.go(Screen::Bench);
            }
            KeyCode::Char('r') => {
                self.ch = self.sel;
                self.go(Screen::Bench);
                self.run();
            }
            KeyCode::Esc | KeyCode::Char('q') => self.go(Screen::Title),
            _ => {}
        }
    }

    fn library_key(&mut self, k: KeyEvent) {
        if Self::list_nav(&mut self.lib_sel, parts::ALL.len(), &k) {
            return;
        }
        if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('l')) {
            self.go(self.back);
        }
    }

    fn cursor_move(&mut self, k: &KeyEvent) -> bool {
        let fast = k.modifiers.contains(KeyModifiers::SHIFT);
        let n = if fast { 4 } else { 1 };
        let (dx, dy) = match k.code {
            KeyCode::Left | KeyCode::Char('h') => (-1, 0),
            KeyCode::Right | KeyCode::Char('l') => (1, 0),
            KeyCode::Up | KeyCode::Char('k') => (0, -1),
            KeyCode::Down | KeyCode::Char('j') => (0, 1),
            KeyCode::Char('H') => (-4, 0),
            KeyCode::Char('L') => (4, 0),
            KeyCode::Char('K') => (0, -4),
            KeyCode::Char('J') => (0, 4),
            _ => return false,
        };
        if let Mode::Move { id, .. } = self.mode {
            if let Some(p) = self.design.part_mut(id) {
                p.x = (p.x + dx * n).clamp(1, BENCH_W - BOX_W - 1);
                p.y = (p.y + dy * n).clamp(0, BENCH_H - BOX_H);
                self.cx = p.x + 1;
                self.cy = p.y + 1;
            }
            self.relayout();
        } else {
            self.cx = (self.cx + dx * n).clamp(0, BENCH_W - 1);
            self.cy = (self.cy + dy * n).clamp(0, BENCH_H - 1);
        }
        true
    }

    /// Tab: jump the cursor to the next part, reading order.
    fn jump(&mut self, back: bool) {
        let mut ids: Vec<(i32, i32, u32)> = self.design.parts.iter().map(|p| (p.y, p.x, p.id)).collect();
        ids.sort();
        if ids.is_empty() {
            return;
        }
        let cur = self.under().and_then(|id| ids.iter().position(|e| e.2 == id));
        let n = ids.len();
        let i = match (cur, back) {
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
            (None, false) => 0,
            (None, true) => n - 1,
        };
        let p = self.design.part(ids[i].2).expect("part");
        self.cx = p.x + 1;
        self.cy = p.y + 1;
    }

    fn bench_key(&mut self, k: KeyEvent) {
        if self.cursor_move(&k) {
            return;
        }
        match self.mode {
            Mode::Move { id, from, .. } => match k.code {
                KeyCode::Enter | KeyCode::Char('m') | KeyCode::Char(' ') => self.drop_part(id, from),
                KeyCode::Esc => {
                    if let Some(p) = self.design.part_mut(id) {
                        (p.x, p.y) = from;
                    }
                    self.mode = Mode::Edit;
                    self.relayout();
                }
                _ => {}
            },
            Mode::Wire { from } => match k.code {
                KeyCode::Tab => self.jump(false),
                KeyCode::BackTab => self.jump(true),
                KeyCode::Enter | KeyCode::Char('w') | KeyCode::Char(' ') => self.wire_to(from),
                KeyCode::Esc => {
                    self.mode = Mode::Edit;
                    self.say("Wiring cancelled");
                }
                _ => {}
            },
            Mode::Edit => match k.code {
                KeyCode::Tab => self.jump(false),
                KeyCode::BackTab => self.jump(true),
                KeyCode::Char('a') | KeyCode::Insert => self.popup = Some(Popup::Add { sel: 0 }),
                KeyCode::Enter | KeyCode::Char('e') => match self.under() {
                    Some(id) => self.edit(id),
                    None if k.code == KeyCode::Enter => self.popup = Some(Popup::Add { sel: 0 }),
                    None => {}
                },
                KeyCode::Char('m') => {
                    if let Some(id) = self.under() {
                        let (px, py) = self.design.part(id).map(|p| (p.x, p.y)).expect("part");
                        self.checkpoint();
                        self.mode = Mode::Move { id, from: (px, py), grab: (1, 1), mouse: false };
                        self.cx = px + 1;
                        self.cy = py + 1;
                    }
                }
                KeyCode::Char('w') => self.start_wire(),
                KeyCode::Char('x') | KeyCode::Delete | KeyCode::Backspace => self.delete_here(),
                KeyCode::Char('u') | KeyCode::Char('z') => self.undo(),
                KeyCode::Char('r') | KeyCode::F(5) => self.run(),
                KeyCode::Char('c') => {
                    self.ch = (self.ch + 1) % challenge::ALL.len();
                    self.save();
                    self.say(format!("Challenge: {}", challenge::ALL[self.ch].name));
                }
                KeyCode::Char('n') => self.popup = Some(Popup::Name { buf: self.design.name.clone(), new: false }),
                KeyCode::Char('s') => {
                    self.save();
                    self.say(format!("Saved {}", self.design.name));
                }
                KeyCode::Char('?') | KeyCode::F(1) => self.go(Screen::Help),
                KeyCode::Char('p') => {
                    if let Some(id) = self.under() {
                        self.lib_sel = parts::ALL.iter().position(|k| Some(*k) == self.design.part(id).map(|p| p.kind)).unwrap_or(0);
                    }
                    self.go(Screen::Library);
                }
                KeyCode::Char('g') => {
                    self.save();
                    self.go(Screen::Garage);
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.save();
                    self.go(Screen::Title);
                }
                _ => {}
            },
        }
    }

    fn drop_part(&mut self, id: u32, from: (i32, i32)) {
        let Some(p) = self.design.part(id) else { return };
        if self.design.fits(p.x, p.y, Some(id)) {
            self.mode = Mode::Edit;
            if (p.x, p.y) == from {
                self.undo.pop();
            }
            self.changed();
        } else {
            self.say("Too close to another part; parts keep a gap for wires");
        }
    }

    fn start_wire(&mut self) {
        match self.under().and_then(|id| self.design.part(id)) {
            Some(p) if p.kind.has_output() => {
                self.mode = Mode::Wire { from: p.id };
                self.say(format!("Wiring from the {}: move to a part and press Enter", p.kind.name().to_lowercase()));
            }
            Some(p) => self.say(format!("The {} has no output; start a wire at what feeds it", p.kind.name().to_lowercase())),
            None => self.say("Put the cursor on a part to wire from it"),
        }
    }

    fn wire_to(&mut self, from: u32) {
        let Some(to) = self.under() else {
            self.say("Move onto the part this wire should go to");
            return;
        };
        let Some(t) = self.design.part(to) else { return };
        let ins = t.kind.inputs().len();
        if ins == 0 {
            self.say(format!("The {} has no inputs", t.kind.name().to_lowercase()));
            return;
        }
        if to == from {
            self.say("A part cannot feed itself");
            return;
        }
        if ins == 1 {
            self.connect(from, to, 0);
        } else {
            // the input row the cursor is on, else the first free one
            let row = (self.cy - t.y - 1) as usize;
            let sel = if row < ins { row } else { (0..ins).find(|&i| self.design.wire_into(to, i).is_none()).unwrap_or(0) };
            self.popup = Some(Popup::Port { from, to, sel });
        }
    }

    fn connect(&mut self, from: u32, to: u32, port: usize) {
        self.checkpoint();
        match self.design.connect(from, to, port) {
            Ok(()) => {
                let (f, t) = (self.design.part(from).expect("part").kind, self.design.part(to).expect("part").kind);
                let port_name = t.inputs()[port];
                self.say(format!("Wired {} to {} {}", f.name(), t.name(), port_name));
                self.mode = Mode::Edit;
                self.changed();
            }
            Err(e) => {
                self.undo.pop();
                self.say(e);
            }
        }
    }

    fn edit(&mut self, id: u32) {
        let Some(p) = self.design.part(id) else { return };
        if p.kind.mounts().is_empty() && p.kind.modes().is_empty() && p.kind.value().is_none() {
            self.say(format!("{}: {}. Nothing to set.", p.kind.name(), p.kind.summary()));
            return;
        }
        self.checkpoint();
        self.popup = Some(Popup::Edit { id, field: 0, typed: String::new() });
    }

    fn delete_here(&mut self) {
        if let Some(id) = self.under() {
            let name = self.design.part(id).map(|p| p.kind.name()).unwrap_or("part");
            self.checkpoint();
            self.design.remove(id);
            self.say(format!("Removed the {}", name.to_lowercase()));
            self.changed();
        } else if let Some(w) = self.layout.wire_at(self.cx, self.cy) {
            self.checkpoint();
            self.design.wires.remove(w);
            self.say("Wire removed");
            self.changed();
        } else {
            self.say("Nothing here to remove");
        }
    }

    fn undo(&mut self) {
        match self.undo.pop() {
            Some(d) => {
                self.design = d;
                self.mode = Mode::Edit;
                self.say("Undone");
                self.changed();
            }
            None => self.say("Nothing to undo"),
        }
    }

    fn run_key(&mut self, k: KeyEvent) {
        if self.result.is_some() {
            match k.code {
                KeyCode::Char('r') | KeyCode::F(5) => self.run(),
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => self.stop(),
                KeyCode::Tab => self.arena_view = !self.arena_view,
                _ => {}
            }
            return;
        }
        if !self.arena_view && self.cursor_move(&k) {
            return;
        }
        match k.code {
            KeyCode::Char(' ') | KeyCode::Char('p') => {
                self.paused = !self.paused;
                log::info!("paused {}", self.paused);
            }
            KeyCode::Char('.') => {
                self.paused = true;
                if let Some(s) = &mut self.sim {
                    s.step();
                }
            }
            KeyCode::Char('f') | KeyCode::Char('+') => {
                self.speed = (self.speed + 1) % SPEEDS.len();
                self.say(format!("Speed x{}", SPEEDS[self.speed]));
            }
            KeyCode::Char('-') => {
                self.speed = self.speed.saturating_sub(1);
                self.say(format!("Speed x{}", SPEEDS[self.speed]));
            }
            KeyCode::Tab => {
                self.arena_view = !self.arena_view;
                log::info!("arena view {}", self.arena_view);
            }
            KeyCode::BackTab => self.jump(false),
            KeyCode::Char('r') | KeyCode::F(5) => self.run(),
            KeyCode::Esc | KeyCode::Char('q') => self.stop(),
            _ => {
                if !self.arena_view {
                    self.say("The rover is running: Esc stops it so you can rewire");
                }
            }
        }
    }

    // -------------------------------------------------------- popups

    pub fn edit_fields(kind: Kind) -> Vec<&'static str> {
        let mut f = Vec::new();
        if !kind.mounts().is_empty() {
            f.push("mount");
        }
        if !kind.modes().is_empty() {
            f.push("mode");
        }
        if kind.value().is_some() {
            f.push("value");
        }
        f
    }

    fn popup_key(&mut self, k: KeyEvent) {
        let Some(popup) = self.popup.clone() else { return };
        match popup {
            Popup::Add { mut sel } => {
                let lib = library();
                if Self::list_nav(&mut sel, lib.len(), &k) {
                    self.popup = Some(Popup::Add { sel });
                    return;
                }
                match k.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        let kind = lib[sel];
                        self.popup = None;
                        self.checkpoint();
                        match self.design.add(kind, self.cx, self.cy) {
                            Ok(id) => {
                                let p = self.design.part(id).expect("part");
                                self.cx = p.x + 1;
                                self.cy = p.y + 1;
                                self.say(format!("{}: {}", kind.name(), kind.summary()));
                                self.changed();
                            }
                            Err(e) => {
                                self.undo.pop();
                                self.say(e);
                            }
                        }
                    }
                    KeyCode::Esc => self.popup = None,
                    KeyCode::Char(c) => {
                        // jump to the next part starting with that letter
                        let c = c.to_ascii_lowercase();
                        let n = lib.len();
                        if let Some(i) = (1..=n).map(|d| (sel + d) % n).find(|&i| lib[i].name().to_ascii_lowercase().starts_with(c)) {
                            self.popup = Some(Popup::Add { sel: i });
                        }
                    }
                    _ => {}
                }
            }
            Popup::Edit { id, mut field, mut typed } => {
                let Some(kind) = self.design.part(id).map(|p| p.kind) else {
                    self.popup = None;
                    return;
                };
                let fields = Self::edit_fields(kind);
                let fname = fields[field.min(fields.len() - 1)];
                let big = k.modifiers.contains(KeyModifiers::SHIFT);
                let apply_typed = |app: &mut App, typed: &mut String| {
                    if fname == "value"
                        && !typed.is_empty()
                        && let (Ok(v), Some(spec)) = (typed.parse::<f64>(), kind.value())
                        && let Some(p) = app.design.part_mut(id)
                    {
                        p.value = v.clamp(spec.min, spec.max);
                    }
                    typed.clear();
                };
                match k.code {
                    KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                        apply_typed(self, &mut typed);
                        field = if k.code == KeyCode::Up { (field + fields.len() - 1) % fields.len() } else { (field + 1) % fields.len() };
                    }
                    KeyCode::Left | KeyCode::Right => {
                        typed.clear();
                        let dir = if k.code == KeyCode::Left { -1 } else { 1 };
                        if let Some(p) = self.design.part_mut(id) {
                            match fname {
                                "mount" => {
                                    let ms = kind.mounts();
                                    let i = ms.iter().position(|m| *m == p.mount).unwrap_or(0) as i32;
                                    p.mount = ms[(i + dir).rem_euclid(ms.len() as i32) as usize];
                                }
                                "mode" => p.mode = ((p.mode as i32 + dir).rem_euclid(kind.modes().len() as i32)) as usize,
                                _ => {
                                    if let Some(spec) = kind.value() {
                                        let step = if big { spec.step * 10.0 } else { spec.step };
                                        let v = ((p.value + dir as f64 * step) / spec.step).round() * spec.step;
                                        p.value = v.clamp(spec.min, spec.max);
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) if fname == "value" && (c.is_ascii_digit() || c == '.' || c == '-') => {
                        if typed.len() < 7 {
                            typed.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        typed.pop();
                    }
                    KeyCode::Enter | KeyCode::Esc => {
                        apply_typed(self, &mut typed);
                        self.popup = None;
                        if self.undo.last() == Some(&self.design) {
                            self.undo.pop();
                        }
                        self.changed();
                        return;
                    }
                    _ => {}
                }
                self.popup = Some(Popup::Edit { id, field, typed });
            }
            Popup::Name { mut buf, new } => match k.code {
                KeyCode::Enter => {
                    let name = design::clean_name(&buf);
                    self.popup = None;
                    if new {
                        let d = Design::blank(&name);
                        if !self.demo {
                            let stem = store::save(&d, None);
                            self.garage = store::garage();
                            self.sel = self.garage.iter().position(|g| Some(&g.stem) == stem.as_ref()).unwrap_or(0);
                        }
                        self.say(format!("{name} is in the garage: a bare Mule, tracks and gyro fitted"));
                    } else {
                        self.design.name = name;
                        self.save();
                    }
                }
                KeyCode::Esc => self.popup = None,
                KeyCode::Backspace => {
                    buf.pop();
                    self.popup = Some(Popup::Name { buf, new });
                }
                KeyCode::Char(c) => {
                    if buf.chars().count() < 20 {
                        buf.push(c);
                    }
                    self.popup = Some(Popup::Name { buf, new });
                }
                _ => {}
            },
            Popup::Port { from, to, mut sel } => {
                let ins = self.design.part(to).map(|p| p.kind.inputs()).unwrap_or(&[]);
                if Self::list_nav(&mut sel, ins.len(), &k) {
                    self.popup = Some(Popup::Port { from, to, sel });
                    return;
                }
                match k.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        self.popup = None;
                        self.connect(from, to, sel);
                    }
                    KeyCode::Char(c) => {
                        if let Some(i) = ins.iter().position(|n| n.starts_with(c)) {
                            self.popup = None;
                            self.connect(from, to, i);
                        }
                    }
                    KeyCode::Esc => self.popup = None,
                    _ => {}
                }
            }
            Popup::Delete { stem } => match k.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.popup = None;
                    if !self.demo {
                        store::delete(&stem);
                        self.garage = store::garage();
                    }
                    if self.stem.as_deref() == Some(stem.as_str()) {
                        self.stem = None;
                        if let Some(i) = self.garage.iter().position(|g| g.design.is_ok()) {
                            self.open(i);
                        } else {
                            self.design = Design::blank("Mule");
                        }
                    }
                    self.sel = self.sel.min(self.garage.len().saturating_sub(1));
                    self.say("Deleted");
                }
                _ => self.popup = None,
            },
        }
    }

    // --------------------------------------------------------- mouse

    pub fn mouse(&mut self, m: MouseEvent) {
        if self.screen != Screen::Bench || self.popup.is_some() || (self.running() && self.arena_view) {
            return;
        }
        let a = self.bench_area;
        let inside = m.column >= a.x && m.column < a.x + a.width && m.row >= a.y && m.row < a.y + a.height;
        let (bx, by) = (m.column as i32 - a.x as i32 + self.vx, m.row as i32 - a.y as i32 + self.vy);
        match m.kind {
            MouseEventKind::ScrollUp => self.vy = (self.vy - 3).max(0),
            MouseEventKind::ScrollDown => self.vy = (self.vy + 3).min(BENCH_H - 4),
            MouseEventKind::ScrollLeft => self.vx = (self.vx - 4).max(0),
            MouseEventKind::ScrollRight => self.vx = (self.vx + 4).min(BENCH_W - 4),
            _ if !inside => {}
            MouseEventKind::Down(MouseButton::Left) => {
                self.cx = bx;
                self.cy = by;
                if self.running() {
                    return;
                }
                match self.mode {
                    Mode::Wire { from } => self.wire_to(from),
                    Mode::Move { id, from, .. } => self.drop_part(id, from),
                    Mode::Edit => {
                        if let Some(id) = self.under() {
                            let (px, py, out) = self.design.part(id).map(|p| (p.x, p.y, p.kind.has_output())).expect("part");
                            if (bx, by) == (px + BOX_W - 1, py + 1) && out {
                                self.start_wire();
                            } else {
                                self.checkpoint();
                                self.mode = Mode::Move { id, from: (px, py), grab: (bx - px, by - py), mouse: true };
                            }
                        }
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Mode::Move { id, grab, .. } = self.mode
                    && let Some(p) = self.design.part_mut(id)
                {
                    p.x = (bx - grab.0).clamp(1, BENCH_W - BOX_W - 1);
                    p.y = (by - grab.1).clamp(0, BENCH_H - BOX_H);
                    self.cx = bx;
                    self.cy = by;
                    self.relayout();
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Mode::Move { id, from, mouse: true, .. } = self.mode {
                    let fits = self.design.part(id).is_some_and(|p| self.design.fits(p.x, p.y, Some(id)));
                    if fits {
                        self.drop_part(id, from);
                    } else {
                        if let Some(p) = self.design.part_mut(id) {
                            (p.x, p.y) = from;
                        }
                        self.undo.pop();
                        self.mode = Mode::Edit;
                        self.relayout();
                        self.say("Too close to another part; put back");
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if !self.running() && self.mode == Mode::Edit {
                    self.cx = bx;
                    self.cy = by;
                    self.delete_here();
                }
            }
            _ => {}
        }
    }

    /// Keeps the cursor inside the bench view.
    pub fn follow_cursor(&mut self, w: i32, h: i32) {
        let margin = 2;
        if self.cx < self.vx + margin {
            self.vx = (self.cx - margin).max(0);
        }
        if self.cx >= self.vx + w - margin {
            self.vx = (self.cx - w + margin + 1).min(BENCH_W - w).max(0);
        }
        if self.cy < self.vy + 1 {
            self.vy = (self.cy - 1).max(0);
        }
        if self.cy >= self.vy + h - 1 {
            self.vy = (self.cy - h + 2).min(BENCH_H - h).max(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    fn key(app: &mut App, c: KeyCode) {
        app.key(KeyEvent::new_with_kind(c, KeyModifiers::NONE, KeyEventKind::Press));
    }

    #[test]
    fn wire_a_rover_by_keyboard() {
        let mut app = App::new(false);
        app.design = Design::blank("Test");
        app.relayout();
        app.screen = Screen::Bench;
        app.cx = 4;
        app.cy = 4;
        // add a number (the add list starts at Bumper; N jumps to NOT, then Number)
        key(&mut app, KeyCode::Char('a'));
        key(&mut app, KeyCode::Char('n'));
        key(&mut app, KeyCode::Char('n'));
        assert_eq!(app.popup, Some(Popup::Add { sel: parts::ALL.iter().position(|k| *k == Kind::Number).unwrap() }));
        key(&mut app, KeyCode::Enter);
        let num = app.under().expect("number placed under the cursor");
        assert_eq!(app.design.part(num).unwrap().kind, Kind::Number);
        // wire it to the left track
        key(&mut app, KeyCode::Char('w'));
        let track = app.design.parts.iter().find(|p| p.kind == Kind::TrackLeft).unwrap().clone();
        app.cx = track.x + 2;
        app.cy = track.y + 1;
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.popup, Some(Popup::Port { from: num, to: track.id, sel: 0 }));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.design.wires.len(), 1);
        assert!(app.layout.paths[0].len() > 2);
        // set the track's power by typing
        key(&mut app, KeyCode::Enter);
        for c in "85".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.design.part(track.id).unwrap().value, 85.0);
        // undo twice takes the value and the wire back
        key(&mut app, KeyCode::Char('u'));
        key(&mut app, KeyCode::Char('u'));
        assert!(app.design.wires.is_empty());
        // run it, and stop
        key(&mut app, KeyCode::Char('r'));
        assert!(app.running());
        for _ in 0..30 {
            app.tick();
        }
        key(&mut app, KeyCode::Esc);
        assert!(!app.running());
    }

    #[test]
    fn mouse_drags_and_removes() {
        let mut app = App::new(false);
        app.screen = Screen::Bench;
        app.bench_area = Rect::new(0, 1, 70, 27);
        let p = app.design.parts[0].clone();
        let m = |kind, x: i32, y: i32| MouseEvent { kind, column: x as u16, row: (y + 1) as u16, modifiers: KeyModifiers::NONE };
        app.mouse(m(MouseEventKind::Down(MouseButton::Left), p.x + 3, p.y + 2));
        app.mouse(m(MouseEventKind::Drag(MouseButton::Left), p.x + 3, p.y + 22));
        app.mouse(m(MouseEventKind::Up(MouseButton::Left), p.x + 3, p.y + 22));
        let moved = app.design.part(p.id).unwrap();
        assert_eq!((moved.x, moved.y), (p.x, p.y + 20));
        assert_eq!(app.mode, Mode::Edit);
        let wires = app.design.wires.len();
        app.mouse(m(MouseEventKind::Down(MouseButton::Right), moved.x + 2, moved.y + 1));
        assert!(app.design.part(p.id).is_none());
        assert!(app.design.wires.len() < wires);
        key(&mut app, KeyCode::Char('u'));
        assert!(app.design.part(p.id).is_some());
    }
}
