//! Lua rovers. Any wiring compiles to a readable Lua script that drives
//! the rover exactly as the wiring does, and a script can be edited or
//! written from scratch. Scripts run sandboxed: no io, no os, no loading
//! of other code, a memory cap, and a budget of instructions per tick.

use crate::brain::{Brain, Drive};
use crate::design::Design;
use crate::parts::{self, Kind, LOB_MAX, LOB_MIN, Mount};
use crate::sim::Sim;
use mlua::{ChunkMode, Function, HookTriggers, Lua, LuaOptions, StdLib, VmState};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

/// Instructions a script may run in one tick.
pub const BUDGET: u32 = 50_000;
const HOOK_EVERY: u32 = 1_000;
const MEMORY: usize = 16 * 1024 * 1024;
const CONSOLE: usize = 60;

/// The helpers every script gets, in plain Lua.
const PRELUDE: &str = r#"
function on(x) return x ~= nil and x ~= false and x ~= 0 end
function bit(b) if b then return 1.0 else return 0.0 end end
function clamp(x, lo, hi)
  lo = lo or -1.0
  hi = hi or 1.0
  if x < lo then return lo elseif x > hi then return hi end
  return x
end
"#;

pub struct LuaBrain {
    lua: Lua,
    used: Rc<Cell<u32>>,
    /// What the script printed, newest last.
    pub console: Rc<RefCell<VecDeque<String>>>,
    /// The last error, if the last tick failed.
    pub error: Option<String>,
}

fn mount(side: Option<String>, default: Mount, kind: Kind) -> mlua::Result<Mount> {
    match side {
        None => Ok(default),
        Some(s) => Mount::parse(&s.to_lowercase()).filter(|m| kind.mounts().contains(m)).ok_or_else(|| mlua::Error::runtime(format!("{}: no side called {s:?} (try nose, right, tail or left)", kind.key()))),
    }
}

fn mode(m: Option<String>, modes: &[&str], default: usize, what: &str) -> mlua::Result<usize> {
    match m {
        None => Ok(default),
        Some(s) => modes.iter().position(|x| *x == s.to_lowercase()).ok_or_else(|| mlua::Error::runtime(format!("{what}: no mode called {s:?} (try {})", modes.join(", ")))),
    }
}

/// Keeps a drive value sane: -1 to 1, and 0 for anything not a number.
fn sane(x: f64) -> f64 {
    if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 }
}

impl std::fmt::Debug for LuaBrain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LuaBrain {{ error: {:?} }}", self.error)
    }
}

