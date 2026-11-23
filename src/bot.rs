//! The open bot port: any program, in any language, can drive a rover.
//! Icewire starts the program, writes each tick's sensor readings to its
//! standard input as plain text lines and reads commands back from its
//! standard output. PROTOCOL.md describes the lines; this is version 1.
//!
//! A bot that is slow gets nothing done that tick; one that stays silent,
//! closes its output or cannot be written to is out for the rest of the
//! run. Every bot runs in a process group of its own (on Unix), and the
//! whole group is killed and reaped when the run ends.

use crate::brain::Drive;
use crate::design::BotSpec;
use crate::parts::{Kind, LOB_MAX, LOB_MIN, Mount};
use crate::sim::Sim;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The protocol's name and version, the first line each way.
pub const HELLO: &str = "icewire-bot 1";
/// How long a bot has to say hello.
const HANDSHAKE: Duration = Duration::from_secs(5);
/// Lines longer than this are cut.
const MAX_LINE: u64 = 1024;
/// Lines a bot may have waiting before it has to wait for Icewire.
const QUEUE: usize = 256;
/// Ticks in a row without an answer before a bot is out.
const MAX_MISSES: u32 = 30;
const CONSOLE: usize = 60;

/// What the reader thread hands over: a line, or the end of output.
enum Msg {
    Line(String),
    Closed,
}

pub struct BotBrain {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Msg>,
    /// Ticks sent and not yet answered.
    pending: u32,
    /// Ticks in a row with no answer in time.
    misses: u32,
    /// The reply being read.
    partial: Vec<String>,
    /// The latest full reply that arrived for the latest tick.
    reply: Option<Vec<String>>,
    pub timeout: Duration,
    /// What the bot wrote to standard error, and Icewire's notes.
    pub console: Arc<Mutex<VecDeque<String>>>,
    /// Why the bot is out, once it is.
    pub dead: Option<String>,
    /// The last thing it said that Icewire did not understand.
    pub confused: Option<String>,
}

fn push(console: &Arc<Mutex<VecDeque<String>>>, line: String) {
    if let Ok(mut c) = console.lock() {
        c.push_back(line);
        while c.len() > CONSOLE {
            c.pop_front();
        }
    }
}

/// Reads lines (each cut to MAX_LINE) until the stream ends.
fn reader<R: Read + Send + 'static>(stream: R, tx: SyncSender<Msg>) {
    std::thread::spawn(move || {
        let mut r = BufReader::new(stream);
        loop {
            let mut buf = Vec::new();
            let mut limited = (&mut r).take(MAX_LINE);
            match limited.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if buf.last() != Some(&b'\n') {
                        // an overlong line: skip the rest of it
                        let mut rest = Vec::new();
                        if r.read_until(b'\n', &mut rest).is_err() {
                            break;
                        }
                    }
                    let line = String::from_utf8_lossy(&buf).trim_end().to_string();
                    if tx.send(Msg::Line(line)).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = tx.send(Msg::Closed);
    });
}

