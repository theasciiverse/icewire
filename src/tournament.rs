//! `icewire --tournament`: rovers of any kind (wired, Lua or bots) fight
//! each other headless, every pair twice with the starting spots swapped,
//! as fast as the bots can answer. Results go to the terminal and every
//! match is saved as a replay.

use crate::challenge::{self, Goal};
use crate::design::{self, BotSpec, Design};
use crate::replay::Recorder;
use crate::sim::{Outcome, Sim};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const USAGE: &str = "icewire --tournament [options] ROVER ROVER [ROVER...]

Runs every pair of rovers against each other twice, once from each side,
with nobody watching, and saves each match as a replay
(watch one with icewire --replay FILE).

A ROVER is a built-in rover (sprocket, kettle, nightjar, tumbler, sleet,
homer, comet), one of the example bots (shellby, dodger), a .rover, .lua
or .bot file, or any command line that starts a bot (quote it), for
example \"python3 examples/dodger.py\".

  --challenge ID   where they fight: boiler-room (the default), laundry,
                   dome or puck
  --timeout MS     how long a bot has to answer each tick (default 100)
  --replays DIR    where the replays go (default: the state folder's
                   replays/ folder)
  --no-replays     keep no replays";

/// Where the example bots are: beside the binary, or in the folder Icewire
/// was started from.
pub fn examples() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("examples")));
    [beside, Some(PathBuf::from("examples"))].into_iter().flatten().find(|p| p.join("dodger.py").exists()).map(|p| std::fs::canonicalize(&p).unwrap_or(p))
}

/// A tournament entrant from its name on the command line.
pub fn entrant(arg: &str) -> Result<Design, String> {
    let builtin = match arg.to_lowercase().as_str() {
        "sprocket" => Some(design::SPROCKET),
        "kettle" => Some(design::KETTLE),
        "nightjar" => Some(design::NIGHTJAR),
        "tumbler" => Some(design::TUMBLER),
        "sleet" => Some(design::SLEET),
        "homer" => Some(design::STARTER),
        "comet" => Some(design::COMET),
        _ => None,
    };
    if let Some(src) = builtin {
        return Design::from_text(src);
    }
    let example = match arg.to_lowercase().as_str() {
        "shellby" => Some(("Shellby", "sh shellby.sh")),
        "dodger" => Some(("Dodger", "python3 dodger.py")),
        _ => None,
    };
    if let Some((name, cmd)) = example {
        let dir = examples().ok_or(format!("cannot find the examples folder for {name}"))?;
        return Ok(Design { name: name.into(), parts: Vec::new(), wires: Vec::new(), lua: None, bot: Some(BotSpec { command: cmd.into(), dir: Some(dir) }) });
    }
    let path = Path::new(arg);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    match path.extension().and_then(|e| e.to_str()) {
        Some("rover") | Some("lua") | Some("bot") if path.exists() => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{arg}: {e}"))?;
            match path.extension().and_then(|e| e.to_str()) {
                Some("rover") => Design::from_text(&text).map_err(|e| format!("{arg}: {e}")),
                Some("lua") => Ok(Design::from_lua(&text, &stem)),
                _ => Design::from_bot(&text, &stem, path.parent().map(|p| p.to_path_buf())).map_err(|e| format!("{arg}: {e}")),
            }
        }
        _ => {
            // any other command line is a bot, started from here
            let words = crate::bot::words(arg);
            let first = words.iter().find(|w| w.contains('.') || w.contains('/')).or(words.first()).cloned().unwrap_or_default();
            let name = Path::new(&first).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(first);
            Ok(Design { name: design::clean_name(&name), parts: Vec::new(), wires: Vec::new(), lua: None, bot: Some(BotSpec { command: arg.into(), dir: None }) })
        }
    }
}

pub struct Settings {
    pub challenge: usize,
    pub timeout: Duration,
    pub replays: Option<PathBuf>,
}

#[derive(Default, Clone)]
pub struct Standing {
    pub name: String,
    pub won: u32,
    pub drawn: u32,
    pub lost: u32,
}

impl Standing {
    pub fn points(&self) -> u32 {
        self.won * 3 + self.drawn
    }
}

/// One match: who won (0 or 1, None for a draw), how, and when.
pub fn play(ch: usize, a: Design, b: Design, timeout: Duration) -> (Option<usize>, String, f64, String) {
    let (na, nb) = (a.name.clone(), b.name.clone());
    let mut sim = Sim::with(ch, vec![a, b], timeout);
    let mut rec = Recorder::new(&sim);
    while sim.outcome == Outcome::Running {
        sim.step();
        rec.frame(&sim);
    }
    let secs = sim.secs();
    let (winner, how) = match sim.outcome {
        Outcome::Won(_) => (Some(0), format!("{na} wins in {secs:.1} s")),
        Outcome::Lost(why) => (Some(1), format!("{nb} wins in {secs:.1} s ({na}: {why})")),
        _ => (None, format!("a draw after {secs:.1} s")),
    };
    let replay = rec.finish(&how);
    (winner, how, secs, replay)
}

