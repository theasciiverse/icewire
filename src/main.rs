//! Icewire: wire a rover's sensors, logic and drive on a bench, then send
//! it out on the ice of Europa.

mod app;
mod bot;
mod brain;
mod campaign;
mod challenge;
mod design;
mod geom;
mod lua;
mod parts;
mod replay;
mod route;
mod sim;
mod store;
mod tournament;
mod ui;

use app::{App, Screen};
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_micros(16_667);
const SNAPSHOT_EVERY: Duration = Duration::from_secs(1);

const HELP: &str = "icewire [options]

Wire a rover's sensors, logic and drive on a bench, then send it into
the night league's challenges on Europa. In the game, ? shows the keys.

  --challenge ID   start on the bench with this challenge picked:
                   ring-hall, cold-stores, puck,
                   boiler-room, long-window, laundry or dome
  --demo           watch a short tour (nothing is saved)
  --replay FILE    watch a saved match
  --tournament ... rovers and bots fight each other headless; see
                   icewire --tournament --help
  --version

Rovers are plain text in $XDG_STATE_HOME/icewire/rovers/ (~/.local/state/...),
beside records.txt, settings.txt and the session log, session.log
(level from ICEWIRE_LOG, default info).";

fn version() -> String {
    format!("Icewire {}", env!("CARGO_PKG_VERSION"))
}

fn init_logging() -> Option<PathBuf> {
    let path = store::state_dir()?.join("session.log");
    let file = std::fs::File::create(&path).ok()?;
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_env("ICEWIRE_LOG")
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format(|buf, rec| writeln!(buf, "{} {:<5} {}", buf.timestamp_millis(), rec.level(), rec.args()))
        .init();
    Some(path)
}

/// One step of the demo tour.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Key(KeyCode),
    /// Put this rover on the bench.
    Load(&'static str),
}

/// The demo tour: keys at ticks (60 a second).
fn tour() -> Vec<(u32, Step)> {
    use KeyCode::*;
    let k = |t: u32, c: KeyCode| (t, Step::Key(c));
    // the league, night 4: the story, the bench, the battle with a look
    // at the live wiring, and what came after
    let mut t = vec![k(90, Enter), k(150, Down), k(175, Down), k(200, Down), k(260, Enter), k(560, Enter), (561, Step::Load(design::SLEET))];
    t.extend([k(640, Tab), k(670, Tab), k(740, Char('r')), k(1100, Tab), k(1340, Tab)]);
    t.extend([k(1600, Enter), k(1850, Enter)]);
    // then the same rover as Lua: compile it, read it, run it again
    t.extend([k(1900, Esc), k(1960, Down), k(1990, Down), k(2020, Enter), k(2100, Char('L'))]);
    t.extend([k(2250, PageDown), k(2400, PageDown), k(2520, Char('r')), k(2600, Tab), k(2840, Tab)]);
    t.push(k(3370, Esc));
    t
}

fn main() -> ExitCode {
    let mut demo = false;
    let mut challenge = None;
    let mut replay_file = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(at) = args.iter().position(|a| a == "--tournament") {
        let _ = init_logging();
        log::info!("{} tournament", version());
        return match tournament::main(&args[at + 1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("icewire: {e}");
                ExitCode::from(2)
            }
        };
    }
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "--version" | "-V" => {
                println!("{}", version());
                return ExitCode::SUCCESS;
            }
            "--demo" => demo = true,
            "--replay" => {
                let Some(f) = args.get(i + 1) else {
                    eprintln!("icewire: --replay needs a replay file");
                    return ExitCode::from(2);
                };
                match std::fs::read_to_string(f).map_err(|e| e.to_string()).and_then(|t| replay::Replay::parse(&t)) {
                    Ok(r) => replay_file = Some(r),
                    Err(e) => {
                        eprintln!("icewire: {f}: {e}");
                        return ExitCode::from(2);
                    }
                }
                i += 1;
            }
            "--challenge" => {
                let Some(id) = args.get(i + 1) else {
                    eprintln!("icewire: --challenge needs a challenge id (see --help)");
                    return ExitCode::from(2);
                };
                let Some(c) = challenge::by_id(id) else {
                    eprintln!("icewire: no challenge {id} (see --help)");
                    return ExitCode::from(2);
                };
                challenge = Some(c);
                i += 1;
            }
            other => {
                eprintln!("icewire: unknown option {other} (try --help)");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    let log_path = init_logging();
    log::info!("{} starting, demo {demo}", version());
    let mut app = App::new(demo);
    if demo {
        app.design = design::Design::from_text(design::COMET).expect("demo rover");
        app.ch = 0;
        app.relayout();
    }
    if let Some(c) = challenge {
        app.ch = c;
        app.screen = Screen::Bench;
    }
    if let Some(r) = replay_file {
        app.watch(r);
    }
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let result = run(&mut terminal, &mut app);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    if !demo {
        app.save();
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            eprintln!("icewire: {e}");
            if let Some(p) = log_path {
                eprintln!("session log: {}", p.display());
            }
            ExitCode::FAILURE
        }
    }
}