/// Splits a command line into words, with "double quotes" for spaces.
pub fn words(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in cmd.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

impl BotBrain {
    /// Starts the bot and waits for it to say hello.
    pub fn start(spec: &BotSpec, name: &str, challenge: &str, timeout: Duration) -> Result<BotBrain, String> {
        let w = words(&spec.command);
        let Some((prog, args)) = w.split_first() else { return Err("the bot has an empty command".into()) };
        let mut cmd = Command::new(prog);
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(d) = &spec.dir {
            cmd.current_dir(d);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn().map_err(|e| format!("cannot start {prog}: {e}"))?;
        log::info!("bot {name} started as pid {}: {}", child.id(), spec.command);
        let (tx, rx) = sync_channel(QUEUE);
        reader(child.stdout.take().expect("piped stdout"), tx);
        let console = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(err) = child.stderr.take() {
            let c = console.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err.take(1 << 20)).lines() {
                    match line {
                        Ok(l) => push(&c, l.chars().take(MAX_LINE as usize).collect()),
                        Err(_) => break,
                    }
                }
            });
        }
        let stdin = child.stdin.take();
        let mut b = BotBrain { child, stdin, rx, pending: 0, misses: 0, partial: Vec::new(), reply: None, timeout, console, dead: None, confused: None };
        b.write(&format!("{HELLO}\nrover {}\nchallenge {challenge}\nend\n", name.replace(' ', "_")));
        let deadline = Instant::now() + HANDSHAKE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match b.rx.recv_timeout(left) {
                Ok(Msg::Line(l)) if l.trim() == HELLO => break,
                Ok(Msg::Line(l)) if l.trim().starts_with("icewire-bot ") => return Err(format!("the bot speaks {}, Icewire speaks {HELLO}", l.trim())),
                Ok(Msg::Line(_)) => continue,
                Ok(Msg::Closed) | Err(RecvTimeoutError::Disconnected) => return Err(b.last_words("the bot ended before saying hello")),
                Err(RecvTimeoutError::Timeout) => return Err(format!("the bot did not answer \"{HELLO}\" within {} seconds", HANDSHAKE.as_secs())),
            }
        }
        if let Some(d) = &b.dead {
            return Err(d.clone());
        }
        Ok(b)
    }

    /// The bot's last line of standard error, if any, for a clearer error.
    fn last_words(&self, what: &str) -> String {
        std::thread::sleep(Duration::from_millis(30));
        match self.console.lock().ok().and_then(|c| c.back().cloned()) {
            Some(l) => format!("{what}: {l}"),
            None => what.to_string(),
        }
    }

    #[cfg(test)]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    fn write(&mut self, text: &str) {
        if self.dead.is_some() {
            return;
        }
        let ok = match self.stdin.as_mut() {
            Some(s) => s.write_all(text.as_bytes()).and_then(|_| s.flush()).is_ok(),
            None => false,
        };
        if !ok {
            self.kill_with("the bot stopped listening");
        }
    }

    fn kill_with(&mut self, why: &str) {
        if self.dead.is_none() {
            log::info!("bot out: {why}");
            push(&self.console, format!("[icewire] {why}"));
            self.dead = Some(why.to_string());
            self.stdin = None;
            kill(&mut self.child);
        }
    }

    /// Takes whatever lines have arrived; with `until`, waits for the
    /// reply to the latest tick until then.
    fn gather(&mut self, until: Option<Instant>) {
        loop {
            let msg = match until {
                None => match self.rx.try_recv() {
                    Ok(m) => m,
                    Err(TryRecvError::Empty) => return,
                    Err(TryRecvError::Disconnected) => Msg::Closed,
                },
                Some(t) => {
                    if self.pending == 0 {
                        return;
                    }
                    match self.rx.recv_timeout(t.saturating_duration_since(Instant::now())) {
                        Ok(m) => m,
                        Err(RecvTimeoutError::Timeout) => return,
                        Err(RecvTimeoutError::Disconnected) => Msg::Closed,
                    }
                }
            };
            match msg {
                Msg::Closed => {
                    let why = self.last_words("the bot's program ended");
                    self.kill_with(&why);
                    return;
                }
                Msg::Line(l) => {
                    if l.trim() == "end" {
                        let reply = std::mem::take(&mut self.partial);
                        if self.pending > 0 {
                            self.pending -= 1;
                        }
                        if self.pending == 0 {
                            self.reply = Some(reply);
                        }
                    } else {
                        self.partial.push(l);
                    }
                }
            }
        }
    }

    /// Phase one of a tick: sends this tick's readings. Answers still owed
    /// for earlier ticks are skipped when they come.
    pub fn send(&mut self, sim: &mut Sim, i: usize) {
        if self.dead.is_some() {
            return;
        }
        self.gather(None);
        let block = readings(sim, i);
        self.reply = None;
        self.pending += 1;
        self.write(&block);
    }

    /// Phase two: waits (until `deadline`) for the answer and turns it into
    /// drive commands. No answer in time means a quiet tick.
    pub fn collect(&mut self, deadline: Instant) -> Drive {
        if self.dead.is_some() {
            return Drive::default();
        }
        self.gather(Some(deadline));
        match self.reply.take() {
            Some(lines) => {
                self.misses = 0;
                let (d, bad) = commands(&lines);
                if let Some(b) = bad
                    && self.confused.as_ref() != Some(&b)
                {
                    push(&self.console, format!("[icewire] did not understand: {b}"));
                    self.confused = Some(b);
                }
                d
            }
            None => {
                if self.dead.is_none() {
                    self.misses += 1;
                    if self.misses >= MAX_MISSES {
                        self.kill_with(&format!("no answer for {MAX_MISSES} ticks in a row"));
                    }
                }
                Drive::default()
            }
        }
    }
}

