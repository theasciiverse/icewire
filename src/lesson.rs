//! Watch and learn: Ines wires a rover on the real bench, key by key, and
//! runs it in Ring Hall three times, each time a little cleverer. Every
//! step is a real key press into the game, at a pace a person can follow,
//! with a caption saying what is happening and why, and the keys shown as
//! they are pressed. The player can pause, step forward and back, or
//! leave; at the end the rover can be kept in the garage.
//!
//! Nothing is saved while the lesson plays (`App::quiet`): no rover, no
//! record, no league night, no settings.
//!
//! The lap times in the captions are measured, not hoped for: the test at
//! the end plays the whole lesson and checks every run.

use std::collections::VecDeque;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Mode, Screen};
use crate::challenge;
use crate::design::Design;
use crate::parts::Kind;

/// What one lesson step does; turned into key presses when it starts, from
/// where things are on the bench at that moment.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// One key.
    Press(KeyCode),
    /// Walk the cursor to an empty spot on the bench.
    Goto(i32, i32),
    /// Walk the cursor onto the nth part of a kind, on its input row `row`.
    On(Kind, usize, usize),
    /// Add a part at the cursor: a, the part's first letter until it is
    /// picked, Enter.
    Add(Kind),
    /// On the part under the cursor: open its settings, go to field
    /// `field`, press → `n` times, Enter.
    Choose(usize, usize),
    /// On the part under the cursor: open its settings, go to field
    /// `field`, type a number, Enter.
    Type(usize, &'static str),
    /// A wire from the nth part of one kind into input `port` of another.
    Wire((Kind, usize), (Kind, usize), usize),
    /// Wait this many ticks (60 a second).
    Hold(u32),
    /// Wait for the run's result card, at most this many ticks.
    UntilDone(u32),
}

pub struct Beat {
    pub title: &'static str,
    pub say: &'static str,
    pub acts: &'static [Act],
}

use Act::*;
use KeyCode::{Char, Enter, Esc, Tab};

const LAP: u32 = 60 * 130;
/// The lesson bench's columns.
const SENSE_X: i32 = 4;
const LOGIC_X: i32 = 22;
const DRIVE_X: i32 = 42;