/// Hands the terminal to the player's editor ($VISUAL, $EDITOR, else nano
/// or vi; Notepad on Windows) for one file, then takes it back.
fn edit_file(terminal: &mut ratatui::DefaultTerminal, path: &std::path::Path) -> Result<(), String> {
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    let mut choices: Vec<String> = ["VISUAL", "EDITOR"].iter().filter_map(|v| std::env::var(v).ok()).filter(|v| !v.trim().is_empty()).collect();
    if cfg!(windows) {
        choices.push("notepad".into());
    } else {
        choices.extend(["nano".into(), "vi".into()]);
    }
    let mut result = Err("No editor found: set EDITOR to your favourite".to_string());
    for c in choices {
        let mut words = c.split_whitespace();
        let Some(prog) = words.next() else { continue };
        match std::process::Command::new(prog).args(words).arg(path).status() {
            Ok(_) => {
                result = Ok(());
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                result = Err(format!("{prog}: {e}"));
                break;
            }
        }
    }
    *terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let _ = terminal.clear();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    let script = if app.demo { tour() } else { Vec::new() };
    let mut next = Instant::now();
    let mut snapshot = Instant::now();
    let mut tick: u32 = 0;
    let mut dirty = true;
    loop {
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    log::debug!("key {:?} {:?}", k.code, k.modifiers);
                    if app.demo {
                        if matches!(k.code, KeyCode::Esc | KeyCode::Char('q')) || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)) {
                            return Ok(());
                        }
                    } else {
                        app.key(k);
                    }
                    dirty = true;
                }
                Event::Mouse(m) => {
                    log::trace!("mouse {:?} at {},{}", m.kind, m.column, m.row);
                    app.mouse(m);
                    dirty = true;
                }
                Event::Resize(w, h) => {
                    log::info!("resize {w}x{h}");
                    dirty = true;
                }
                _ => {}
            }
        }
        if let Some(path) = app.edit_request.take() {
            let how = edit_file(terminal, &path);
            log::info!("editor closed: {how:?}");
            app.after_edit(&path, how);
            dirty = true;
            next = Instant::now();
        }
        let now = Instant::now();
        if now < next {
            let _ = event::poll(next - now)?;
            continue;
        }
        next += TICK;
        if now - next > Duration::from_millis(250) {
            next = now;
        }
        for (_, step) in script.iter().filter(|(t, _)| *t == tick) {
            if tick == script.last().map(|s| s.0).unwrap_or(0) {
                return Ok(());
            }
            match *step {
                Step::Key(k) => app.key(KeyEvent::new(k, KeyModifiers::NONE)),
                Step::Load(src) => {
                    app.design = design::Design::from_text(src).expect("demo rover");
                    app.relayout();
                }
            }
            dirty = true;
        }
        app.tick();
        tick += 1;
        if app.exit {
            return Ok(());
        }
        // 30 frames a second while something moves, else on change
        if dirty || (tick.is_multiple_of(2) && (app.running() || app.msg.is_some())) {
            terminal.draw(|f| ui::draw(f, app))?;
            dirty = false;
        }
        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            match &app.sim {
                Some(s) => {
                    let r = &s.rovers[0].rover;
                    log::debug!(
                        "snapshot {:?} {} t {:.1} at ({:.2},{:.2}) facing {:.0} v {:.2} drive {:?} outcome {:?} paused {} speed x{}",
                        app.screen,
                        s.ch.id,
                        s.secs(),
                        r.x,
                        r.y,
                        r.heading_deg(),
                        r.speed(),
                        r.drive,
                        s.outcome,
                        app.paused,
                        app::SPEEDS[app.speed]
                    );
                }
                None => log::debug!("snapshot {:?} rover {:?} parts {} wires {} cursor {},{} mode {:?} popup {:?}", app.screen, app.design.name, app.design.parts.len(), app.design.wires.len(), app.cx, app.cy, app.mode, app.popup),
            }
        }
    }
}
