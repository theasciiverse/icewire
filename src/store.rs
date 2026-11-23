//! The state folder, `$XDG_STATE_HOME/icewire` (~/.local/state/icewire):
//! rovers as `.rover` text files in `rovers/`, best results in
//! `records.txt`, what was open last in `settings.txt`, and the session
//! log. Tests never get a state folder, so they never touch a real one.

use crate::design::{self, Design};
use std::path::PathBuf;

pub fn state_dir() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join(".local/state")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()).map(PathBuf::from))?;
    let dir = base.join("icewire");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub fn rovers_dir() -> Option<PathBuf> {
    let d = state_dir()?.join("rovers");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// A rover file's stem from its name: "Ice Mule 2" is ice-mule-2.
pub fn stem(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "rover".into() } else { s }
}

/// One file in the garage.
pub struct Stored {
    pub stem: String,
    pub design: Result<Design, String>,
}

/// Every rover in the garage, by name. Puts Homer there on first run.
pub fn garage() -> Vec<Stored> {
    let Some(dir) = rovers_dir() else { return vec![Stored { stem: "homer".into(), design: Design::from_text(design::STARTER) }] };
    let mut list: Vec<Stored> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "rover" || x == "lua"))
                .map(|e| {
                    let p = e.path();
                    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string());
                    let design = if p.extension().is_some_and(|x| x == "lua") { text.map(|t| Design::from_lua(&t, &stem)) } else { text.and_then(|t| Design::from_text(&t)) };
                    Stored { stem, design }
                })
                .collect()
        })
        .unwrap_or_default();
    if list.is_empty() && !dir.join(".started").exists() {
        let d = Design::from_text(design::STARTER).expect("starter rover");
        let _ = std::fs::write(dir.join(".started"), "");
        save(&d, None);
        list.push(Stored { stem: stem(&d.name), design: Ok(d) });
    }
    list.sort_by_key(|s| s.stem.clone());
    list
}

/// A stem no other rover uses, for a new name.
pub fn free_stem(name: &str, keep: Option<&str>) -> String {
    let base = stem(name);
    let Some(dir) = rovers_dir() else { return base };
    let mut s = base.clone();
    let mut n = 2;
    while Some(s.as_str()) != keep && (dir.join(format!("{s}.rover")).exists() || dir.join(format!("{s}.lua")).exists()) {
        s = format!("{base}-{n}");
        n += 1;
    }
    s
}

/// Saves under the rover's own name; `old` is the file it was loaded
/// from, removed when the name changed. Returns the stem saved as.
pub fn save(d: &Design, old: Option<&str>) -> Option<String> {
    let dir = rovers_dir()?;
    let s = free_stem(&d.name, old);
    let ext = if d.lua.is_some() { "lua" } else { "rover" };
    let path = dir.join(format!("{s}.{ext}"));
    let text = d.lua_text().unwrap_or_else(|| d.to_text());
    if let Err(e) = std::fs::write(&path, text) {
        log::error!("saving {}: {e}", path.display());
        return None;
    }
    if let Some(old) = old
        && old != s
    {
        let _ = std::fs::remove_file(dir.join(format!("{old}.{ext}")));
    }
    log::info!("saved rover {:?} as {s}.{ext}", d.name);
    Some(s)
}

/// Where a rover's file is (for the editor).
pub fn path_of(d: &Design, stem: &str) -> Option<PathBuf> {
    Some(rovers_dir()?.join(format!("{stem}.{}", if d.lua.is_some() { "lua" } else { "rover" })))
}

pub fn delete(stem: &str) {
    if let Some(dir) = rovers_dir() {
        let _ = std::fs::remove_file(dir.join(format!("{stem}.rover")));
        let _ = std::fs::remove_file(dir.join(format!("{stem}.lua")));
        log::info!("deleted rover {stem}");
    }
}

