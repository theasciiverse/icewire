//! Icewire: wire a rover's sensors, logic and drive on a bench, then send
//! it out on the ice of Europa.

mod app;
mod brain;
mod challenge;
mod design;
mod geom;
mod parts;
mod route;
mod sim;
mod store;
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
                   ring-hall, cold-stores or puck
  --demo           watch a short tour (nothing is saved)
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

/// The demo tour: keys at ticks (60 a second).
fn tour() -> Vec<(u32, KeyCode)> {
    use KeyCode::*;
    // the bench, a look inside the compare, Ring Hall with the live wiring
    let mut t = vec![(90, Enter), (170, Tab), (200, Tab), (230, Tab), (260, Tab), (300, Enter), (350, Right), (390, Left), (430, Enter), (470, Char('r'))];
    t.extend([(830, Tab), (1060, Tab), (1080, Char('f'))]);
    // then the Puck: Comet eases off near its target, so give it full power first
    t.extend([(2010, Enter), (2060, Char('c')), (2100, Char('c')), (2150, Tab), (2180, Tab), (2210, Tab), (2260, Enter), (2310, Char('1')), (2360, Enter), (2420, Char('r'))]);
    t.extend([(2760, Tab), (2940, Tab), (2960, Char('f'))]);
    t.push((3460, Esc));
    t
}

fn main() -> ExitCode {
    let mut demo = false;
    let mut challenge = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
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
            "--challenge" => {
                let Some(id) = args.get(i + 1) else {
                    eprintln!("icewire: --challenge needs ring-hall, cold-stores or puck");
                    return ExitCode::from(2);
                };
                let Some(c) = challenge::by_id(id) else {
                    eprintln!("icewire: no challenge {id}; try ring-hall, cold-stores or puck");
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
        let now = Instant::now();
        if now < next {
            let _ = event::poll(next - now)?;
            continue;
        }
        next += TICK;
        if now - next > Duration::from_millis(250) {
            next = now;
        }
        for (_, k) in script.iter().filter(|(t, _)| *t == tick) {
            if *k == KeyCode::Esc && tick == script.last().map(|s| s.0).unwrap_or(0) {
                return Ok(());
            }
            app.key(KeyEvent::new(*k, KeyModifiers::NONE));
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