impl LuaBrain {
    pub fn new(script: &str) -> Result<LuaBrain, String> {
        let lua = Lua::new_with(StdLib::MATH | StdLib::STRING | StdLib::TABLE | StdLib::UTF8, LuaOptions::default()).map_err(|e| e.to_string())?;
        lua.set_memory_limit(MEMORY).map_err(|e| e.to_string())?;
        let used = Rc::new(Cell::new(0u32));
        let u = used.clone();
        lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_EVERY), move |_, _| {
            u.set(u.get() + HOOK_EVERY);
            if u.get() > BUDGET {
                Err(mlua::Error::runtime(format!("over the budget of {BUDGET} instructions in one tick")))
            } else {
                Ok(VmState::Continue)
            }
        })
        .map_err(|e| e.to_string())?;
        let console = Rc::new(RefCell::new(VecDeque::new()));
        let setup = || -> mlua::Result<()> {
            let g = lua.globals();
            for name in ["dofile", "loadfile", "load", "require", "collectgarbage"] {
                g.set(name, mlua::Value::Nil)?;
            }
            let string: mlua::Table = g.get("string")?;
            string.set("dump", mlua::Value::Nil)?;
            let c = console.clone();
            g.set(
                "print",
                lua.create_function(move |lua, args: mlua::MultiValue| {
                    let parts: Vec<String> = args.iter().map(|v| lua.coerce_string(v.clone()).ok().flatten().map(|s| s.to_string_lossy()).unwrap_or_else(|| format!("{v:?}"))).collect();
                    let mut c = c.borrow_mut();
                    c.push_back(parts.join("  "));
                    while c.len() > CONSOLE {
                        c.pop_front();
                    }
                    Ok(())
                })?,
            )?;
            g.set("round", lua.create_function(|_, x: f64| Ok(x.round()))?)?;
            lua.load(PRELUDE).set_name("prelude").set_mode(ChunkMode::Text).exec()?;
            lua.load("math.randomseed(1)").exec()?;
            Ok(())
        };
        setup().map_err(|e| e.to_string())?;
        used.set(0);
        lua.load(script).set_name("rover").set_mode(ChunkMode::Text).exec().map_err(|e| tidy(&e.to_string()))?;
        if lua.globals().get::<Function>("tick").is_err() {
            return Err("the script has no tick() function".into());
        }
        Ok(LuaBrain { lua, used, console, error: None })
    }

    /// Runs the script's tick() for rover `i` and returns what it asked of
    /// the drive and weapons.
    pub fn tick(&mut self, sim: &mut Sim, i: usize) -> Drive {
        self.used.set(0);
        let sim = RefCell::new(sim);
        let drive = RefCell::new(Drive::default());
        let read = |kind: Kind, m: Mount, mode: usize, value: f64| sim.borrow_mut().read(i, kind, m, mode, value);
        let res = self.lua.scope(|scope| {
            let g = self.lua.globals();
            g.set("bumper", scope.create_function(|_, side: Option<String>| Ok(read(Kind::Bumper, mount(side, Mount::All, Kind::Bumper)?, 0, 0.0)))?)?;
            g.set(
                "whisker",
                scope.create_function(|_, (side, reach): (Option<String>, Option<f64>)| {
                    let reach = reach.unwrap_or(1.0);
                    let reach = if reach.is_finite() { reach.clamp(0.1, 4.0) } else { 1.0 };
                    Ok(read(Kind::Whisker, mount(side, Mount::Nose, Kind::Whisker)?, 0, reach))
                })?,
            )?;
            g.set("ranger", scope.create_function(|_, side: Option<String>| Ok(read(Kind::Ranger, mount(side, Mount::Nose, Kind::Ranger)?, 0, 0.0)))?)?;
            g.set("compass", scope.create_function(|_, m: Option<String>| Ok(read(Kind::Compass, Mount::Nose, mode(m, &parts::COMPASS_MODES, 1, "compass")?, 0.0)))?)?;
            g.set("radar", scope.create_function(|_, m: Option<String>| Ok(read(Kind::Radar, Mount::Nose, mode(m, &parts::RADAR_MODES, 0, "radar")?, 0.0)))?)?;
            g.set("gauge", scope.create_function(|_, ()| Ok(read(Kind::Gauge, Mount::Nose, 0, 0.0)))?)?;
            g.set(
                "loaded",
                scope.create_function(|_, w: Option<String>| {
                    let kind = match w.as_deref().unwrap_or("pellet") {
                        "pellet" | "pellet-gun" | "gun" => Kind::PelletGun,
                        "lobber" | "lob" => Kind::Lobber,
                        other => return Err(mlua::Error::runtime(format!("loaded: no weapon called {other:?} (try \"pellet\" or \"lobber\")"))),
                    };
                    Ok(read(kind, Mount::Nose, 0, 0.0))
                })?,
            )?;
            g.set("time", scope.create_function(|_, ()| Ok(sim.borrow().secs()))?)?;
            g.set(
                "tracks",
                scope.create_function(|_, (l, r): (f64, f64)| {
                    let mut d = drive.borrow_mut();
                    d.left = sane(l);
                    d.right = sane(r);
                    Ok(())
                })?,
            )?;
            g.set(
                "gyro",
                scope.create_function(|_, x: f64| {
                    drive.borrow_mut().gyro = sane(x);
                    Ok(())
                })?,
            )?;
            g.set(
                "fire",
                scope.create_function(|_, ()| {
                    drive.borrow_mut().fire = true;
                    Ok(())
                })?,
            )?;
            g.set(
                "lob",
                scope.create_function(|_, range: f64| {
                    let r = if range.is_finite() { range.clamp(LOB_MIN, LOB_MAX) } else { LOB_MIN };
                    drive.borrow_mut().lob = Some(r);
                    Ok(())
                })?,
            )?;
            let tick: Function = g.get("tick")?;
            tick.call::<()>(())
        });
        match res {
            Ok(()) => {
                self.error = None;
                drive.into_inner()
            }
            Err(e) => {
                self.error = Some(tidy(&e.to_string()));
                Drive::default()
            }
        }
    }
}