/// Kills the bot's whole process group and waits for it, so nothing is
/// left running and nothing is left a zombie.
fn kill(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        // SAFETY: kill(2) on our own child's process group; no memory is touched.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

impl Drop for BotBrain {
    fn drop(&mut self) {
        self.stdin = None;
        kill(&mut self.child);
    }
}

fn n(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == r.trunc() { format!("{}", r as i64) } else { format!("{r}") }
}

/// One tick's readings for rover `i`, in protocol lines.
pub fn readings(sim: &mut Sim, i: usize) -> String {
    let mut s = format!("tick {}\ntime {}\n", sim.tick, n(sim.secs()));
    let sides = Mount::SIDES;
    let mut line = |label: &str, kind: Kind, value: f64| {
        let vals: Vec<String> = sides.iter().map(|m| format!("{} {}", m.name(), n(sim.read(i, kind, *m, 0, value)))).collect();
        format!("{label} {}\n", vals.join(" "))
    };
    s.push_str(&line("ranger", Kind::Ranger, 0.0));
    s.push_str(&line("bumper", Kind::Bumper, 0.0));
    let r = |sim: &mut Sim, kind: Kind, mode: usize| n(sim.read(i, kind, Mount::Nose, mode, 0.0));
    s.push_str(&format!("compass heading {} bearing {} distance {}\n", r(sim, Kind::Compass, 0), r(sim, Kind::Compass, 1), r(sim, Kind::Compass, 2)));
    s.push_str(&format!("radar bearing {} distance {}\n", r(sim, Kind::Radar, 0), r(sim, Kind::Radar, 1)));
    s.push_str(&format!("charge {}\n", r(sim, Kind::Gauge, 0)));
    s.push_str(&format!("loaded pellet {} lobber {}\n", r(sim, Kind::PelletGun, 0), r(sim, Kind::Lobber, 0)));
    s.push_str(&format!("speed {}\nend\n", n(sim.rovers[i].rover.speed())));
    s
}

/// A reply's lines as drive commands, and the first line not understood.
pub fn commands(lines: &[String]) -> (Drive, Option<String>) {
    let mut d = Drive::default();
    let mut bad = None;
    let num = |w: Option<&str>| w.and_then(|x| x.parse::<f64>().ok()).filter(|v| v.is_finite());
    for l in lines {
        let mut w = l.split_whitespace();
        let ok = match w.next() {
            None => true,
            Some("tracks") => match (num(w.next()), num(w.next())) {
                (Some(a), Some(b)) => {
                    d.left = a.clamp(-1.0, 1.0);
                    d.right = b.clamp(-1.0, 1.0);
                    true
                }
                _ => false,
            },
            Some("gyro") => match num(w.next()) {
                Some(g) => {
                    d.gyro = g.clamp(-1.0, 1.0);
                    true
                }
                None => false,
            },
            Some("fire") => {
                d.fire = true;
                true
            }
            Some("lob") => match num(w.next()) {
                Some(r) => {
                    d.lob = Some(r.clamp(LOB_MIN, LOB_MAX));
                    true
                }
                None => false,
            },
            Some("say") | Some("#") => true,
            Some(_) => false,
        };
        if !ok && bad.is_none() {
            bad = Some(l.chars().take(80).collect());
        }
    }
    (d, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_parse() {
        let lines: Vec<String> = ["tracks 0.5 2", "gyro -0.25", "fire", "lob 20", "say hello", "dance"].iter().map(|s| s.to_string()).collect();
        let (d, bad) = commands(&lines);
        assert_eq!((d.left, d.right, d.gyro, d.fire, d.lob), (0.5, 1.0, -0.25, true, Some(LOB_MAX)));
        assert_eq!(bad.as_deref(), Some("dance"));
        let (d, bad) = commands(&["tracks nan 1".to_string()]);
        assert_eq!(d.left, 0.0);
        assert!(bad.is_some());
    }

    #[test]
    fn command_lines_split() {
        assert_eq!(words(r#"python3 "my bots/dodger.py"  --fast"#), ["python3", "my bots/dodger.py", "--fast"]);
        assert!(words("   ").is_empty());
    }

    #[cfg(unix)]
    mod processes {
        use super::super::*;
        use crate::challenge;
        use crate::design::{self, BotSpec, Design};
        use crate::sim::{Outcome, Sim};
        use std::path::PathBuf;

        fn examples() -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")
        }

        fn bot(name: &str, command: &str, dir: PathBuf) -> Design {
            Design { name: name.into(), parts: Vec::new(), wires: Vec::new(), lua: None, bot: Some(BotSpec { command: command.into(), dir: Some(dir) }) }
        }

        /// A throwaway folder for a test's scripts (never the state folder).
        fn scratch(tag: &str) -> PathBuf {
            let d = std::env::temp_dir().join(format!("icewire-test-{}-{tag}", std::process::id()));
            std::fs::create_dir_all(&d).unwrap();
            d
        }

        fn script(dir: &PathBuf, body: &str) -> String {
            let p = dir.join("bot.sh");
            std::fs::write(&p, format!("read hello\nwhile read l; do [ \"$l\" = end ] && break; done\n{body}")).unwrap();
            format!("sh {}", p.display())
        }

        /// Gone, or at least not left running: on Linux, no /proc entry or a
        /// finished one reaped by its parent.
        fn gone(pid: u32) -> bool {
            for _ in 0..100 {
                let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"));
                match stat {
                    Err(_) => return true,
                    Ok(s) if !cfg!(target_os = "linux") || s.split_whitespace().nth(2) == Some("Z") && pid_parent_is_init(&s) => return true,
                    _ => std::thread::sleep(Duration::from_millis(20)),
                }
            }
            false
        }

        fn pid_parent_is_init(stat: &str) -> bool {
            // an orphaned zombie is init's (or a subreaper's) to reap, not ours
            stat.split_whitespace().nth(3).and_then(|p| p.parse::<u32>().ok()) != Some(std::process::id())
        }

        fn run_ticks(s: &mut Sim, n: u32) {
            for _ in 0..n {
                s.step();
            }
        }

        #[test]
        fn shellby_finishes_ring_hall() {
            let d = bot("Shellby", "sh shellby.sh", examples());
            let mut s = Sim::new(challenge::by_id("ring-hall").unwrap(), d);
            s.run(60 * 120);
            assert!(matches!(s.outcome, Outcome::Won(_)), "{:?} {:?}", s.outcome, s.events);
            assert!(s.rovers[0].bot.as_ref().unwrap().dead.is_none());
        }

        #[test]
        fn dodger_beats_sprocket_and_kettle() {
            if std::process::Command::new("python3").arg("--version").output().is_err() {
                eprintln!("no python3 here; skipping");
                return;
            }
            for ch in ["boiler-room", "laundry"] {
                let d = bot("Dodger", "python3 dodger.py", examples());
                let mut s = Sim::new(challenge::by_id(ch).unwrap(), d);
                s.run(60 * 120);
                assert!(matches!(s.outcome, Outcome::Won(_)), "{ch}: {:?} {:?}", s.outcome, s.events.last());
            }
        }

        #[test]
        fn a_silent_bot_is_out_after_its_misses_and_is_reaped() {
            let dir = scratch("silent");
            let cmd = script(&dir, "echo icewire-bot 1\nsleep 30 &\necho $! > child.pid\nwait\n");
            let d = bot("Mute", &cmd, dir.clone());
            let mut s = Sim::with(challenge::by_id("boiler-room").unwrap(), vec![d, Design::from_text(design::SPROCKET).unwrap()], Duration::from_millis(5));
            let pid = s.rovers[0].bot.as_ref().unwrap().pid();
            run_ticks(&mut s, MAX_MISSES + 5);
            let b = s.rovers[0].bot.as_ref().unwrap();
            assert!(b.dead.as_deref().unwrap_or("").contains("no answer"), "{:?}", b.dead);
            assert_eq!(s.outcome, Outcome::Running, "the run goes on without it");
            assert!(gone(pid), "the bot is killed as soon as it is out");
            let grandchild: u32 = std::fs::read_to_string(dir.join("child.pid")).unwrap().trim().parse().unwrap();
            assert!(gone(grandchild), "and so is anything it started");
            drop(s);
            let _ = std::fs::remove_dir_all(dir);
        }

        #[test]
        fn a_bot_that_quits_is_out_at_once() {
            let dir = scratch("quits");
            let cmd = script(&dir, "echo icewire-bot 1\nread l\necho 'giving up' >&2\nexit 3\n");
            let mut s = Sim::new(challenge::by_id("ring-hall").unwrap(), bot("Quitter", &cmd, dir.clone()));
            run_ticks(&mut s, 5);
            let why = s.rovers[0].bot.as_ref().unwrap().dead.clone().unwrap_or_default();
            assert!(why.contains("ended"), "{why}");
            assert!(s.events.iter().any(|e| e.1.contains("bot is out")));
            let _ = std::fs::remove_dir_all(dir);
        }

        #[test]
        fn a_flood_without_end_cannot_jam_icewire() {
            let dir = scratch("flood");
            let cmd = script(&dir, "echo icewire-bot 1\nwhile :; do echo 'tracks 1 1'; done\n");
            let mut s = Sim::with(0, vec![bot("Flood", &cmd, dir.clone())], Duration::from_millis(5));
            let pid = s.rovers[0].bot.as_ref().unwrap().pid();
            run_ticks(&mut s, MAX_MISSES + 2);
            assert!(s.rovers[0].bot.as_ref().unwrap().dead.is_some());
            assert_eq!(s.rovers[0].rover.speed(), 0.0, "nothing it said counted");
            drop(s);
            assert!(gone(pid));
            let _ = std::fs::remove_dir_all(dir);
        }

        #[test]
        fn wrong_versions_and_nonsense() {
            let dir = scratch("version");
            let cmd = script(&dir, "echo icewire-bot 9\nsleep 5\n");
            let s = Sim::new(0, bot("Future", &cmd, dir.clone()));
            assert!(s.rovers[0].bot.is_none());
            assert!(s.events.iter().any(|e| e.1.contains("icewire-bot 9")), "{:?}", s.events);
            let cmd = script(&dir, "echo icewire-bot 1\nwhile read l; do [ \"$l\" = end ] && printf 'dance\\ntracks 1 1\\nend\\n'; done\n");
            let mut s = Sim::new(0, bot("Babbler", &cmd, dir.clone()));
            run_ticks(&mut s, 30);
            let b = s.rovers[0].bot.as_ref().unwrap();
            assert_eq!(b.confused.as_deref(), Some("dance"));
            assert!(b.dead.is_none());
            assert!(s.rovers[0].rover.speed() > 0.1, "the lines it got right still count");
            let missing = Sim::new(0, bot("Ghost", "no-such-program-here", dir.clone()));
            assert!(missing.rovers[0].bot.is_none());
            assert!(missing.events.iter().any(|e| e.1.contains("cannot start")));
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