/// The lesson. Times and stars as measured in Ring Hall: nothing wired,
/// out of time; a number into the tracks, out of time; plus the compass
/// into the gyro, 53.4 s and one star; tracks at 100, 40.5 s and two;
/// slowing within 3 m of a checkpoint, 36.5 s and three.
pub const BEATS: [Beat; 16] = [
    Beat {
        title: "The bench",
        say: "Ines pulls a stool up to the workbench. \"Every rover is a Mule: two tracks and a gyro. Sensors feel the ice, logic decides, drive moves. Tonight you wire one yourself.\" On the right sit the parts every Mule has: the left track, the right track and the gyro. Nothing is wired yet.",
        acts: &[Hold(60)],
    },
    Beat {
        title: "Nothing wired",
        say: "r runs the rover in the challenge, here Ring Hall: one lap, eight checkpoints. The Mule sits still. Drive parts only do what their wires tell them, and nothing is telling them anything. Esc stops the run.",
        acts: &[Press(Char('r')), Hold(180), Press(Esc)],
    },
    Beat {
        title: "A number",
        say: "Move the cursor to an empty spot and press a to add a part. Typing a letter jumps through the list, so n twice finds the Number. A number always gives its value, and a new one gives 1.",
        acts: &[Goto(SENSE_X, 8), Add(Kind::Number)],
    },
    Beat {
        title: "Two wires",
        say: "w starts a wire at the part under the cursor. Walk to the left track's fwd input and press Enter, then the same for the right track. A track drives at its input times its power: 1 times 60 percent.",
        acts: &[Wire((Kind::Number, 0), (Kind::TrackLeft, 0), 0), Wire((Kind::Number, 0), (Kind::TrackRight, 0), 0)],
    },
    Beat {
        title: "Straight ahead",
        say: "Run it again. The Mule rolls off, straight and steady, and never finds a single checkpoint. Moving was the easy part; now it has to know where to go.",
        acts: &[Press(Char('r')), Hold(300), Press(Esc)],
    },
    Beat {
        title: "A compass",
        say: "Add a compass (a, then c) and press Enter on it to set it. It starts on heading; one press of \u{2192} makes it bearing: how far the next checkpoint lies to the right of the nose, from -180 to 180 degrees.",
        acts: &[Goto(SENSE_X, 2), Add(Kind::Compass), Choose(0, 1)],
    },
    Beat {
        title: "Steering",
        say: "Wire the compass into the gyro's right input. A checkpoint off to the right gives a positive bearing and spins the Mule right; one to the left gives a negative bearing and spins it left. Point at the checkpoint, drive at it: that is all Homer, the league's first rover, does.",
        acts: &[Wire((Kind::Compass, 0), (Kind::Gyro, 0), 0)],
    },
    Beat {
        title: "First lap",
        say: "Run it, and press f twice for four times the speed. The Mule turns to each checkpoint in turn and goes all the way round: a lap in about 53 seconds, one star. Hana clicks the stopwatch.",
        acts: &[Press(Char('r')), Hold(40), Press(Char('f')), Hold(20), Press(Char('f')), UntilDone(LAP), Hold(300), Press(Esc)],
    },
    Beat {
        title: "The live bench",
        say: "While a rover runs, Tab shows the live bench: every wire lit with the number going through it. Watch the compass swing as one checkpoint is passed and it turns to the next. Tab again goes back to the ice.",
        acts: &[Press(Char('r')), Hold(90), Press(Tab), Hold(480), Press(Tab), Hold(90), Press(Esc)],
    },
    Beat {
        title: "More power",
        say: "Faster, then. On each track, Enter opens its power; type 100 and press Enter. Full power on both sides.",
        acts: &[On(Kind::TrackLeft, 0, 0), Type(0, "100"), On(Kind::TrackRight, 0, 0), Type(0, "100")],
    },
    Beat {
        title: "Second lap",
        say: "Full power: round in about 40 seconds, two stars. Three stars want 39 seconds or less, and this Mule takes every turn as fast as the straights. It needs to ease off where it turns: at the checkpoints.",
        acts: &[Press(Char('r')), UntilDone(LAP), Hold(300), Press(Esc)],
    },
    Beat {
        title: "How far",
        say: "A second compass, set to distance (\u{2192} twice): metres to the next checkpoint. A compare asks a question of it: is a greater than b? Set b to 3. It gives 1 while the checkpoint is more than 3 metres off, and 0 once it is close.",
        acts: &[Goto(SENSE_X, 14), Add(Kind::Compass), Choose(0, 2), Goto(LOGIC_X, 14), Add(Kind::Compare), Type(1, "3"), Wire((Kind::Compass, 1), (Kind::Compare, 0), 0)],
    },
    Beat {
        title: "Choosing a speed",
        say: "An OR gives a when a is on, else b. Into a goes the compare's answer; into b the number, now set to 0.4. Far from a checkpoint the OR gives 1, full speed; close to one it gives 0.4.",
        acts: &[Goto(LOGIC_X, 8), Add(Kind::Or), Wire((Kind::Compare, 0), (Kind::Or, 0), 0), On(Kind::Number, 0, 0), Type(0, "0.4"), Wire((Kind::Number, 0), (Kind::Or, 0), 1)],
    },
    Beat {
        title: "Rewiring",
        say: "Now wire the OR into both tracks' fwd. Wiring into an input that is already taken replaces the old wire, so the number stops driving the tracks itself and only speaks through the OR.",
        acts: &[Wire((Kind::Or, 0), (Kind::TrackLeft, 0), 0), Wire((Kind::Or, 0), (Kind::TrackRight, 0), 0)],
    },
    Beat {
        title: "Third lap",
        say: "Flat out down the straights, easing off for every checkpoint: round in about 36 seconds, three stars. Ines writes something in the notebook and turns it round for you to read: First Light, in pencil.",
        acts: &[Press(Char('r')), UntilDone(LAP), Hold(360), Press(Esc)],
    },
    Beat {
        title: "Your turn",
        say: "That is wiring: sensors feel, logic decides, drive moves. Enter keeps First Light in your garage and leaves you at the bench to change it; Esc goes back to the title. On the bench, p opens the parts library and ? lists every key. The league waits on the title screen.",
        acts: &[],
    },
];