/// Runs the whole tournament, printing as it goes; returns the standings.
pub fn run(entrants: &[Design], s: &Settings, out: &mut dyn std::io::Write) -> Vec<Standing> {
    let ch = &challenge::ALL[s.challenge];
    let n = entrants.len();
    let _ = writeln!(out, "Icewire tournament: {}, {} rovers, every pair twice\n", ch.name, n);
    let folder = s.replays.as_ref().map(|d| {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        d.join(format!("tournament-{stamp}"))
    });
    if let Some(f) = &folder
        && let Err(e) = std::fs::create_dir_all(f)
    {
        let _ = writeln!(out, "(no replays: cannot make {}: {e})", f.display());
    }
    let mut table: Vec<Standing> = entrants.iter().map(|e| Standing { name: e.name.clone(), ..Standing::default() }).collect();
    let mut m = 0;
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            m += 1;
            let (winner, how, _, replay) = play(s.challenge, entrants[i].clone(), entrants[j].clone(), s.timeout);
            match winner {
                Some(0) => {
                    table[i].won += 1;
                    table[j].lost += 1;
                }
                Some(_) => {
                    table[j].won += 1;
                    table[i].lost += 1;
                }
                None => {
                    table[i].drawn += 1;
                    table[j].drawn += 1;
                }
            }
            let mut line = format!("Match {m:>2}: {} against {}: {how}", entrants[i].name, entrants[j].name);
            if let Some(f) = &folder {
                let file = f.join(format!("match-{m:02}-{}-vs-{}.replay", crate::store::stem(&entrants[i].name), crate::store::stem(&entrants[j].name)));
                if std::fs::write(&file, replay).is_ok() {
                    line.push_str(&format!("\n          replay: {}", file.display()));
                }
            }
            let _ = writeln!(out, "{line}");
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&k| (std::cmp::Reverse(table[k].points()), std::cmp::Reverse(table[k].won), k));
    let _ = writeln!(out, "\nStandings (3 for a win, 1 for a draw)");
    for (place, &k) in order.iter().enumerate() {
        let t = &table[k];
        let _ = writeln!(out, "  {}. {:<20} {:>3} points  {} won, {} drawn, {} lost", place + 1, t.name, t.points(), t.won, t.drawn, t.lost);
    }
    order.into_iter().map(|k| table[k].clone()).collect()
}

/// The command line after `--tournament`.
pub fn main(args: &[String]) -> Result<(), String> {
    let mut s = Settings { challenge: challenge::by_id("boiler-room").unwrap_or(0), timeout: crate::sim::BOT_TIMEOUT, replays: crate::store::state_dir().map(|d| d.join("replays")) };
    let mut names = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| args.get(i + 1).cloned().ok_or(format!("{} needs a value", args[i]));
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--challenge" => {
                let id = value(i)?;
                s.challenge = challenge::by_id(&id).ok_or(format!("no challenge {id}"))?;
                i += 1;
            }
            "--timeout" => {
                let v = value(i)?;
                let ms: u64 = v.parse().map_err(|_| format!("--timeout {v}: not a number of milliseconds"))?;
                s.timeout = Duration::from_millis(ms.clamp(1, 10_000));
                i += 1;
            }
            "--replays" => {
                s.replays = Some(PathBuf::from(value(i)?));
                i += 1;
            }
            "--no-replays" => s.replays = None,
            other => names.push(other.to_string()),
        }
        i += 1;
    }
    let goal = challenge::ALL[s.challenge].goal;
    if goal != Goal::Battle && goal != Goal::Sumo {
        return Err(format!("{} is for one rover; pick boiler-room, laundry, dome or puck", challenge::ALL[s.challenge].name));
    }
    if names.len() < 2 {
        return Err(format!("a tournament needs at least two rovers\n\n{USAGE}"));
    }
    let entrants = names.iter().map(|n| entrant(n)).collect::<Result<Vec<_>, _>>()?;
    log::info!("tournament in {} with {}", challenge::ALL[s.challenge].id, names.join(", "));
    run(&entrants, &s, &mut std::io::stdout());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tournament_of_built_in_rovers() {
        let dir = std::env::temp_dir().join(format!("icewire-test-{}-tournament", std::process::id()));
        let s = Settings { challenge: challenge::by_id("boiler-room").unwrap(), timeout: crate::sim::BOT_TIMEOUT, replays: Some(dir.clone()) };
        let entrants: Vec<Design> = ["sprocket", "sleet", "kettle"].iter().map(|n| entrant(n).unwrap()).collect();
        let mut out = Vec::new();
        let table = run(&entrants, &s, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.matches("Match").count(), 6, "{text}");
        assert_eq!(table.iter().map(|t| t.won + t.drawn + t.lost).sum::<u32>(), 12);
        assert_ne!(table[0].name, "Sprocket", "{text}");
        let folder = std::fs::read_dir(&dir).unwrap().flatten().next().unwrap().path();
        let replays: Vec<_> = std::fs::read_dir(&folder).unwrap().flatten().collect();
        assert_eq!(replays.len(), 6);
        let rp = crate::replay::Replay::parse(&std::fs::read_to_string(replays[0].path()).unwrap()).unwrap();
        assert_eq!(rp.names.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn entrants_from_the_command_line() {
        assert_eq!(entrant("Kettle").unwrap().name, "Kettle");
        let d = entrant("python3 my-bots/zigzag.py --fast").unwrap();
        assert_eq!(d.name, "zigzag");
        assert_eq!(d.bot.unwrap().command, "python3 my-bots/zigzag.py --fast");
        assert!(main(&["--challenge".into(), "ring-hall".into(), "kettle".into(), "sprocket".into()]).is_err());
        assert!(main(&["kettle".into()]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn bots_and_wired_rovers_meet() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
        let shellby = Design { name: "Shellby".into(), parts: Vec::new(), wires: Vec::new(), lua: None, bot: Some(BotSpec { command: "sh shellby.sh".into(), dir: Some(dir) }) };
        let (_, how, _, replay) = play(challenge::by_id("boiler-room").unwrap(), shellby, entrant("sprocket").unwrap(), crate::sim::BOT_TIMEOUT);
        assert!(replay.contains("rover 0 Shellby"), "{how}");
    }
}