/// Lua's error text, first line, without the traceback.
fn tidy(e: &str) -> String {
    let first = e.lines().next().unwrap_or(e).trim();
    first.strip_prefix("runtime error: ").or_else(|| first.strip_prefix("syntax error: ")).unwrap_or(first).replace("[string \"rover\"]", "line").to_string()
}

/// A number as Lua source that reads back as exactly the same double.
fn num(v: f64) -> String {
    if v.is_finite() { format!("{v:?}") } else { "0.0".into() }
}

/// Compiles a wiring to Lua that drives the rover exactly as the wiring
/// does: one line (or a few) per part, in the order the wiring runs.
pub fn compile(d: &Design) -> String {
    let b = Brain::new(d);
    let mut s = String::new();
    s.push_str(&format!("{} {}\n", crate::design::LUA_HEADER, d.name));
    s.push_str(&format!(
        "-- Compiled from {}'s wiring by Icewire {}.
--
-- Icewire calls tick() sixty times a second. Each part
-- of the wiring is a line or two below, in the order the
-- wiring runs; out[n] is part n's output. A wire from a
-- part further down reads its value from the last tick.
-- Change anything: this is your rover now.
--
-- Sensors  bumper(side) whisker(side, reach)
--          ranger(side) gauge() time()
--          compass(\"heading\"|\"bearing\"|\"distance\")
--          radar(\"bearing\"|\"distance\")
-- Drive    tracks(left, right) gyro(spin), -1 to 1
-- Weapons  fire() lob(metres) loaded(\"pellet\"|\"lobber\")
-- Helpers  on(x) bit(test) clamp(x) round(x) print(...)
-- Sides    \"nose\" \"right\" \"tail\" \"left\" (bumper: \"all\")

",
        d.name,
        env!("CARGO_PKG_VERSION")
    ));
    let ids: Vec<String> = d.parts.iter().map(|p| format!("[{}] = 0.0", p.id)).collect();
    s.push_str(&format!("local out = {{ {} }}\n", ids.join(", ")));
    s.push_str("local last = {}  -- each part's input on the tick before\n");
    s.push_str("local timer, toggle, count, held = {}, {}, {}, {}\n\n");
    s.push_str("-- true when x has just turned on since the tick before\n");
    s.push_str("local function rising(n, x)\n  local r = on(x) and not on(last[n] or 0.0)\n  last[n] = x\n  return r\nend\n\n");
    s.push_str("function tick()\n  local left, right, spin = 0.0, 0.0, 0.0\n");
    for &i in &b.order {
        let p = &d.parts[i];
        let n = p.id;
        let input = |k: usize| -> Option<String> { b.feeds[i][k].map(|src| format!("out[{}]", d.parts[src].id)) };
        let a = input(0).unwrap_or_else(|| "0.0".into());
        let b_in = input(1).unwrap_or_else(|| if matches!(p.kind, Kind::Compare | Kind::Lobber) { num(p.value) } else { "0.0".into() });
        let label = p.label(input(1).is_some());
        s.push_str(&format!("\n  -- {n}: {}{}\n", p.kind.name(), if label.is_empty() { String::new() } else { format!(", {label}") }));
        let line = |s: &mut String, code: String| {
            for l in code.lines() {
                s.push_str("  ");
                s.push_str(l);
                s.push('\n');
            }
        };
        match p.kind {
            Kind::Bumper => line(&mut s, format!("out[{n}] = bumper(\"{}\")", p.mount.name())),
            Kind::Whisker => line(&mut s, format!("out[{n}] = whisker(\"{}\", {})", p.mount.name(), num(p.value))),
            Kind::Ranger => line(&mut s, format!("out[{n}] = ranger(\"{}\")", p.mount.name())),
            Kind::Compass => line(&mut s, format!("out[{n}] = compass(\"{}\")", parts::COMPASS_MODES[p.mode])),
            Kind::Radar => line(&mut s, format!("out[{n}] = radar(\"{}\")", parts::RADAR_MODES[p.mode])),
            Kind::Gauge => line(&mut s, format!("out[{n}] = gauge()")),
            Kind::And => line(&mut s, format!("if on({a}) then out[{n}] = {b_in} else out[{n}] = 0.0 end")),
            Kind::Or => line(&mut s, format!("if on({a}) then out[{n}] = {a} else out[{n}] = {b_in} end")),
            Kind::Not => line(&mut s, format!("out[{n}] = bit(not on({a}))")),
            Kind::Compare => {
                let test = match p.mode {
                    0 => format!("{a} > {b_in}"),
                    1 => format!("{a} < {b_in}"),
                    _ => format!("round({a} * 10.0) == round({b_in} * 10.0)"),
                };
                line(&mut s, format!("out[{n}] = bit({test})"))
            }
            Kind::Timer => {
                let ticks = (p.value * crate::sim::TICKS_PER_SEC as f64).round().max(1.0);
                line(&mut s, format!("if rising({n}, {a}) then timer[{n}] = {} end\nif (timer[{n}] or 0) > 0 then\n  timer[{n}] = timer[{n}] - 1\n  out[{n}] = 1.0\nelse\n  out[{n}] = 0.0\nend", ticks as u32))
            }
            Kind::Toggle => line(&mut s, format!("if rising({n}, {a}) then toggle[{n}] = not toggle[{n}] end\nout[{n}] = bit(toggle[{n}])")),
            Kind::Counter => line(&mut s, format!("local turned_on = rising({n}, {a})\nif on({b_in}) then\n  count[{n}] = 0.0\nelseif turned_on then\n  count[{n}] = (count[{n}] or 0.0) + 1.0\nend\nout[{n}] = count[{n}] or 0.0")),
            Kind::Memory => line(&mut s, format!("if on({b_in}) then held[{n}] = {a} end\nout[{n}] = held[{n}] or 0.0")),
            Kind::Number => line(&mut s, format!("out[{n}] = {}", num(p.value))),
            Kind::TrackLeft | Kind::TrackRight | Kind::Gyro => {
                let var = match p.kind {
                    Kind::TrackLeft => "left",
                    Kind::TrackRight => "right",
                    _ => "spin",
                };
                line(&mut s, format!("out[{n}] = clamp(({a} - {b_in}) * {} / 100.0)\n{var} = out[{n}]", num(p.value)))
            }
            Kind::PelletGun => line(&mut s, format!("if on({a}) then fire() end\nout[{n}] = loaded(\"pellet\")")),
            Kind::Lobber => line(&mut s, format!("if on({a}) then lob({b_in}) end\nout[{n}] = loaded(\"lobber\")")),
        }
    }
    s.push_str("\n  tracks(left, right)\n  gyro(spin)\nend\n");
    s
}

/// A short starting script for a rover written from scratch.
pub const STARTER: &str = "-- Icewire rover: Scribbler
-- A rover in plain Lua. Icewire calls tick() sixty
-- times a second.
--
-- Sensors  bumper(side) whisker(side, reach)
--          ranger(side) gauge() time()
--          compass(\"heading\"|\"bearing\"|\"distance\")
--          radar(\"bearing\"|\"distance\")
-- Drive    tracks(left, right) gyro(spin), -1 to 1
-- Weapons  fire() lob(metres) loaded(\"pellet\"|\"lobber\")
-- Helpers  on(x) bit(test) clamp(x) round(x) print(...)
-- Sides    \"nose\" \"right\" \"tail\" \"left\" (bumper: \"all\")

function tick()
  -- turn toward the beacon, ease off when a wall is close
  local bearing = compass(\"bearing\")
  gyro(clamp(bearing / 30))
  local ahead = ranger(\"nose\")
  local speed = clamp(ahead / 3, 0.3, 1.0)
  tracks(speed, speed)
end
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::challenge;
    use crate::design::{self, Design};
    use crate::sim::Outcome;

    fn run(ch: &str, d: Design) -> Sim {
        let mut s = Sim::new(challenge::by_id(ch).unwrap(), d);
        s.run(60 * 200);
        s
    }

    /// The compiled script must drive the rover exactly as its wiring.
    #[test]
    fn compiled_lua_runs_exactly_like_the_wiring() {
        let cases = [
            ("ring-hall", design::STARTER),
            ("long-window", design::COMET),
            ("puck", design::TUMBLER),
            ("boiler-room", design::SLEET),
            ("laundry", design::KETTLE),
            ("dome", design::NIGHTJAR),
            ("boiler-room", design::SPROCKET),
        ];
        for (ch, src) in cases {
            let wired = Design::from_text(src).unwrap();
            let script = compile(&wired);
            let lua = Design::from_lua(&script, "x");
            assert_eq!(lua.name, wired.name);
            let (a, b) = (run(ch, wired), run(ch, lua));
            assert_eq!(a.outcome, b.outcome, "{ch} {src}");
            assert_eq!(a.tick, b.tick, "{ch}");
            let (ra, rb) = (&a.rovers[0].rover, &b.rovers[0].rover);
            assert_eq!((ra.x.to_bits(), ra.y.to_bits(), ra.charge.to_bits()), (rb.x.to_bits(), rb.y.to_bits(), rb.charge.to_bits()), "{ch}: {}", script);
        }
    }

    #[test]
    fn compiled_logic_parts_match() {
        // every logic part in one rover, compared tick by tick on the race
        let src = "icewire rover 1
name Everything
part 1 bumper at 4 2
part 2 compass at 4 9 mode distance
part 3 counter at 22 2
part 4 memory at 22 9
part 5 timer at 22 16 value 0.4
part 6 toggle at 40 2
part 7 compare at 40 9 mode = value 3
part 8 or at 40 16
part 9 not at 58 2
part 10 number at 4 16 value 0.5
part 11 track-left at 76 2 value 80
part 12 track-right at 76 9 value 70
part 13 gyro at 76 16 value 30
part 14 compass at 4 23 mode bearing
part 15 and at 58 9
wire 1 to 3 in
wire 1 to 5 in
wire 2 to 4 data
wire 1 to 4 store
wire 5 to 6 in
wire 2 to 7 a
wire 7 to 8 a
wire 10 to 8 b
wire 5 to 9 in
wire 8 to 11 fwd
wire 9 to 12 fwd
wire 14 to 13 right
wire 6 to 15 a
wire 10 to 15 b
wire 15 to 13 left
";
        let wired = Design::from_text(src).unwrap();
        let lua = Design::from_lua(&compile(&wired), "x");
        let mut a = Sim::new(0, wired);
        let mut b = Sim::new(0, lua);
        for t in 0..3000 {
            a.step();
            b.step();
            let (ra, rb) = (&a.rovers[0].rover, &b.rovers[0].rover);
            assert_eq!((ra.x.to_bits(), ra.y.to_bits()), (rb.x.to_bits(), rb.y.to_bits()), "tick {t}");
        }
    }

    #[test]
    fn the_sandbox_holds() {
        for (bad, why) in [
            ("io.write('x')\nfunction tick() end", "io"),
            ("os.exit(1)\nfunction tick() end", "os"),
            ("load('return 1')\nfunction tick() end", "load"),
            ("require('x')\nfunction tick() end", "require"),
            ("dofile('/etc/passwd')\nfunction tick() end", "dofile"),
        ] {
            assert!(LuaBrain::new(bad).is_err(), "{why} should be out of reach");
        }
        assert!(LuaBrain::new("x = 1").unwrap_err().contains("tick"));
        assert!(LuaBrain::new("function tick( end").is_err());
    }

    #[test]
    fn a_runaway_tick_hits_the_budget_and_the_run_goes_on() {
        let d = Design::from_lua("function tick() tracks(1, 1) while true do end end", "Spinner");
        let mut s = Sim::new(0, d);
        for _ in 0..10 {
            s.step();
        }
        assert_eq!(s.outcome, Outcome::Running);
        let e = s.rovers[0].lua.as_ref().unwrap().error.clone().unwrap();
        assert!(e.contains("budget"), "{e}");
        assert_eq!(s.rovers[0].rover.speed(), 0.0, "a failed tick drives nothing");
    }

    #[test]
    fn print_goes_to_the_console_and_runs_repeat() {
        let d = Design::from_lua("n = 0\nfunction tick() n = n + 1 if n % 60 == 0 then print('second', n // 60, math.random(100)) end tracks(1, 1) end", "Talker");
        let mut a = Sim::new(0, d.clone());
        let mut b = Sim::new(0, d);
        for _ in 0..180 {
            a.step();
            b.step();
        }
        let ca: Vec<String> = a.rovers[0].lua.as_ref().unwrap().console.borrow().iter().cloned().collect();
        let cb: Vec<String> = b.rovers[0].lua.as_ref().unwrap().console.borrow().iter().cloned().collect();
        assert_eq!(ca.len(), 3);
        assert!(ca[0].starts_with("second  1"), "{ca:?}");
        assert_eq!(ca, cb, "math.random is seeded the same every run");
    }

    #[test]
    fn the_starter_script_finishes_ring_hall() {
        let s = run("ring-hall", Design::from_lua(STARTER, "x"));
        assert!(matches!(s.outcome, Outcome::Won(_)), "{:?}", s.outcome);
    }
}