#[derive(Clone, Debug)]
enum Step {
    Key(KeyEvent, String),
    Wait(u32),
    Until(u32),
}

/// What the player had before the lesson, put back when they leave.
struct Saved {
    design: Design,
    stem: Option<String>,
    ch: usize,
    speed: usize,
}

pub struct Lesson {
    pub beat: usize,
    acts: VecDeque<Act>,
    steps: VecDeque<Step>,
    /// Ticks into this beat.
    pub elapsed: u32,
    pub paused: bool,
    /// The keys pressed in this beat, for the caption.
    pub keys: Vec<String>,
    /// Every run's result in order, for the test.
    pub results: Vec<(bool, f64, u8)>,
    /// All beats played; waiting for Enter or Esc.
    pub done: bool,
    saved: Saved,
}

fn label(c: KeyCode) -> String {
    match c {
        Char(' ') => "Space".into(),
        Char(c) => c.to_string(),
        Enter => "Enter".into(),
        Esc => "Esc".into(),
        Tab => "Tab".into(),
        KeyCode::Left => "\u{2190}".into(),
        KeyCode::Right => "\u{2192}".into(),
        KeyCode::Up => "\u{2191}".into(),
        KeyCode::Down => "\u{2193}".into(),
        other => format!("{other:?}"),
    }
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

/// How long a beat stays on screen at least: time to read its caption.
fn reading_ticks(b: &Beat) -> u32 {
    let words = b.say.split_whitespace().count() as u32;
    60 * 3 + words * 60 / 3
}

/// The keys shown in the caption, runs of the same key as one: "→×12".
pub fn keys_shown(keys: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < keys.len() {
        let mut n = 1;
        while i + n < keys.len() && keys[i + n] == keys[i] {
            n += 1;
        }
        out.push(if n > 1 { format!("{}\u{00d7}{n}", keys[i]) } else { keys[i].clone() });
        i += n;
    }
    out.join("  ")
}

impl App {
    /// Starts the lesson on a blank bench in Ring Hall.
    pub fn lesson_start(&mut self) {
        let saved = Saved { design: self.design.clone(), stem: self.stem.clone(), ch: self.ch, speed: self.speed };
        self.lesson = Some(Lesson {
            beat: 0,
            acts: VecDeque::new(),
            steps: VecDeque::new(),
            elapsed: 0,
            paused: false,
            keys: Vec::new(),
            results: Vec::new(),
            done: false,
            saved,
        });
        log::info!("lesson starts");
        self.lesson_reset();
        self.lesson_begin(0);
    }

    /// The world as the lesson starts it: a blank Mule on the bench.
    fn lesson_reset(&mut self) {
        self.stop();
        self.popup = None;
        self.mode = Mode::Edit;
        self.design = Design::blank("First Light");
        // three columns that fit an 80-column terminal: sensors at 4,
        // logic at 22, drive (the blank Mule's tracks and gyro) at 42
        for p in &mut self.design.parts {
            p.x = DRIVE_X;
        }
        self.stem = None;
        self.undo.clear();
        self.ch = challenge::by_id("ring-hall").unwrap_or(0);
        self.chapter = None;
        self.speed = 0;
        self.arena_view = true;
        self.cx = SENSE_X + 2;
        self.cy = 3;
        self.vx = 0;
        self.vy = 0;
        self.msg = None;
        self.relayout();
        self.screen = Screen::Bench;
    }

    fn lesson_begin(&mut self, beat: usize) {
        let Some(l) = &mut self.lesson else { return };
        l.beat = beat;
        l.elapsed = 0;
        l.keys.clear();
        l.steps.clear();
        l.acts = BEATS.get(beat).map(|b| b.acts.iter().copied().collect()).unwrap_or_default();
        l.done = beat + 1 >= BEATS.len();
        log::info!("lesson beat {}: {}", beat + 1, BEATS.get(beat).map(|b| b.title).unwrap_or(""));
    }

    /// Turns an act into key presses and waits, from where things are now.
    fn lesson_expand(&self, act: Act) -> Vec<Step> {
        let mut s = Vec::new();
        let press = |s: &mut Vec<Step>, c: KeyCode, wait: u32| {
            s.push(Step::Key(key(c), label(c)));
            s.push(Step::Wait(wait));
        };
        let walk = |s: &mut Vec<Step>, from: (i32, i32), to: (i32, i32)| {
            let (dx, dy) = (to.0 - from.0, to.1 - from.1);
            let h = if dx < 0 { KeyCode::Left } else { KeyCode::Right };
            let v = if dy < 0 { KeyCode::Up } else { KeyCode::Down };
            for _ in 0..dx.abs() {
                press(s, h, 2);
            }
            for _ in 0..dy.abs() {
                press(s, v, 2);
            }
            s.push(Step::Wait(12));
        };
        let spot = |kind: Kind, nth: usize, row: usize| {
            self.design.parts.iter().filter(|p| p.kind == kind).nth(nth).map(|p| (p.x + 2, p.y + 1 + row as i32))
        };
        match act {
            Press(c) => press(&mut s, c, 20),
            Hold(n) => s.push(Step::Wait(n)),
            UntilDone(n) => s.push(Step::Until(n)),
            Goto(x, y) => walk(&mut s, (self.cx, self.cy), (x, y)),
            On(kind, nth, row) => {
                if let Some(to) = spot(kind, nth, row) {
                    walk(&mut s, (self.cx, self.cy), to);
                }
            }
            Add(kind) => {
                press(&mut s, Char('a'), 30);
                // the add list jumps to the next part starting with a letter
                let lib = crate::app::library();
                let letter = kind.name().chars().next().unwrap_or('a').to_ascii_lowercase();
                let mut sel = 0;
                let mut guard = 0;
                while lib[sel] != kind && guard < lib.len() {
                    let n = lib.len();
                    sel = (1..=n).map(|d| (sel + d) % n).find(|&i| lib[i].name().to_ascii_lowercase().starts_with(letter)).unwrap_or(sel);
                    press(&mut s, Char(letter), 24);
                    guard += 1;
                }
                press(&mut s, Enter, 30);
            }
            Choose(field, n) => {
                press(&mut s, Enter, 24);
                for _ in 0..field {
                    press(&mut s, KeyCode::Down, 12);
                }
                for _ in 0..n {
                    press(&mut s, KeyCode::Right, 24);
                }
                press(&mut s, Enter, 24);
            }
            Type(field, text) => {
                press(&mut s, Enter, 24);
                for _ in 0..field {
                    press(&mut s, KeyCode::Down, 12);
                }
                for c in text.chars() {
                    press(&mut s, Char(c), 12);
                }
                press(&mut s, Enter, 24);
            }
            Wire((fk, fnth), (tk, tnth), port) => {
                let (Some(from), Some(to)) = (spot(fk, fnth, 0), spot(tk, tnth, port)) else { return s };
                walk(&mut s, (self.cx, self.cy), from);
                press(&mut s, Char('w'), 24);
                walk(&mut s, from, to);
                press(&mut s, Enter, 24);
                // a part with more than one input asks which; the cursor's
                // row has already picked it
                if tk.inputs().len() > 1 {
                    press(&mut s, Enter, 24);
                }
            }
        }
        s
    }

    /// One tick of the lesson, before the world's.
    pub fn lesson_tick(&mut self) {
        let Some(l) = &mut self.lesson else { return };
        if l.paused || l.done && l.acts.is_empty() && l.steps.is_empty() {
            return;
        }
        l.elapsed += 1;
        loop {
            let Some(l) = &mut self.lesson else { return };
            if let Some(step) = l.steps.front_mut() {
                match step {
                    Step::Wait(n) => {
                        if *n == 0 {
                            l.steps.pop_front();
                            continue;
                        }
                        *n -= 1;
                        return;
                    }
                    Step::Until(n) => {
                        if self.result.is_some() || *n == 0 || self.sim.is_none() {
                            l.steps.pop_front();
                            continue;
                        }
                        *n -= 1;
                        return;
                    }
                    Step::Key(..) => {
                        let Some(Step::Key(k, name)) = l.steps.pop_front() else { return };
                        l.keys.push(name);
                        self.lesson_press(k);
                        return;
                    }
                }
            }
            if let Some(act) = l.acts.pop_front() {
                let steps = self.lesson_expand(act);
                if let Some(l) = &mut self.lesson {
                    l.steps.extend(steps);
                }
                continue;
            }
            // every key pressed: stay until the caption has been read
            if l.done {
                return;
            }
            if l.elapsed >= reading_ticks(&BEATS[l.beat]) {
                let next = l.beat + 1;
                self.lesson_begin(next);
            }
            return;
        }
    }

    /// A lesson key goes to the game as if typed, and its result is noted.
    fn lesson_press(&mut self, k: KeyEvent) {
        let had = self.result.is_some();
        self.press(k);
        let _ = had;
    }

    /// Plays the rest of this beat at once: keys without waiting, runs to
    /// their end.
    fn lesson_finish_beat(&mut self) {
        loop {
            let Some(l) = &mut self.lesson else { return };
            let step = match l.steps.pop_front() {
                Some(s) => s,
                None => match l.acts.pop_front() {
                    Some(act) => {
                        let steps = self.lesson_expand(act);
                        if let Some(l) = &mut self.lesson {
                            l.steps.extend(steps);
                        }
                        continue;
                    }
                    None => return,
                },
            };
            match step {
                Step::Key(k, name) => {
                    if let Some(l) = &mut self.lesson {
                        l.keys.push(name);
                    }
                    self.lesson_press(k);
                }
                Step::Wait(_) => {}
                Step::Until(n) => {
                    let mut left = n;
                    while self.sim.is_some() && self.result.is_none() && left > 0 {
                        self.world_tick();
                        left -= 1;
                    }
                }
            }
        }
    }

    /// Notes a run's result when its card comes up, for the test.
    pub fn lesson_saw_result(&mut self) {
        if let (Some(l), Some(r)) = (&mut self.lesson, &self.result) {
            l.results.push((r.won, r.secs, r.stars));
        }
    }

    /// Jumps to the start of a beat: the bench rebuilt from scratch with
    /// every beat before it played at once, so it is just as it would be.
    fn lesson_goto(&mut self, beat: usize) {
        self.lesson_reset();
        for b in 0..beat {
            self.lesson_begin(b);
            self.lesson_finish_beat();
        }
        self.stop();
        self.lesson_begin(beat);
    }

    /// The player's keys while the lesson plays.
    pub fn lesson_key(&mut self, k: KeyEvent) {
        let Some(l) = &mut self.lesson else { return };
        let (beat, elapsed, done) = (l.beat, l.elapsed, l.done && l.acts.is_empty() && l.steps.is_empty());
        match k.code {
            Esc | Char('q') => self.lesson_leave(),
            Enter if done => self.lesson_keep(),
            Char(' ') | Char('p') => {
                l.paused = !l.paused;
                let p = l.paused;
                if self.sim.is_some() {
                    self.paused = p;
                }
            }
            KeyCode::Right | Enter | Char('n') | Char('l') => {
                if done {
                    return;
                }
                l.paused = false;
                self.lesson_finish_beat();
                self.stop();
                self.lesson_begin(beat + 1);
            }
            KeyCode::Left | Char('b') | Char('h') => {
                // back to this beat's start, or the one before if just begun
                let to = if elapsed > 90 || beat == 0 { beat } else { beat - 1 };
                if let Some(l) = &mut self.lesson {
                    l.paused = false;
                }
                self.lesson_goto(to);
            }
            _ => {}
        }
    }

    /// Leaves the lesson and puts back the player's own rover.
    pub fn lesson_leave(&mut self) {
        let Some(l) = self.lesson.take() else { return };
        self.stop();
        self.popup = None;
        self.mode = Mode::Edit;
        self.design = l.saved.design;
        self.stem = l.saved.stem;
        self.ch = l.saved.ch;
        self.speed = l.saved.speed;
        self.undo.clear();
        self.relayout();
        self.msg = None;
        log::info!("lesson left at beat {}", l.beat + 1);
        self.go(Screen::Title);
    }

    /// Keeps the lesson's rover as a new rover in the garage.
    fn lesson_keep(&mut self) {
        if self.lesson.take().is_none() {
            return;
        }
        self.stop();
        self.stem = None;
        self.undo.clear();
        self.save();
        self.say("First Light is in your garage. Change something, then r to run it");
        log::info!("lesson rover kept");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays the whole lesson as a viewer would and checks every run
    /// against what the captions say.
    #[test]
    fn the_lesson_plays_through_and_every_run_is_as_told() {
        let mut app = App::new(true);
        app.lesson_start();
        let mut ticks = 0;
        while app.lesson.as_ref().is_some_and(|l| !(l.done && l.acts.is_empty() && l.steps.is_empty())) && ticks < 60 * 60 * 20 {
            app.tick();
            ticks += 1;
        }
        let l = app.lesson.as_ref().expect("still in the lesson");
        assert!(l.done, "the lesson reached its last beat");
        let runs = &l.results;
        // four runs end with a card: three laps (first lap, second, third)
        // and the live-bench run is stopped before its end
        let laps: Vec<_> = runs.iter().filter(|r| r.0).collect();
        assert_eq!(laps.len(), 3, "three laps won: {runs:?}");
        let (t1, t2, t3) = (laps[0].1, laps[1].1, laps[2].1);
        assert!((52.0..55.0).contains(&t1) && laps[0].2 == 1, "first lap about 53 s, one star: {t1} {}", laps[0].2);
        assert!((39.0..42.0).contains(&t2) && laps[1].2 == 2, "second about 40 s, two stars: {t2} {}", laps[1].2);
        assert!((35.0..38.0).contains(&t3) && laps[2].2 == 3, "third about 36 s, three stars: {t3} {}", laps[2].2);
        // the finished rover is the one the captions build
        let d = &app.design;
        assert_eq!(d.name, "First Light");
        assert_eq!(d.parts.len(), 8);
        assert_eq!(d.wires.len(), 6);
    }

    #[test]
    fn skipping_and_going_back_land_on_the_same_bench() {
        let mut app = App::new(true);
        app.lesson_start();
        // skip to beat 13 (rewiring) by pressing → repeatedly
        for _ in 0..13 {
            app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
        let skipped = app.design.to_text();
        // back to the same beat's start, rebuilt from scratch
        app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.lesson.as_ref().map(|l| l.beat), Some(12));
        app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.design.to_text(), skipped);
        // leaving puts the player's own rover back
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.lesson.is_none());
        assert_eq!(app.design.name, "Homer");
        assert_eq!(app.screen, Screen::Title);
    }
}