/// The best result per challenge: seconds, stars and the rover that did it.
#[derive(Default, Clone)]
pub struct Records {
    pub best: Vec<(String, f64, u8, String)>,
}

impl Records {
    pub fn load() -> Records {
        let mut r = Records::default();
        let Some(text) = state_dir().and_then(|d| std::fs::read_to_string(d.join("records.txt")).ok()) else { return r };
        for line in text.lines() {
            // "id secs sN rover", or from 0.1.0 "id secs rover"
            let mut w = line.splitn(3, ' ');
            if let (Some(id), Some(secs), Some(rest)) = (w.next(), w.next(), w.next())
                && let Ok(secs) = secs.parse::<f64>()
            {
                let (stars, rover) = match rest.split_once(' ') {
                    Some((s, rover)) if s.len() == 2 && s.starts_with('s') && s[1..].parse::<u8>().is_ok() => (s[1..].parse::<u8>().unwrap_or(1), rover),
                    _ => (crate::challenge::by_id(id).map(|c| crate::challenge::ALL[c].stars(secs, 100.0)).unwrap_or(1), rest),
                };
                r.best.push((id.to_string(), secs, stars.clamp(1, 3), rover.to_string()));
            }
        }
        r
    }

    /// Seconds, stars and rover of the best win.
    pub fn get(&self, id: &str) -> Option<(f64, u8, &str)> {
        self.best.iter().find(|b| b.0 == id).map(|b| (b.1, b.2, b.3.as_str()))
    }

    /// Records a win; true when it beats the old best (more stars, or as
    /// many stars sooner).
    pub fn offer(&mut self, id: &str, secs: f64, stars: u8, rover: &str) -> bool {
        match self.best.iter_mut().find(|b| b.0 == id) {
            Some(b) if (b.2, -b.1) >= (stars, -secs) => return false,
            Some(b) => {
                b.1 = secs;
                b.2 = stars;
                b.3 = rover.to_string();
            }
            None => self.best.push((id.to_string(), secs, stars, rover.to_string())),
        }
        if let Some(d) = state_dir() {
            let text: String = self.best.iter().map(|(i, s, n, r)| format!("{i} {s:.3} s{n} {r}\n")).collect();
            let _ = std::fs::write(d.join("records.txt"), text);
        }
        true
    }
}

/// What was open last time.
pub struct Settings {
    pub rover: Option<String>,
    pub challenge: Option<String>,
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings { rover: None, challenge: None };
        if let Some(text) = state_dir().and_then(|d| std::fs::read_to_string(d.join("settings.txt")).ok()) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match k.trim() {
                        "rover" if !v.trim().is_empty() => s.rover = Some(v.trim().to_string()),
                        "challenge" if !v.trim().is_empty() => s.challenge = Some(v.trim().to_string()),
                        _ => {}
                    }
                }
            }
        }
        s
    }

    pub fn save(&self) {
        if let Some(d) = state_dir() {
            let text = format!("rover = {}\nchallenge = {}\n", self.rover.as_deref().unwrap_or(""), self.challenge.as_deref().unwrap_or(""));
            let _ = std::fs::write(d.join("settings.txt"), text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_state_folder_in_tests() {
        assert!(state_dir().is_none());
        assert!(save(&Design::blank("x"), None).is_none());
        assert_eq!(garage().len(), 1);
    }

    #[test]
    fn stems() {
        assert_eq!(stem("Ice Mule 2"), "ice-mule-2");
        assert_eq!(stem("  --"), "rover");
    }

    #[test]
    fn records_keep_the_best() {
        let mut r = Records::default();
        assert!(r.offer("puck", 20.0, 2, "A"));
        assert!(!r.offer("puck", 25.0, 2, "B"));
        assert!(r.offer("puck", 10.0, 2, "C"));
        assert!(r.offer("puck", 30.0, 3, "D"));
        assert!(!r.offer("puck", 5.0, 2, "E"));
        assert_eq!(r.get("puck"), Some((30.0, 3, "D")));
    }
}
