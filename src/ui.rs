//! Everything drawn: title, garage, challenges, parts library, help, the
//! wiring bench and the arena.

use crate::app::{App, Mode, Popup, RunResult, SPEEDS, Screen, TITLE_MENU};
use crate::campaign;
use crate::challenge::{self, Goal};
use crate::design::{BENCH_H, BENCH_W, Part};
use crate::geom;
use crate::parts::{self, BOX_H, BOX_W, Category, Kind};
use crate::route;
use crate::sim::{RADIUS, Sim};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

const ICE: Color = Color::Rgb(110, 170, 210);
const TEXT: Color = Color::Rgb(205, 222, 235);
const DIM: Color = Color::Rgb(95, 115, 135);
const FAINT: Color = Color::Rgb(45, 58, 72);
const AMBER: Color = Color::Rgb(255, 190, 80);
const GOOD: Color = Color::Rgb(120, 230, 150);
const BAD: Color = Color::Rgb(255, 110, 100);
const WALL: Color = Color::Rgb(38, 58, 86);
const WALL_EDGE: Color = Color::Rgb(70, 100, 140);
const SLUSH: Color = Color::Rgb(12, 22, 36);
const ME: Color = Color::Rgb(120, 230, 255);
const THEM: Color = Color::Rgb(255, 140, 90);
const WIRE_IDLE: Color = Color::Rgb(140, 155, 175);
const WIRE_OFF: Color = Color::Rgb(60, 78, 98);
const WIRE_ON: Color = Color::Rgb(90, 235, 255);
const WIRE_NEG: Color = Color::Rgb(255, 175, 70);

fn cat_color(c: Category) -> Color {
    match c {
        Category::Sensor => Color::Rgb(90, 205, 215),
        Category::Logic => Color::Rgb(185, 160, 255),
        Category::Drive => Color::Rgb(255, 160, 85),
        Category::Weapon => Color::Rgb(255, 120, 150),
    }
}

fn signal_color(v: f64) -> Color {
    if v > 0.0 {
        WIRE_ON
    } else if v < 0.0 {
        WIRE_NEG
    } else {
        WIRE_OFF
    }
}

fn stars(n: u8) -> String {
    (0..3).map(|i| if i < n { '★' } else { '☆' }).collect()
}

/// Puts a string into the buffer, clipped to `area`.
fn put(buf: &mut Buffer, area: Rect, x: i32, y: i32, s: &str, style: Style) {
    if y < area.y as i32 || y >= (area.y + area.height) as i32 {
        return;
    }
    for (i, ch) in s.chars().enumerate() {
        let cx = x + i as i32;
        if cx >= area.x as i32 && cx < (area.x + area.width) as i32 {
            buf[(cx as u16, y as u16)].set_char(ch).set_style(style);
        }
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.buffer_mut().set_style(area, Style::default().fg(TEXT));
    if area.width < 60 || area.height < 20 {
        let p = Paragraph::new(format!("Icewire needs a terminal of at least 60x20 (this one is {}x{}).", area.width, area.height)).wrap(Wrap { trim: true });
        f.render_widget(p, area);
        return;
    }
    // while the lesson plays, the game draws above its caption
    let (area, caption) = if app.lesson.is_some() {
        let h = if area.height >= 32 { 9 } else { 8 };
        let [top, cap] = Layout::vertical([Constraint::Min(10), Constraint::Length(h)]).areas(area);
        (top, Some(cap))
    } else {
        (area, None)
    };
    match app.screen {
        Screen::Title => title(f, app, area),
        Screen::League => league(f, app, area),
        Screen::Story => story(f, app, area),
        Screen::Garage => garage(f, app, area),
        Screen::Challenges => challenges(f, app, area),
        Screen::Library => library(f, app, area),
        Screen::Help => help(f, app, area),
        Screen::Bench => {
            if app.running() && app.arena_view {
                arena(f, app, area);
            } else if app.design.bot.is_some() {
                bot_view(f, app, area);
            } else if app.design.lua.is_some() {
                lua_view(f, app, area);
            } else {
                bench(f, app, area);
            }
            if let Some(r) = app.result.clone() {
                result_card(f, app, &r, area);
            }
        }
    }
    if let Some(p) = app.popup.clone() {
        popup(f, app, &p, area);
    }
    if let Some(cap) = caption {
        lesson_caption(f, app, cap);
    }
}

/// The lesson's caption: which step, what is happening and why, the keys
/// just pressed, and how to pause, step and leave.
fn lesson_caption(f: &mut Frame, app: &App, area: Rect) {
    let Some(l) = &app.lesson else { return };
    let beat = &crate::lesson::BEATS[l.beat.min(crate::lesson::BEATS.len() - 1)];
    let n = crate::lesson::BEATS.len();
    let finished = l.done;
    let mut title = vec![
        Span::styled(" Watch and learn ", Style::default().fg(Color::Black).bg(ICE).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}/{n} ", l.beat + 1), Style::default().fg(DIM)),
        Span::styled(format!("{} ", beat.title), Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
    ];
    if l.paused {
        title.push(Span::styled(" paused ", Style::default().fg(Color::Black).bg(AMBER)));
    }
    let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(ICE)).title(Line::from(title));
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let [text, keys, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(beat.say).style(Style::default().fg(TEXT)).wrap(Wrap { trim: true }), text);
    // the keys of this step, newest last, cut from the left to fit
    let shown = crate::lesson::keys_shown(&l.keys);
    let room = keys.width.saturating_sub(7) as usize;
    let chars: Vec<char> = shown.chars().collect();
    let shown: String = if chars.len() > room { format!("\u{2026}{}", chars[chars.len() - room + 1..].iter().collect::<String>()) } else { shown };
    let mut ks = vec![Span::styled(" keys ", Style::default().fg(DIM))];
    if !shown.is_empty() {
        ks.push(Span::styled(shown, Style::default().fg(WIRE_ON).add_modifier(Modifier::BOLD)));
    }
    f.render_widget(Paragraph::new(Line::from(ks)), keys);
    let help: &[(&str, &str)] = if finished {
        &[("Enter", "keep First Light"), ("Esc", "back to the title"), ("\u{2190}", "a step back")]
    } else {
        &[("Space", "pause"), ("\u{2192}", "next step"), ("\u{2190}", "a step back"), ("Esc", "leave")]
    };
    let mut spans = Vec::new();
    for (k, what) in help {
        spans.push(Span::styled(format!(" {k}"), Style::default().fg(AMBER)));
        spans.push(Span::styled(format!(" {what} "), Style::default().fg(DIM)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), foot);
}

// ------------------------------------------------------------- Braille

/// A Braille dot canvas over an area of cells.
struct Dots {
    area: Rect,
    cells: Vec<(u8, Color)>,
}

impl Dots {
    fn new(area: Rect) -> Dots {
        Dots { area, cells: vec![(0, Color::Reset); area.width as usize * area.height as usize] }
    }

    fn set(&mut self, x: i32, y: i32, c: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let (cx, cy) = (x / 2, y / 4);
        if cx >= self.area.width as i32 || cy >= self.area.height as i32 {
            return;
        }
        const BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
        let cell = &mut self.cells[(cy * self.area.width as i32 + cx) as usize];
        cell.0 |= BITS[(x % 2) as usize][(y % 4) as usize];
        cell.1 = c;
    }

    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, c: Color, every: usize) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as usize).max(1);
        for i in (0..=n).step_by(every.max(1)) {
            let t = i as f64 / n as f64;
            self.set((x0 + (x1 - x0) * t).round() as i32, (y0 + (y1 - y0) * t).round() as i32, c);
        }
    }

    fn render(&self, buf: &mut Buffer) {
        for (i, (bits, c)) in self.cells.iter().enumerate() {
            if *bits == 0 {
                continue;
            }
            let x = self.area.x + (i % self.area.width as usize) as u16;
            let y = self.area.y + (i / self.area.width as usize) as u16;
            let ch = char::from_u32(0x2800 + *bits as u32).unwrap_or(' ');
            buf[(x, y)].set_char(ch).set_fg(*c);
        }
    }
}

// --------------------------------------------------------------- title

const LOGO: [&str; 5] = [
    "█ ▄▀▀▀ █▀▀▀ █   █ █ █▀▀▄ █▀▀▀",
    "█ █    █▄▄  █ ▄ █ █ █▄▄▀ █▄▄ ",
    "█ █    █    █ █ █ █ █  █ █   ",
    "█ ▀▄▄▄ █▄▄▄ ▀▄▀▄▀ █ █  █ █▄▄▄",
    "",
];

fn jupiter(buf: &mut Buffer, area: Rect) {
    // a gas giant low over the ice, banded, with its storm
    let mut d = Dots::new(area);
    let (w, h) = (area.width as i32 * 2, area.height as i32 * 4);
    let horizon = (h as f64 * 0.82) as i32;
    let r = (h as f64 * 0.42).min(w as f64 * 0.2);
    let (cx, cy) = (w as f64 * 0.75, horizon as f64 - r * 0.55);
    let bands = [Color::Rgb(225, 195, 150), Color::Rgb(185, 130, 90), Color::Rgb(235, 215, 180), Color::Rgb(165, 115, 80), Color::Rgb(215, 180, 135)];
    for y in 0..horizon {
        for x in 0..w {
            let (dx, dy) = (x as f64 - cx, (y as f64 - cy) * 1.0);
            if dx * dx + dy * dy <= r * r {
                let shade = (dx * 0.6 + dy) / r; // lit from the upper left
                let band = (((y as f64 - cy) / r * 7.0 + 7.0) as usize) % bands.len();
                let spot = ((x as f64 - (cx + r * 0.3)) / 6.0).powi(2) + ((y as f64 - (cy - r * 0.35)) / 3.0).powi(2) < 1.0;
                let c = if spot { Color::Rgb(200, 90, 60) } else { bands[band] };
                // stipple the dark side
                if shade < 0.35 || (shade < 0.8 && (x + y) % 2 == 0) || (x % 2 == 0 && y % 4 == 0) {
                    d.set(x, y, c);
                }
            }
        }
    }
    // stars, fixed
    let mut seed: u32 = 2026;
    for _ in 0..60 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let x = w / 2 + (seed >> 8) as i32 % (w / 2);
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let y = (seed >> 8) as i32 % horizon;
        let (dx, dy) = (x as f64 - cx, y as f64 - cy);
        if dx * dx + dy * dy > (r + 2.0) * (r + 2.0) {
            d.set(x, y, DIM);
        }
    }
    // the ice: a horizon and a few cracks
    for x in 0..w {
        d.set(x, horizon, ICE);
        if x % 7 == 0 {
            d.set(x, horizon + 4 + (x / 7 % 3), FAINT);
        }
    }
    d.render(buf);
}

fn title(f: &mut Frame, app: &App, area: Rect) {
    jupiter(f.buffer_mut(), area);
    let buf = f.buffer_mut();
    let x = area.x as i32 + 4;
    let y0 = area.y as i32 + 2;
    for (i, l) in LOGO.iter().enumerate() {
        put(buf, area, x, y0 + i as i32, l, Style::default().fg(ICE).add_modifier(Modifier::BOLD));
    }
    put(buf, area, x, y0 + 5, "Robot rovers, wired by hand, on the ice of Europa", Style::default().fg(DIM));
    let my = y0 + 7;
    let gap = if area.height >= 30 { 2 } else { 1 };
    for (i, item) in TITLE_MENU.iter().enumerate() {
        let sel = i == app.sel;
        let style = if sel { Style::default().fg(Color::Black).bg(AMBER) } else { Style::default().fg(TEXT) };
        put(buf, area, x, my + i as i32 * gap, &format!(" {}{item} ", if sel { "▸ " } else { "  " }), style);
    }
    let ch = &challenge::ALL[app.ch];
    let note = format!("Rover: {}   Challenge: {}", app.design.name, ch.name);
    put(buf, area, x, area.y as i32 + area.height as i32 - 2, &note, Style::default().fg(DIM));
    put(buf, area, x, area.y as i32 + area.height as i32 - 1, "↑↓ choose   Enter go   Q quit", Style::default().fg(FAINT));
}

// ------------------------------------------------------------- league

fn league(f: &mut Frame, app: &App, area: Rect) {
    let [main, foot] = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).areas(area);
    let [list, side] = Layout::horizontal([Constraint::Length(36), Constraint::Min(20)]).areas(main);
    let mut lines = vec![Line::from(Span::styled("A first season on Conamara Station", Style::default().fg(DIM))), Line::from("")];
    for (i, c) in campaign::CHAPTERS.iter().enumerate() {
        let sel = i == app.league_sel;
        let (mark, fg) = if i < app.won {
            ("✓", GOOD)
        } else if i == app.won {
            ("▸", AMBER)
        } else {
            ("·", FAINT)
        };
        let style = if sel { Style::default().bg(Color::Rgb(40, 60, 85)).add_modifier(Modifier::BOLD) } else { Style::default() };
        let st = challenge::by_id(c.challenge).and_then(|k| app.records.get(challenge::ALL[k].id)).map(|b| stars(b.1)).unwrap_or_default();
        let name = if i <= app.won { c.title } else { "?" };
        lines.push(Line::from(vec![Span::styled(format!(" {mark} "), style.fg(fg)), Span::styled(format!("Night {}  {:<17}", i + 1, name), style.fg(if i <= app.won { TEXT } else { FAINT })), Span::styled(st, style.fg(AMBER))]));
    }
    if app.won >= campaign::CHAPTERS.len() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(" The notebook is yours. Same time tomorrow.", Style::default().fg(GOOD))));
    }
    f.render_widget(Paragraph::new(lines).block(frame_block("The league")), list);
    let i = app.league_sel;
    let c = &campaign::CHAPTERS[i];
    let mut info = Vec::new();
    if i <= app.won {
        let ch = challenge::by_id(c.challenge).map(|k| &challenge::ALL[k]);
        info.push(Line::from(Span::styled(format!("Night {}: {}", i + 1, c.title), Style::default().fg(ICE).add_modifier(Modifier::BOLD))));
        if let Some(ch) = ch {
            info.push(Line::from(Span::styled(format!("{}: {}", ch.name, ch.tagline), Style::default().fg(AMBER))));
            info.push(Line::from(""));
            info.push(Line::from(ch.brief));
        }
    } else {
        info.push(Line::from(Span::styled("Still ahead", Style::default().fg(DIM))));
        info.push(Line::from(""));
        info.push(Line::from("Win the nights before this one and Ines will tell you where to be."));
    }
    f.render_widget(Paragraph::new(info).wrap(Wrap { trim: true }).block(frame_block("Tonight")), side);
    keys_line(f, foot, &[("↑↓", "choose"), ("Enter", "play the night"), ("Esc", "back")], app.msg.as_ref().map(|m| m.0.as_str()));
}

fn story(f: &mut Frame, app: &App, area: Rect) {
    let Some(i) = app.chapter else { return };
    let c = &campaign::CHAPTERS[i];
    let [main, foot] = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).areas(area);
    let art_w = if main.width >= 96 { main.width / 3 } else { 0 };
    let [text, art] = Layout::horizontal([Constraint::Min(40), Constraint::Length(art_w)]).areas(main);
    if art_w > 0 {
        jupiter(f.buffer_mut(), art);
    }
    let title = if app.story_after { format!("Night {}: {}, after", i + 1, c.title) } else { format!("Night {}: {}", i + 1, c.title) };
    let body = if app.story_after { c.after } else { c.before };
    let inner = text.inner(ratatui::layout::Margin::new(2, 1));
    f.render_widget(frame_block(&title), text);
    f.render_widget(Paragraph::new(body).style(Style::default().fg(TEXT)).wrap(Wrap { trim: false }), inner);
    let keys: &[(&str, &str)] = if app.story_after { &[("Enter", "back to the league")] } else { &[("Enter", "to the bench"), ("Esc", "not tonight")] };
    keys_line(f, foot, keys, None);
}

// -------------------------------------------------------------- frames

fn frame_block(title: &str) -> Block<'_> {
    Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(WALL_EDGE)).title(Span::styled(format!(" {title} "), Style::default().fg(ICE).add_modifier(Modifier::BOLD)))
}

fn keys_line(f: &mut Frame, area: Rect, keys: &[(&str, &str)], msg: Option<&str>) {
    let line = if let Some(m) = msg {
        Line::from(Span::styled(format!(" {m}"), Style::default().fg(AMBER)))
    } else {
        let mut spans = Vec::new();
        for (k, what) in keys {
            spans.push(Span::styled(format!(" {k}"), Style::default().fg(AMBER)));
            spans.push(Span::styled(format!(" {what} "), Style::default().fg(DIM)));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
}

fn garage(f: &mut Frame, app: &App, area: Rect) {
    let [main, foot] = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).areas(area);
    let [list, side] = Layout::horizontal([Constraint::Length(34), Constraint::Min(20)]).areas(main);
    let mut lines = Vec::new();
    for (i, g) in app.garage.iter().enumerate() {
        let sel = i == app.sel;
        let (name, style) = match &g.design {
            Ok(d) => (d.name.clone(), Style::default().fg(TEXT)),
            Err(_) => (format!("{}.rover (unreadable)", g.stem), Style::default().fg(BAD)),
        };
        let mark = if Some(&g.stem) == app.stem.as_ref() { "•" } else { " " };
        let style = if sel { style.bg(Color::Rgb(40, 60, 85)).add_modifier(Modifier::BOLD) } else { style };
        lines.push(Line::from(Span::styled(format!(" {mark} {name:<28}"), style)));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(" The garage is empty. N builds a new rover.", Style::default().fg(DIM))));
    }
    f.render_widget(Paragraph::new(lines).block(frame_block("Garage")), list);
    let mut info = Vec::new();
    if let Some(g) = app.garage.get(app.sel) {
        match &g.design {
            Ok(d) if d.bot.is_some() => {
                let b = d.bot.as_ref().expect("bot");
                info.push(Line::from(Span::styled(d.name.clone(), Style::default().fg(ICE).add_modifier(Modifier::BOLD))));
                info.push(Line::from("A bot: a program of your own, on the line protocol"));
                info.push(Line::from(""));
                info.push(Line::from(vec![Span::styled("runs  ", Style::default().fg(DIM)), Span::raw(b.command.clone())]));
                info.push(Line::from(""));
                info.push(Line::from(Span::styled(format!("File: rovers/{}.bot in the state folder.", g.stem), Style::default().fg(DIM))));
            }
            Ok(d) if d.lua.is_some() => {
                let script = d.lua.as_deref().unwrap_or("");
                info.push(Line::from(Span::styled(d.name.clone(), Style::default().fg(ICE).add_modifier(Modifier::BOLD))));
                info.push(Line::from(format!("A rover in Lua, {} lines", script.lines().count())));
                info.push(Line::from(""));
                for l in script.lines().skip(1).filter(|l| l.starts_with("--")).take(6) {
                    info.push(Line::from(Span::styled(l.trim_start_matches("--").trim().to_string(), Style::default().fg(DIM))));
                }
                info.push(Line::from(""));
                info.push(Line::from(Span::styled(format!("File: rovers/{}.lua in the state folder; e on the bench opens it in your editor.", g.stem), Style::default().fg(DIM))));
            }
            Ok(d) => {
                info.push(Line::from(Span::styled(d.name.clone(), Style::default().fg(ICE).add_modifier(Modifier::BOLD))));
                info.push(Line::from(format!("{} sensors, {} logic parts, {} wires", d.count(Category::Sensor), d.count(Category::Logic), d.wires.len())));
                info.push(Line::from(""));
                for c in [Category::Sensor, Category::Logic, Category::Drive] {
                    let mut names: Vec<String> = d.parts.iter().filter(|p| p.kind.category() == c).map(|p| p.kind.name().to_string()).collect();
                    names.sort();
                    if !names.is_empty() {
                        info.push(Line::from(vec![Span::styled(format!("{:<8}", c.name()), Style::default().fg(cat_color(c))), Span::raw(names.join(", "))]));
                    }
                }
                info.push(Line::from(""));
                info.push(Line::from(Span::styled(format!("File: rovers/{}.rover in the state folder; plain text, edit it freely.", g.stem), Style::default().fg(DIM))));
            }
            Err(e) => {
                info.push(Line::from(Span::styled(format!("rovers/{}.rover does not read as a rover:", g.stem), Style::default().fg(BAD))));
                info.push(Line::from(e.clone()));
            }
        }
    }
    f.render_widget(Paragraph::new(info).wrap(Wrap { trim: true }).block(frame_block("Rover")), side);
    keys_line(f, foot, &[("Enter", "to the bench"), ("N", "new"), ("L", "new in Lua"), ("B", "new bot"), ("C", "copy"), ("D", "delete"), ("Esc", "back")], app.msg.as_ref().map(|m| m.0.as_str()));
}

fn challenges(f: &mut Frame, app: &App, area: Rect) {
    let [main, foot] = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).areas(area);
    let [list, side] = Layout::horizontal([Constraint::Length(40), Constraint::Min(20)]).areas(main);
    let mut lines = Vec::new();
    for (i, c) in challenge::ALL.iter().enumerate() {
        let sel = i == app.sel;
        let best = app.records.get(c.id);
        let st = stars(best.map(|b| b.1).unwrap_or(0));
        let style = if sel { Style::default().bg(Color::Rgb(40, 60, 85)).add_modifier(Modifier::BOLD) } else { Style::default() };
        lines.push(Line::from(vec![Span::styled(format!(" {:<24}", c.name), style.fg(TEXT)), Span::styled(st, style.fg(AMBER))]));
        lines.push(Line::from(Span::styled(format!("   {}", c.tagline), Style::default().fg(DIM))));
        lines.push(Line::from(""));
    }
    f.render_widget(Paragraph::new(lines).block(frame_block("The night league")), list);
    let c = &challenge::ALL[app.sel];
    let mut info = vec![Line::from(Span::styled(c.name, Style::default().fg(ICE).add_modifier(Modifier::BOLD))), Line::from(""), Line::from(c.brief), Line::from("")];
    info.push(Line::from(vec![Span::styled("Beacon   ", Style::default().fg(DIM)), Span::raw(c.beacon)]));
    info.push(Line::from(vec![Span::styled("Clock    ", Style::default().fg(DIM)), Span::raw(format!("{} s", c.limit))]));
    info.push(Line::from(vec![Span::styled("Stars    ", Style::default().fg(DIM)), Span::raw({
        let [two, three] = c.par_text();
        format!("★ {}   ★★ {two}   ★★★ {three}", if c.goal == Goal::Battle { "win" } else { "finish" })
    })]));
    match app.records.get(c.id) {
        Some((s, n, who)) => info.push(Line::from(vec![Span::styled("Best     ", Style::default().fg(DIM)), Span::styled(format!("{} in {s:.2} s by {who}", stars(n)), Style::default().fg(AMBER))])),
        None => info.push(Line::from(vec![Span::styled("Best     ", Style::default().fg(DIM)), Span::raw("nobody yet")])),
    }
    f.render_widget(Paragraph::new(info).wrap(Wrap { trim: true }).block(frame_block("Challenge")), side);
    keys_line(f, foot, &[("Enter", "pick and go to the bench"), ("R", "run it now"), ("Esc", "back")], None);
}

fn library(f: &mut Frame, app: &App, area: Rect) {
    let [main, foot] = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).areas(area);
    let [list, side] = Layout::horizontal([Constraint::Length(26), Constraint::Min(20)]).areas(main);
    let mut lines = Vec::new();
    let mut last = None;
    for (i, k) in parts::ALL.iter().enumerate() {
        if last != Some(k.category()) {
            if last.is_some() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(k.category().name(), Style::default().fg(cat_color(k.category())).add_modifier(Modifier::BOLD))));
            last = Some(k.category());
        }
        let style = if i == app.lib_sel { Style::default().bg(Color::Rgb(40, 60, 85)).add_modifier(Modifier::BOLD) } else { Style::default() };
        lines.push(Line::from(Span::styled(format!("  {:<18}", k.name()), style.fg(TEXT))));
    }
    f.render_widget(Paragraph::new(lines).block(frame_block("Parts")), list);
    let k = parts::ALL[app.lib_sel];
    let [pic, text] = Layout::vertical([Constraint::Length(BOX_H as u16 + 2), Constraint::Min(4)]).areas(side.inner(ratatui::layout::Margin::new(1, 1)));
    f.render_widget(frame_block(k.name()), side);
    let sample = Part::new(0, k, pic.x as i32 + 3, pic.y as i32 + 1);
    let label = sample.label(false);
    draw_box(f.buffer_mut(), pic, &sample, (0, 0), &label, None, false, false);
    let mut info = vec![Line::from(k.about()), Line::from("")];
    let ins = k.inputs();
    info.push(Line::from(vec![Span::styled("Inputs   ", Style::default().fg(DIM)), Span::raw(if ins.is_empty() { "none".to_string() } else { ins.join(", ") })]));
    info.push(Line::from(vec![Span::styled("Output   ", Style::default().fg(DIM)), Span::raw(if k.has_output() { "one, to as many inputs as you like" } else { "none: it drives the rover" })]));
    if !k.mounts().is_empty() {
        info.push(Line::from(vec![Span::styled("Mount    ", Style::default().fg(DIM)), Span::raw(k.mounts().iter().map(|m| m.name()).collect::<Vec<_>>().join(", "))]));
    }
    if !k.modes().is_empty() {
        info.push(Line::from(vec![Span::styled("Modes    ", Style::default().fg(DIM)), Span::raw(k.modes().join(", "))]));
    }
    if let Some(v) = k.value() {
        info.push(Line::from(vec![Span::styled("Setting  ", Style::default().fg(DIM)), Span::raw(format!("{} from {} to {}{}, {} to start", v.label, parts::fmt_num(v.min), parts::fmt_num(v.max), v.unit, parts::fmt_num(v.default)))]));
    }
    let slots = match k.category() {
        Category::Sensor => format!("the Mule has {} sensor slots", parts::SENSOR_SLOTS),
        Category::Logic => format!("the Mule's logic board holds {}", parts::LOGIC_SLOTS),
        Category::Drive => "one per rover, fitted on every Mule".to_string(),
        Category::Weapon => "one mount for each weapon on the Mule's nose".to_string(),
    };
    info.push(Line::from(vec![Span::styled("Fitting  ", Style::default().fg(DIM)), Span::raw(slots)]));
    f.render_widget(Paragraph::new(info).wrap(Wrap { trim: true }), text);
    keys_line(f, foot, &[("↑↓", "browse"), ("Esc", "back")], None);
}

const HELP: &str = r#"Every rover is a Mule: a squat ice-crawler with two drive tracks and a gyro. What it does is up to its wiring. Sensors feel the world, logic decides, drive moves. Signals are numbers; 0 is off and anything else is on.

THE BENCH
  arrows / hjkl   move the cursor (Shift: faster)
  Tab, Shift-Tab  jump from part to part
  a, Enter        add a part at the cursor
  Enter, e        set a part's mount, mode or value
                  (arrows change it, or type a number)
  w               wire from the part under the cursor, then
                  move to the part it feeds and press Enter
  m               move a part (arrows, then Enter)
  x, Delete       remove the part or wire under the cursor
  u               undo
  r, F5           run the rover in the challenge
  c               next challenge
  L               compile the wiring to a Lua rover
  n               rename the rover        p   parts library
  g               garage                  Esc back to the title
  Mouse: click a part to drag it, click its output to start a wire, right-click removes.

THE ARENA
  Space  pause      .  one tick at a time      f  faster      -  slower
  Tab    the live bench: every wire lit with its signal as the rover runs
  r      run again                              Esc  stop and rewire

LUA
  Any wiring compiles to Lua with L: a new rover beside the wired one, one line or two per part, that drives exactly as the wiring does. On a Lua rover's bench, e opens the script in your editor ($VISUAL or $EDITOR, else nano, vi or Notepad) and r runs it; print() shows in the live view. In the garage, l starts a rover in plain Lua.
  Sensors: bumper(side) whisker(side, reach) ranger(side) compass("heading"|"bearing"|"distance") radar("bearing"|"distance") gauge() time(). Drive: tracks(left, right) and gyro(spin), each -1 to 1. Weapons: fire() lob(metres) loaded("pellet"|"lobber"). Helpers: on(x) bit(test) clamp(x) round(x) print(...).
  Scripts get math, string, table and utf8, nothing that touches files or the system, and at most 50000 instructions a tick; a tick over budget drives nothing and the run goes on.

BOTS
  A bot is a program of your own, in any language, that drives a rover over plain text lines: Icewire writes its readings to the program's standard input sixty times a second and reads commands back from its standard output. B in the garage makes a bot rover from the Dodger example; e on its bench edits its command. PROTOCOL.md (beside the game) has every line. A bot gets 100 ms a tick and is out after 30 silent ticks in a row.
  icewire --tournament ROVER ROVER... runs rovers and bots against each other with nobody watching and saves every match as a replay; icewire --replay FILE watches one.

BATTLES
  Every rover starts with 100 charge. A pellet takes 5, a slush ball 15, and a rover at 0 stalls where it stands. Battle stars come from the charge you have left when you win. The radar finds the nearest rover through walls; the gauge reads your own charge.

FIRST STEPS
  Watch and learn, on the title screen, wires a rover in front of you, key by key, and runs it three times, each time cleverer: every key shown, Space to pause, the arrows to step. Then the league on the title screen is a season of seven nights, each a story and a challenge; free play opens every challenge at once. Homer waits in the garage. Run him in Ring Hall (r), watch, then try more power on the tracks, a compass in distance mode to slow for corners, or a ranger to see walls coming.

Rovers are plain text files in the state folder (rovers/*.rover); copy them, share them, edit them by hand."#;

fn help(f: &mut Frame, app: &App, area: Rect) {
    let [main, foot] = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).areas(area);
    f.render_widget(Paragraph::new(HELP).wrap(Wrap { trim: false }).scroll((app.help_scroll, 0)).block(frame_block("How to play")), main);
    keys_line(f, foot, &[("↑↓ PgDn", "scroll"), ("Esc", "back")], None);
}

// --------------------------------------------------------------- bench

/// Draws one part box. `off` maps bench cells to the screen.
#[allow(clippy::too_many_arguments)]
fn draw_box(buf: &mut Buffer, clip: Rect, p: &Part, off: (i32, i32), label: &str, live: Option<f64>, selected: bool, bad: bool) {
    let (x, y) = (p.x - off.0, p.y - off.1);
    let col = if bad {
        BAD
    } else if selected {
        Color::Rgb(255, 255, 255)
    } else {
        cat_color(p.kind.category())
    };
    let border = Style::default().fg(col);
    let inner_w = (BOX_W - 2) as usize;
    let name: String = p.kind.name().chars().take(inner_w).collect();
    let top = format!("┌{name}{}┐", "─".repeat(inner_w - name.chars().count()));
    put(buf, clip, x, y, &top, border.add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() }));
    for r in 1..BOX_H - 1 {
        put(buf, clip, x, y + r, "│", border);
        put(buf, clip, x + 1, y + r, &" ".repeat(inner_w), Style::default().bg(Color::Rgb(16, 24, 34)));
        put(buf, clip, x + BOX_W - 1, y + r, "│", border);
    }
    put(buf, clip, x, y + BOX_H - 1, &format!("└{}┘", "─".repeat(inner_w)), border);
    // ports
    let ins = p.kind.inputs();
    for (i, n) in ins.iter().enumerate() {
        let mark = if ins.len() == 1 { "▸".to_string() } else { n.chars().next().unwrap_or('?').to_string() };
        put(buf, clip, x, y + 1 + i as i32, &mark, Style::default().fg(TEXT).add_modifier(Modifier::BOLD));
    }
    if p.kind.has_output() {
        put(buf, clip, x + BOX_W - 1, y + 1, "▸", Style::default().fg(TEXT).add_modifier(Modifier::BOLD));
    }
    let lab: String = label.chars().take(inner_w - 1).collect();
    put(buf, clip, x + 2, y + 1, &lab, Style::default().fg(TEXT).bg(Color::Rgb(16, 24, 34)));
    if let Some(v) = live {
        let s = if p.kind.category() == Category::Drive { format!("{}%", parts::fmt_num(v * 100.0)) } else { parts::fmt_num(v) };
        let s = format!("{s:>w$}", w = inner_w);
        put(buf, clip, x + 1, y + 2, &s, Style::default().fg(signal_color(v)).bg(Color::Rgb(16, 24, 34)).add_modifier(Modifier::BOLD));
    }
}

fn bench(f: &mut Frame, app: &mut App, area: Rect) {
    let [head, main, foot] = Layout::vertical([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)]).areas(area);
    let side_w = if area.width >= 90 { 30 } else { 24 };
    let [view, side] = Layout::horizontal([Constraint::Min(30), Constraint::Length(side_w)]).areas(main);
    app.bench_area = view;
    app.follow_cursor(view.width as i32, view.height as i32);
    let ch = &challenge::ALL[app.ch];
    let d = &app.design;
    // header
    let mut hs = vec![
        Span::styled(" ICEWIRE ", Style::default().fg(Color::Black).bg(ICE).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", d.name), Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", ch.name), Style::default().fg(AMBER)),
        Span::styled(format!(" sensors {}/{}  logic {}/{} ", d.count(Category::Sensor), parts::SENSOR_SLOTS, d.count(Category::Logic), parts::LOGIC_SLOTS), Style::default().fg(DIM)),
    ];
    if let Some(sim) = &app.sim {
        hs.push(Span::styled(format!(" LIVE {:.1} s{} ", sim.secs(), if app.paused { " paused" } else { "" }), Style::default().fg(Color::Black).bg(WIRE_ON)));
    }
    f.render_widget(Paragraph::new(Line::from(hs)), head);

    let live = app.sim.as_ref().map(|s| &s.rovers[0].brain);
    let off = (app.vx - view.x as i32, app.vy - view.y as i32);
    let buf = f.buffer_mut();
    // pegboard
    for sy in 0..view.height as i32 {
        for sx in 0..view.width as i32 {
            let (bx, by) = (sx + app.vx, sy + app.vy);
            let (px, py) = (view.x + sx as u16, view.y + sy as u16);
            if bx >= BENCH_W || by >= BENCH_H {
                buf[(px, py)].set_char('░').set_fg(FAINT);
            } else if bx % 4 == 0 && by % 2 == 0 {
                buf[(px, py)].set_char('·').set_fg(FAINT);
            }
        }
    }
    // wires
    let value_of = |id: Option<u32>| -> Color {
        match (id, live) {
            (Some(id), Some(b)) => d.parts.iter().position(|p| p.id == id).map(|i| signal_color(b.out[i])).unwrap_or(WIRE_IDLE),
            _ => WIRE_IDLE,
        }
    };
    for sy in 0..view.height as i32 {
        for sx in 0..view.width as i32 {
            if let Some(c) = app.layout.at(sx + app.vx, sy + app.vy) {
                let color = value_of(c.h.or(c.v));
                let (px, py) = (view.x + sx as u16, view.y + sy as u16);
                buf[(px, py)].set_char(route::glyph(c.mask)).set_fg(color);
            }
        }
    }
    // boxes
    let moving = match app.mode {
        Mode::Move { id, .. } => Some(id),
        _ => None,
    };
    let wiring_from = match app.mode {
        Mode::Wire { from } => Some(from),
        _ => None,
    };
    let under = app.design.part_at(app.cx, app.cy);
    for (i, p) in d.parts.iter().enumerate() {
        let b_wired = d.wire_into(p.id, 1).is_some();
        let lv = live.map(|b| b.out[i]);
        let bad = moving == Some(p.id) && !d.fits(p.x, p.y, Some(p.id));
        let sel = under == Some(p.id) || moving == Some(p.id) || wiring_from == Some(p.id);
        draw_box(buf, view, p, off, &p.label(b_wired), lv, sel, bad);
    }
    // cursor
    let (sx, sy) = (app.cx - off.0, app.cy - off.1);
    if sx >= view.x as i32 && sy >= view.y as i32 && sx < (view.x + view.width) as i32 && sy < (view.y + view.height) as i32 {
        let cell = &mut buf[(sx as u16, sy as u16)];
        if cell.symbol() == " " {
            cell.set_char('+');
        }
        cell.set_style(Style::default().fg(Color::Black).bg(if wiring_from.is_some() { WIRE_ON } else { AMBER }));
    }
    bench_side(f, app, side);
    // footer
    let keys: &[(&str, &str)] = if app.running() {
        &[("Tab", "arena"), ("Space", "pause"), (".", "step"), ("f", "faster"), ("Esc", "stop")]
    } else {
        match app.mode {
            Mode::Move { .. } => &[("arrows", "move"), ("Enter", "drop here"), ("Esc", "put back")],
            Mode::Wire { .. } => &[("arrows/Tab", "find a part"), ("Enter", "connect"), ("Esc", "cancel")],
            Mode::Edit => &[("a", "add"), ("w", "wire"), ("Enter", "set"), ("m", "move"), ("x", "remove"), ("u", "undo"), ("r", "RUN"), ("c", "challenge"), ("?", "help")],
        }
    };
    keys_line(f, foot, keys, app.msg.as_ref().map(|m| m.0.as_str()));
}

fn bench_side(f: &mut Frame, app: &App, area: Rect) {
    let d = &app.design;
    let mut l: Vec<Line> = Vec::new();
    let live = app.sim.as_ref().map(|s| &s.rovers[0].brain);
    match app.under().and_then(|id| d.part(id)) {
        Some(p) => {
            let i = d.parts.iter().position(|q| q.id == p.id).unwrap_or(0);
            l.push(Line::from(Span::styled(p.kind.name(), Style::default().fg(cat_color(p.kind.category())).add_modifier(Modifier::BOLD))));
            l.push(Line::from(Span::styled(p.kind.summary(), Style::default().fg(DIM))));
            l.push(Line::from(""));
            if !p.kind.mounts().is_empty() {
                l.push(Line::from(format!("mount  {}", p.mount.name())));
            }
            if !p.kind.modes().is_empty() {
                l.push(Line::from(format!("mode   {}", p.kind.modes()[p.mode])));
            }
            if let Some(v) = p.kind.value() {
                l.push(Line::from(format!("{:<6} {}{}", v.label, parts::fmt_num(p.value), v.unit)));
            }
            for (k, n) in p.kind.inputs().iter().enumerate() {
                let from = d.wires.iter().find(|w| w.to == p.id && w.port == k).and_then(|w| d.part(w.from)).map(|f| f.kind.name()).unwrap_or("nothing");
                let val = live.map(|b| format!(" = {}", parts::fmt_num(b.ins[i][k]))).unwrap_or_default();
                l.push(Line::from(format!("{n:<6} from {from}{val}")));
            }
            let outs: Vec<&str> = d.wires.iter().filter(|w| w.from == p.id).filter_map(|w| d.part(w.to)).map(|t| t.kind.name()).collect();
            if p.kind.has_output() {
                l.push(Line::from(format!("out    to {}", if outs.is_empty() { "nothing yet".into() } else { outs.join(", ") })));
            }
            if let Some(b) = live {
                l.push(Line::from(Span::styled(format!("now    {}", parts::fmt_num(b.out[i])), Style::default().fg(signal_color(b.out[i])))));
            }
        }
        None => {
            if app.layout.wire_at(app.cx, app.cy).is_some() {
                l.push(Line::from(Span::styled("A wire", Style::default().fg(WIRE_IDLE).add_modifier(Modifier::BOLD))));
                l.push(Line::from(Span::styled("x removes it", Style::default().fg(DIM))));
            } else {
                l.push(Line::from(Span::styled("The bench", Style::default().fg(ICE).add_modifier(Modifier::BOLD))));
                l.push(Line::from(Span::styled("a adds a part here", Style::default().fg(DIM))));
            }
        }
    }
    l.push(Line::from(""));
    // the chassis and what sits where
    l.push(Line::from(Span::styled("The Mule", Style::default().fg(ICE).add_modifier(Modifier::BOLD))));
    for m in parts::Mount::SIDES {
        let on: Vec<&str> = d.parts.iter().filter(|p| !p.kind.mounts().is_empty() && p.mount == m).map(|p| p.kind.name()).collect();
        let all = d.parts.iter().filter(|p| p.kind == Kind::Bumper && p.mount == parts::Mount::All).count();
        let mut s = on.join(", ");
        if all > 0 {
            s = if s.is_empty() { "bumper (all)".into() } else { format!("{s}, bumper (all)") };
        }
        l.push(Line::from(vec![Span::styled(format!("{:<6}", m.name()), Style::default().fg(DIM)), Span::raw(if s.is_empty() { "-".to_string() } else { s })]));
    }
    l.push(Line::from(""));
    let ch = &challenge::ALL[app.ch];
    l.push(Line::from(Span::styled(ch.name, Style::default().fg(AMBER).add_modifier(Modifier::BOLD))));
    l.push(Line::from(Span::styled(format!("beacon: {}", ch.beacon), Style::default().fg(DIM))));
    if let Some((s, n, _)) = app.records.get(ch.id) {
        l.push(Line::from(format!("best {s:.2} s {}", stars(n))));
    }
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: true }).block(Block::default().borders(Borders::LEFT).border_style(Style::default().fg(FAINT))), area);
}

// ----------------------------------------------------------------- lua

const LUA_WORDS: [&str; 22] = ["and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while"];
const LUA_API: [&str; 17] = ["bumper", "whisker", "ranger", "compass", "radar", "gauge", "time", "tracks", "gyro", "fire", "lob", "loaded", "on", "bit", "clamp", "round", "print"];

/// One line of Lua in colour: comments, strings, keywords, the rover's
/// own calls and numbers.
fn lua_line(src: &str) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Span<'static>>| {
        if !plain.is_empty() {
            out.push(Span::styled(std::mem::take(plain), Style::default().fg(TEXT)));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '-' && chars.get(i + 1) == Some(&'-') {
            flush(&mut plain, &mut out);
            out.push(Span::styled(chars[i..].iter().collect::<String>(), Style::default().fg(Color::Rgb(110, 150, 120))));
            break;
        }
        if c == '"' || c == '\'' {
            flush(&mut plain, &mut out);
            let mut j = i + 1;
            while j < chars.len() && chars[j] != c {
                j += if chars[j] == '\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(chars.len());
            out.push(Span::styled(chars[i..end].iter().collect::<String>(), Style::default().fg(AMBER)));
            i = end;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let w: String = chars[i..j].iter().collect();
            let style = if LUA_WORDS.contains(&w.as_str()) {
                Some(Style::default().fg(Color::Rgb(185, 160, 255)).add_modifier(Modifier::BOLD))
            } else if LUA_API.contains(&w.as_str()) {
                Some(Style::default().fg(Color::Rgb(90, 205, 215)))
            } else {
                None
            };
            match style {
                Some(st) => {
                    flush(&mut plain, &mut out);
                    out.push(Span::styled(w, st));
                }
                None => plain.push_str(&w),
            }
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            flush(&mut plain, &mut out);
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '.') {
                j += 1;
            }
            out.push(Span::styled(chars[i..j].iter().collect::<String>(), Style::default().fg(WIRE_ON)));
            i = j;
            continue;
        }
        plain.push(c);
        i += 1;
    }
    flush(&mut plain, &mut out);
    out
}

fn lua_view(f: &mut Frame, app: &mut App, area: Rect) {
    let [head, main, foot] = Layout::vertical([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)]).areas(area);
    let side_w = if area.width >= 90 { 34 } else { 26 };
    let [code, side] = Layout::horizontal([Constraint::Min(30), Constraint::Length(side_w)]).areas(main);
    let ch = &challenge::ALL[app.ch];
    let script = app.design.lua.clone().unwrap_or_default();
    let mut hs = vec![
        Span::styled(" ICEWIRE ", Style::default().fg(Color::Black).bg(ICE).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", app.design.name), Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", ch.name), Style::default().fg(AMBER)),
        Span::styled(" Lua ", Style::default().fg(DIM)),
    ];
    if let Some(sim) = &app.sim {
        hs.push(Span::styled(format!(" LIVE {:.1} s{} ", sim.secs(), if app.paused { " paused" } else { "" }), Style::default().fg(Color::Black).bg(WIRE_ON)));
    }
    f.render_widget(Paragraph::new(Line::from(hs)), head);
    let lines: Vec<&str> = script.lines().collect();
    let max_scroll = lines.len().saturating_sub(code.height as usize / 2) as u16;
    app.lua_scroll = app.lua_scroll.min(max_scroll);
    let width = lines.len().to_string().len().max(2);
    let body: Vec<Line> = lines
        .iter()
        .enumerate()
        .skip(app.lua_scroll as usize)
        .take(code.height as usize)
        .map(|(n, l)| {
            let mut spans = vec![Span::styled(format!("{:>width$} ", n + 1), Style::default().fg(FAINT))];
            spans.extend(lua_line(l));
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(body), code);
    // the side: what it is, whether it loads, what it says
    let mut l = vec![Line::from(Span::styled("A rover in Lua", Style::default().fg(ICE).add_modifier(Modifier::BOLD)))];
    l.push(Line::from(Span::styled(format!("{} lines, at most {} instructions a tick", lines.len(), crate::lua::BUDGET), Style::default().fg(DIM))));
    l.push(Line::from(""));
    match app.sim.as_ref().and_then(|s| s.rovers[0].lua.as_ref()) {
        Some(lb) => {
            match &lb.error {
                Some(e) => l.push(Line::from(Span::styled(e.clone(), Style::default().fg(BAD)))),
                None => l.push(Line::from(Span::styled("Running", Style::default().fg(GOOD)))),
            }
            l.push(Line::from(""));
            l.push(Line::from(Span::styled("print()", Style::default().fg(DIM))));
            let console = lb.console.borrow();
            let room = (side.height as usize).saturating_sub(l.len() + 1);
            for line in console.iter().skip(console.len().saturating_sub(room)) {
                l.push(Line::from(line.clone()));
            }
        }
        None if app.sim.is_none() => match crate::lua::LuaBrain::new(&script) {
            Ok(_) => l.push(Line::from(Span::styled("The script loads.", Style::default().fg(GOOD)))),
            Err(e) => l.push(Line::from(Span::styled(format!("It will not load: {e}"), Style::default().fg(BAD)))),
        },
        None => {}
    }
    if app.sim.is_none() {
        l.push(Line::from(""));
        l.push(Line::from(Span::styled("e opens it in your editor ($EDITOR); save and close to come back. On a wired rover, L compiles its wiring to Lua like this.", Style::default().fg(DIM))));
    }
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: true }).block(Block::default().borders(Borders::LEFT).border_style(Style::default().fg(FAINT))), side);
    let keys: &[(&str, &str)] = if app.running() {
        &[("Tab", "arena"), ("↑↓", "scroll"), ("Space", "pause"), ("f", "faster"), ("Esc", "stop")]
    } else {
        &[("e", "edit"), ("r", "RUN"), ("↑↓ PgDn", "scroll"), ("c", "challenge"), ("n", "rename"), ("g", "garage"), ("?", "help")]
    };
    keys_line(f, foot, keys, app.msg.as_ref().map(|m| m.0.as_str()));
}

// ---------------------------------------------------------------- bots

const PROTOCOL_BRIEF: &str = "Icewire starts the program and writes \"icewire-bot 1\", the rover's name and the challenge, then \"end\"; the bot answers \"icewire-bot 1\". Then, sixty times a second, Icewire writes a block of readings:

tick 120
time 2
ranger nose 3.2 right 0.8 tail 8 left 0.6
bumper nose 0 right 0 tail 0 left 0
compass heading 90 bearing -12.5 distance 14.2
radar bearing 33 distance 5.1
charge 100
loaded pellet 1 lobber 0
speed 1.92
end

and the bot answers with any of these, then \"end\":

tracks LEFT RIGHT   each -1 to 1
gyro SPIN           -1 to 1, positive turns right
fire
lob METRES

Whatever it writes to standard error shows here during a run. A bot gets 100 ms a tick; one that goes quiet for 30 ticks in a row, or ends, is out. PROTOCOL.md has it all.";

fn bot_view(f: &mut Frame, app: &mut App, area: Rect) {
    let [head, main, foot] = Layout::vertical([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)]).areas(area);
    let side_w = if area.width >= 90 { 34 } else { 26 };
    let [doc, side] = Layout::horizontal([Constraint::Min(30), Constraint::Length(side_w)]).areas(main);
    let ch = &challenge::ALL[app.ch];
    let spec = app.design.bot.clone().expect("bot");
    let mut hs = vec![
        Span::styled(" ICEWIRE ", Style::default().fg(Color::Black).bg(ICE).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", app.design.name), Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", ch.name), Style::default().fg(AMBER)),
        Span::styled(" bot ", Style::default().fg(DIM)),
    ];
    if let Some(sim) = &app.sim {
        hs.push(Span::styled(format!(" LIVE {:.1} s{} ", sim.secs(), if app.paused { " paused" } else { "" }), Style::default().fg(Color::Black).bg(WIRE_ON)));
    }
    f.render_widget(Paragraph::new(Line::from(hs)), head);
    let mut l = vec![
        Line::from(vec![Span::styled("runs   ", Style::default().fg(DIM)), Span::styled(spec.command.clone(), Style::default().fg(AMBER))]),
        Line::from(vec![Span::styled("in     ", Style::default().fg(DIM)), Span::raw(spec.dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "the folder Icewire started in".into()))]),
        Line::from(""),
    ];
    for p in PROTOCOL_BRIEF.lines() {
        l.push(Line::from(Span::styled(p.to_string(), Style::default().fg(if p.starts_with(|c: char| c.is_ascii_lowercase()) { WIRE_ON } else { TEXT }))));
    }
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }).scroll((app.lua_scroll, 0)), doc);
    let mut s = vec![Line::from(Span::styled("A bot", Style::default().fg(ICE).add_modifier(Modifier::BOLD))), Line::from("")];
    match app.sim.as_ref().map(|sim| &sim.rovers[0]) {
        Some(e) => {
            match &e.bot {
                Some(b) => {
                    match &b.dead {
                        Some(why) => s.push(Line::from(Span::styled(format!("Out: {why}"), Style::default().fg(BAD)))),
                        None => s.push(Line::from(Span::styled("Running", Style::default().fg(GOOD)))),
                    }
                    if let Some(c) = &b.confused {
                        s.push(Line::from(Span::styled(format!("Did not understand: {c}"), Style::default().fg(AMBER))));
                    }
                    s.push(Line::from(""));
                    s.push(Line::from(Span::styled("standard error", Style::default().fg(DIM))));
                    if let Ok(c) = b.console.lock() {
                        let room = (side.height as usize).saturating_sub(s.len() + 1);
                        for line in c.iter().skip(c.len().saturating_sub(room)) {
                            s.push(Line::from(line.clone()));
                        }
                    }
                }
                None => {
                    if let Some(ev) = app.sim.as_ref().and_then(|sim| sim.events.iter().find(|e| e.1.contains("will not start"))) {
                        s.push(Line::from(Span::styled(ev.1.clone(), Style::default().fg(BAD))));
                    }
                }
            }
        }
        None => s.push(Line::from(Span::styled("r starts it in the arena. e edits its command (the .bot file) in your editor. icewire --tournament pits bots against each other with nobody watching.", Style::default().fg(DIM)))),
    }
    f.render_widget(Paragraph::new(s).wrap(Wrap { trim: true }).block(Block::default().borders(Borders::LEFT).border_style(Style::default().fg(FAINT))), side);
    let keys: &[(&str, &str)] = if app.running() { &[("Tab", "arena"), ("Space", "pause"), ("f", "faster"), ("Esc", "stop")] } else { &[("r", "RUN"), ("e", "edit"), ("↑↓", "scroll"), ("c", "challenge"), ("n", "rename"), ("g", "garage"), ("?", "help")] };
    keys_line(f, foot, keys, app.msg.as_ref().map(|m| m.0.as_str()));
}

// --------------------------------------------------------------- arena

/// Cells per tile across and down; a tile is 8 by 8 Braille dots.
const TW: i32 = 4;
const TH: i32 = 2;
const DPM: f64 = 8.0;

fn arena(f: &mut Frame, app: &App, area: Rect) {
    let Some(sim) = &app.sim else { return };
    let [main, feed, foot] = Layout::vertical([Constraint::Min(8), Constraint::Length(2), Constraint::Length(1)]).areas(area);
    let side_w = if area.width >= 90 { 28 } else { 24 };
    let [view, side] = Layout::horizontal([Constraint::Min(30), Constraint::Length(side_w)]).areas(main);
    draw_map(f.buffer_mut(), sim, view);
    arena_side(f, app, sim, side);
    let ev: Vec<Line> = sim.events.iter().rev().take(2).map(|(t, m)| Line::from(vec![Span::styled(format!(" {:>6.1}s ", *t as f64 / 60.0), Style::default().fg(DIM)), Span::raw(m.clone())])).collect();
    f.render_widget(Paragraph::new(ev), feed);
    let keys: &[(&str, &str)] = if app.replay.is_some() {
        &[("Space", "pause"), (".", "step"), ("←→", "skip 5 s"), ("f", "faster"), ("r", "from the start"), ("Esc", "leave")]
    } else {
        &[("Space", "pause"), (".", "step"), ("f", "faster"), ("Tab", "live wiring"), ("r", "again"), ("Esc", "stop")]
    };
    keys_line(f, foot, keys, app.msg.as_ref().map(|m| m.0.as_str()));
}

fn draw_map(buf: &mut Buffer, sim: &Sim, view: Rect) {
    let a = &sim.arena;
    let me = &sim.rovers[0].rover;
    let (mw, mh) = (a.w * TW, a.h * TH);
    let (vw, vh) = (view.width as i32, view.height as i32);
    // camera in cells: on the rover, inside the map
    let cam = |pos: f64, scale: i32, view: i32, map: i32| -> i32 {
        if map <= view { -(view - map) / 2 } else { ((pos * scale as f64) as i32 - view / 2).clamp(0, map - view) }
    };
    let ox = cam(me.x, TW, vw, mw);
    let oy = cam(me.y, TH, vh, mh);
    let ring = a.ring;
    for sy in 0..vh {
        for sx in 0..vw {
            let (cx, cy) = (sx + ox, sy + oy);
            let cell = &mut buf[(view.x + sx as u16, view.y + sy as u16)];
            if cx < 0 || cy < 0 || cx >= mw || cy >= mh {
                cell.set_char(' ').set_bg(Color::Reset);
                continue;
            }
            let (tx, ty) = (cx.div_euclid(TW), cy.div_euclid(TH));
            if a.window.get((ty * a.w + tx) as usize).copied().unwrap_or(false) {
                // the long window: night sky, and Jupiter's bands across the middle
                let sky = Color::Rgb(10, 12, 26);
                if cy % TH == TH - 1 {
                    cell.set_char('▄').set_fg(WALL_EDGE).set_bg(sky);
                } else {
                    let span = mw as f64;
                    let f = cx as f64 / span;
                    if (0.35..0.65).contains(&f) {
                        let bands = [Color::Rgb(225, 195, 150), Color::Rgb(185, 130, 90), Color::Rgb(235, 215, 180), Color::Rgb(200, 90, 60)];
                        let k = ((f - 0.35) * 40.0) as usize;
                        cell.set_char('▀').set_fg(bands[k % 3]).set_bg(bands[(k + 1) % 3]);
                        if k == 7 {
                            cell.set_bg(bands[3]);
                        }
                    } else {
                        cell.set_char(if (cx * 7 + 3) % 11 == 0 { '·' } else { ' ' }).set_fg(DIM).set_bg(sky);
                    }
                }
                continue;
            }
            if a.solid(tx, ty) {
                // walls: lit edge where they face open floor
                let open_below = !a.solid(tx, ty + 1) && cy % TH == TH - 1;
                cell.set_char(if open_below { '▄' } else { ' ' }).set_fg(WALL_EDGE).set_bg(WALL);
                continue;
            }
            let i = (ty * a.w + tx) as usize;
            let (wx, wy) = ((cx as f64 + 0.5) / TW as f64, (cy as f64 + 0.5) / TH as f64);
            if let Some((rx, ry, rr)) = ring
                && (wx - rx).powi(2) + (wy - ry).powi(2) > rr * rr
            {
                cell.set_char(if (cx + cy * 3) % 7 == 0 { '~' } else { ' ' }).set_fg(Color::Rgb(40, 60, 85)).set_bg(SLUSH);
                continue;
            }
            if a.cp[i] > 0 {
                let k = a.cp[i] as usize;
                let next = me.next_cp + 1;
                let (ch, col) = if k == next {
                    ('·', AMBER)
                } else if k < next {
                    (' ', FAINT)
                } else {
                    ('·', DIM)
                };
                cell.set_char(ch).set_fg(col);
                // the checkpoint number in its middle tile
                if k >= next && cx % TW == 1 && cy % TH == 0 {
                    let cen = a.cp_centres[k - 1];
                    if (cen.0 - (tx as f64 + 0.5)).abs() < 0.6 && (cen.1 - (ty as f64 + 0.5)).abs() < 0.6 {
                        cell.set_char(char::from_digit(k as u32, 10).unwrap_or('?')).set_fg(if k == next { AMBER } else { DIM });
                    }
                }
                continue;
            }
            if a.exit[i] {
                cell.set_char('░').set_fg(GOOD);
                continue;
            }
            cell.set_char(' ').set_bg(Color::Reset);
        }
    }
    let mut dots = Dots::new(view);
    let to_dot = |x: f64, y: f64| (x * DPM - (ox * 2) as f64, y * DPM - (oy * 4) as f64);
    // the ring's edge
    if let Some((rx, ry, rr)) = ring {
        let n = 240;
        for k in 0..n {
            let t = k as f64 / n as f64 * geom::TAU;
            let (x, y) = to_dot(rx + rr * geom::cos(t), ry + rr * geom::sin(t));
            dots.set(x.round() as i32, y.round() as i32, ICE);
        }
    }
    // trails
    for (n, e) in sim.rovers.iter().enumerate() {
        let col = if n == 0 { Color::Rgb(50, 95, 115) } else { Color::Rgb(110, 65, 45) };
        for (k, (x, y)) in e.rover.trail.iter().enumerate() {
            if k % 2 == 0 {
                let (x, y) = to_dot(*x, *y);
                dots.set(x as i32, y as i32, col);
            }
        }
    }
    // sensor beams
    for b in &sim.beams {
        let (x0, y0) = to_dot(b.x0, b.y0);
        let (x1, y1) = to_dot(b.x1, b.y1);
        dots.line(x0, y0, x1, y1, Color::Rgb(150, 120, 60), 2);
    }
    // pellets, with a short tail
    for p in &sim.pellets {
        let (x, y) = to_dot(p.x, p.y);
        let (tx, ty) = to_dot(p.x - p.vx * 0.05, p.y - p.vy * 0.05);
        dots.line(tx, ty, x, y, if p.owner == 0 { Color::Rgb(200, 245, 255) } else { Color::Rgb(255, 200, 170) }, 1);
    }
    // lobs: a shadow on the ground and the ball above it, highest midway
    for l in &sim.lobs {
        let t = l.t as f64 / l.flight as f64;
        let (gx, gy) = (l.x0 + (l.x1 - l.x0) * t, l.y0 + (l.y1 - l.y0) * t);
        let (sx, sy) = to_dot(gx, gy);
        dots.set(sx as i32, sy as i32, DIM);
        let lift = 4.0 * t * (1.0 - t) * 14.0;
        let (bx, by) = (sx as i32, (sy - lift) as i32);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            dots.set(bx + dx, by + dy, Color::Rgb(230, 240, 255));
        }
        let (tx, ty) = to_dot(l.x1, l.y1);
        dots.set(tx as i32, ty as i32, Color::Rgb(150, 70, 70));
    }
    // splashes and sparks
    for &(x, y, t, big) in &sim.splashes {
        let age = sim.tick.saturating_sub(t) as f64;
        let rmax = if big { crate::sim::LOB_SPLASH } else { 0.25 };
        let rr = rmax * (age / 12.0).min(1.0);
        if age > 18.0 {
            continue;
        }
        let (cx, cy) = to_dot(x, y);
        let n = if big { 24 } else { 8 };
        for k in 0..n {
            let a = k as f64 / n as f64 * geom::TAU;
            dots.set((cx + geom::cos(a) * rr * DPM).round() as i32, (cy + geom::sin(a) * rr * DPM).round() as i32, if big { Color::Rgb(220, 235, 255) } else { AMBER });
        }
    }
    // rovers
    for (n, e) in sim.rovers.iter().enumerate() {
        let r = &e.rover;
        if r.fell.is_some_and(|t| sim.tick > t + 30) {
            continue;
        }
        let flash = r.hit_at.is_some_and(|t| sim.tick < t + 6);
        let col = if r.stalled.is_some() {
            Color::Rgb(70, 80, 90)
        } else if flash {
            Color::Rgb(255, 255, 255)
        } else if n == 0 {
            ME
        } else {
            THEM
        };
        let (cx, cy) = to_dot(r.x, r.y);
        let rad = RADIUS * DPM;
        let ri = rad.ceil() as i32 + 1;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let (px, py) = (cx.floor() as i32 + dx, cy.floor() as i32 + dy);
                let (fx, fy) = (px as f64 + 0.5 - cx, py as f64 + 0.5 - cy);
                if fx * fx + fy * fy <= rad * rad {
                    dots.set(px, py, col);
                }
            }
        }
        let (hx, hy) = geom::dir(r.h);
        dots.line(cx, cy, cx + hx * rad * 1.9, cy + hy * rad * 1.9, col, 1);
    }
    dots.render(buf);
}

fn bar(v: f64, w: usize) -> String {
    // a centred bar from -1 to 1
    let half = w / 2;
    let n = ((v.abs() * half as f64).round() as usize).min(half);
    let mut s = vec!['·'; w];
    for k in 0..n {
        if v > 0.0 {
            s[half + k] = '█';
        } else {
            s[half - 1 - k] = '█';
        }
    }
    s.into_iter().collect()
}

fn arena_side(f: &mut Frame, app: &App, sim: &Sim, area: Rect) {
    let ch = sim.ch;
    let e = &sim.rovers[0];
    let r = &e.rover;
    let mut l = vec![Line::from(Span::styled(ch.name, Style::default().fg(AMBER).add_modifier(Modifier::BOLD)))];
    let tcol = if sim.secs() > ch.limit - 10.0 { BAD } else { TEXT };
    l.push(Line::from(vec![Span::styled("Clock  ", Style::default().fg(DIM)), Span::styled(format!("{:5.1} / {} s", sim.secs(), ch.limit), Style::default().fg(tcol))]));
    match ch.goal {
        Goal::Race => l.push(Line::from(vec![Span::styled("Passed ", Style::default().fg(DIM)), Span::raw(format!("{} of {} checkpoints", r.next_cp, sim.arena.cp_centres.len()))])),
        Goal::Maze => {
            let (bx, by) = sim.beacon(0);
            let dist = ((bx - r.x).powi(2) + (by - r.y).powi(2)).sqrt();
            l.push(Line::from(vec![Span::styled("Hatch  ", Style::default().fg(DIM)), Span::raw(format!("{dist:.1} m away"))]));
        }
        Goal::Sumo => {
            let (cx, cy, rr) = sim.arena.ring.unwrap_or((0.0, 0.0, 1.0));
            let edge = |r: &crate::sim::Rover| rr - ((r.x - cx).powi(2) + (r.y - cy).powi(2)).sqrt();
            l.push(Line::from(vec![Span::styled("Edge   ", Style::default().fg(DIM)), Span::raw(format!("you {:.1} m", edge(r).max(0.0)))]));
            if let Some(o) = sim.rovers.get(1) {
                l.push(Line::from(vec![Span::styled("       ", Style::default().fg(DIM)), Span::styled(format!("{} {:.1} m", o.design.name, edge(&o.rover).max(0.0)), Style::default().fg(THEM))]));
            }
        }
        Goal::Battle => {
            let w = (area.width as usize).saturating_sub(14).clamp(5, 13);
            for (n, e) in sim.rovers.iter().enumerate() {
                let c = e.rover.charge;
                let filled = ((c / crate::sim::CHARGE) * w as f64).round() as usize;
                let bar: String = "█".repeat(filled) + &"·".repeat(w - filled.min(w));
                let name: String = if n == 0 && app.replay.is_none() { "You".to_string() } else { e.design.name.chars().take(8).collect() };
                let col = if c < 30.0 { BAD } else if n == 0 { ME } else { THEM };
                l.push(Line::from(vec![Span::styled(format!("{name:<9}"), Style::default().fg(DIM)), Span::styled(bar, Style::default().fg(col)), Span::raw(format!(" {c:.0}"))]));
            }
        }
    }
    if let Some((s, n, _)) = app.records.get(ch.id) {
        l.push(Line::from(vec![Span::styled("Best   ", Style::default().fg(DIM)), Span::raw(format!("{s:.2} s {}", stars(n)))]));
    }
    if app.replay.is_some() {
        l.push(Line::from(""));
        l.push(Line::from(Span::styled("A replay", Style::default().fg(ICE).add_modifier(Modifier::BOLD))));
        for (n, e) in sim.rovers.iter().enumerate() {
            l.push(Line::from(Span::styled(e.design.name.clone(), Style::default().fg(if n == 0 { ME } else { THEM }))));
        }
        let state = if app.paused { "paused" } else { "" };
        l.push(Line::from(""));
        l.push(Line::from(Span::styled(format!("x{}  {state}", SPEEDS[app.speed]), Style::default().fg(AMBER))));
        f.render_widget(Paragraph::new(l).wrap(Wrap { trim: true }).block(Block::default().borders(Borders::LEFT).border_style(Style::default().fg(FAINT))), area);
        return;
    }
    l.push(Line::from(""));
    l.push(Line::from(Span::styled(e.design.name.clone(), Style::default().fg(ME).add_modifier(Modifier::BOLD))));
    l.push(Line::from(vec![Span::styled("Speed  ", Style::default().fg(DIM)), Span::raw(format!("{:.1} m/s", r.speed()))]));
    l.push(Line::from(vec![Span::styled("Facing ", Style::default().fg(DIM)), Span::raw(format!("{:.0}°", r.heading_deg()))]));
    if let Some(b) = &e.bot {
        l.push(Line::from(""));
        match &b.dead {
            Some(why) => l.push(Line::from(Span::styled(format!("out: {why}"), Style::default().fg(BAD)))),
            None => l.push(Line::from(Span::styled("bot", Style::default().fg(DIM)))),
        }
        if let Ok(c) = b.console.lock() {
            for line in c.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev() {
                l.push(Line::from(Span::styled(line.chars().take(area.width as usize).collect::<String>(), Style::default().fg(TEXT))));
            }
        }
    }
    if let Some(lb) = &e.lua {
        l.push(Line::from(""));
        match &lb.error {
            Some(err) => l.push(Line::from(Span::styled(err.chars().take(area.width as usize * 2).collect::<String>(), Style::default().fg(BAD)))),
            None => l.push(Line::from(Span::styled("Lua", Style::default().fg(DIM)))),
        }
        for line in lb.console.borrow().iter().rev().take(4).collect::<Vec<_>>().into_iter().rev() {
            l.push(Line::from(Span::styled(line.chars().take(area.width as usize).collect::<String>(), Style::default().fg(TEXT))));
        }
    }
    let sensors: Vec<(usize, &Part)> = e.design.parts.iter().enumerate().filter(|(_, p)| p.kind.category() == Category::Sensor).collect();
    if !sensors.is_empty() {
        l.push(Line::from(""));
        for (i, p) in sensors.iter().take(6) {
            let what = match p.kind {
                Kind::Compass => parts::COMPASS_MODES[p.mode].to_string(),
                _ => p.mount.name().to_string(),
            };
            let v = e.brain.out[*i];
            l.push(Line::from(vec![Span::styled(format!("{:<8}{:<9}", p.kind.name(), what), Style::default().fg(DIM)), Span::styled(parts::fmt_num(v), Style::default().fg(signal_color(v)))]));
        }
    }
    l.push(Line::from(""));
    let w = (area.width as usize).saturating_sub(10).clamp(6, 16);
    for (name, v) in [("Left  ", r.drive.left), ("Right ", r.drive.right), ("Gyro  ", r.drive.gyro)] {
        l.push(Line::from(vec![Span::styled(name, Style::default().fg(DIM)), Span::styled(bar(v, w), Style::default().fg(signal_color(v)))]));
    }
    l.push(Line::from(""));
    let mut state = format!("x{}", SPEEDS[app.speed]);
    if app.paused {
        state.push_str("  paused");
    }
    l.push(Line::from(Span::styled(state, Style::default().fg(AMBER))));
    f.render_widget(Paragraph::new(l).block(Block::default().borders(Borders::LEFT).border_style(Style::default().fg(FAINT))), area);
}

// -------------------------------------------------------------- popups

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

fn result_card(f: &mut Frame, app: &App, r: &RunResult, area: Rect) {
    let ch = &challenge::ALL[app.ch];
    let rect = centered(area, 46, 9);
    f.render_widget(Clear, rect);
    let mut l = vec![Line::from(Span::styled(r.text.clone(), Style::default().fg(if r.won { GOOD } else { AMBER }).add_modifier(Modifier::BOLD))), Line::from("")];
    if r.won {
        l.push(Line::from(vec![Span::styled(stars(r.stars), Style::default().fg(AMBER)), Span::raw(if r.best { "   a new best!" } else { "" })]));
        let next = match r.stars {
            1 => format!("{} {} for two stars", if ch.goal == Goal::Battle { "Win" } else { "Finish" }, ch.par_text()[0]),
            2 => format!("{} {} for three", if ch.goal == Goal::Battle { "Win" } else { "Finish" }, ch.par_text()[1]),
            _ => "Hana will want to see that wiring".to_string(),
        };
        l.push(Line::from(Span::styled(next, Style::default().fg(DIM))));
    } else {
        l.push(Line::from(Span::styled("Tab shows the wiring as it was when it ended.", Style::default().fg(DIM))));
    }
    l.push(Line::from(""));
    let enter = if r.story { " the story goes on   " } else { " back to the bench   " };
    l.push(Line::from(vec![Span::styled("Enter", Style::default().fg(AMBER)), Span::raw(enter), Span::styled("R", Style::default().fg(AMBER)), Span::raw(" run again")]));
    f.render_widget(Paragraph::new(l).block(frame_block(ch.name)).centered(), rect);
}

fn popup(f: &mut Frame, app: &App, p: &Popup, area: Rect) {
    match p {
        Popup::Add { sel } => {
            let lib = crate::app::library();
            let h = (lib.len() as u16 + 6).min(area.height.saturating_sub(2));
            let rect = centered(area, 70, h);
            f.render_widget(Clear, rect);
            let mut l = Vec::new();
            let mut last = None;
            for (i, k) in lib.iter().enumerate() {
                if last != Some(k.category()) {
                    l.push(Line::from(Span::styled(k.category().name(), Style::default().fg(cat_color(k.category())).add_modifier(Modifier::BOLD))));
                    last = Some(k.category());
                }
                let room = app.design.room_for(*k).is_ok();
                let style = if i == *sel {
                    Style::default().bg(Color::Rgb(40, 60, 85)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let fg = if room { TEXT } else { FAINT };
                l.push(Line::from(vec![Span::styled(format!("  {:<12}", k.name()), style.fg(fg)), Span::styled(k.summary(), Style::default().fg(DIM))]));
            }
            // keep the chosen part in view
            let at = lib.iter().take(*sel + 1).fold((0u16, None), |(n, last), k| (n + 1 + (last != Some(k.category())) as u16, Some(k.category()))).0;
            let inner_h = rect.height.saturating_sub(2);
            let scroll = at.saturating_sub(inner_h);
            f.render_widget(Paragraph::new(l).scroll((scroll, 0)).block(frame_block("Add a part")), rect);
        }
        Popup::Edit { id, field, typed } => {
            let Some(part) = app.design.part(*id) else { return };
            let fields = App::edit_fields(part.kind);
            let rect = centered(area, 52, fields.len() as u16 + 6);
            f.render_widget(Clear, rect);
            let mut l = vec![Line::from(Span::styled(part.kind.summary(), Style::default().fg(DIM))), Line::from("")];
            for (i, name) in fields.iter().enumerate() {
                let val = match *name {
                    "mount" => part.mount.name().to_string(),
                    "mode" => part.kind.modes()[part.mode].to_string(),
                    _ => {
                        let spec = part.kind.value().expect("value");
                        if i == *field && !typed.is_empty() { format!("{typed}_") } else { format!("{}{}", parts::fmt_num(part.value), spec.unit) }
                    }
                };
                let label = if *name == "value" { part.kind.value().map(|v| v.label).unwrap_or("value") } else { name };
                let sel = i == *field;
                let style = if sel { Style::default().fg(Color::Black).bg(AMBER) } else { Style::default().fg(TEXT) };
                l.push(Line::from(vec![Span::styled(format!("  {label:<7}"), Style::default().fg(DIM)), Span::styled(format!(" ◂ {val} ▸ "), style)]));
            }
            l.push(Line::from(""));
            l.push(Line::from(Span::styled("←→ change  ↑↓ field  type a number  Enter done", Style::default().fg(DIM))));
            f.render_widget(Paragraph::new(l).block(frame_block(part.kind.name())), rect);
        }
        Popup::Name { buf, new } => {
            let rect = centered(area, 40, 5);
            f.render_widget(Clear, rect);
            let l = vec![Line::from(""), Line::from(vec![Span::raw("  "), Span::styled(format!("{buf}_"), Style::default().fg(AMBER))])];
            f.render_widget(Paragraph::new(l).block(frame_block(if *new { "Name the new rover" } else { "Rename the rover" })), rect);
        }
        Popup::Port { to, sel, .. } => {
            let Some(t) = app.design.part(*to) else { return };
            let ins = t.kind.inputs();
            let rect = centered(area, 36, ins.len() as u16 + 4);
            f.render_widget(Clear, rect);
            let mut l = vec![Line::from(Span::styled("Into which input?", Style::default().fg(DIM)))];
            for (i, n) in ins.iter().enumerate() {
                let used = app.design.wire_into(t.id, i).is_some();
                let style = if i == *sel { Style::default().fg(Color::Black).bg(AMBER) } else { Style::default().fg(TEXT) };
                l.push(Line::from(Span::styled(format!("  {n}{}", if used { " (replaces its wire)" } else { "" }), style)));
            }
            f.render_widget(Paragraph::new(l).block(frame_block(t.kind.name())), rect);
        }
        Popup::Delete { stem } => {
            let rect = centered(area, 44, 5);
            f.render_widget(Clear, rect);
            let l = vec![Line::from(""), Line::from(format!("  Delete rovers/{stem}.rover?  y / n"))];
            f.render_widget(Paragraph::new(l).block(frame_block("Delete")), rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn screen_text(app: &mut App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        let mut s = String::new();
        for y in 0..h {
            for x in 0..w {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn every_screen_draws_at_both_sizes() {
        for (w, h) in [(100, 30), (80, 24)] {
            let mut app = App::new(false);
            app.chapter = Some(3);
            for s in [Screen::Title, Screen::League, Screen::Story, Screen::Garage, Screen::Challenges, Screen::Library, Screen::Help, Screen::Bench] {
                app.screen = s;
                let text = screen_text(&mut app, w, h);
                assert!(!text.trim().is_empty());
            }
            app.screen = Screen::Bench;
            let text = screen_text(&mut app, w, h);
            assert!(text.contains("┌Compass"), "{text}");
            let wired = app.design.clone();
            app.design = crate::design::Design::from_lua(&crate::lua::compile(&wired), "x");
            let text = screen_text(&mut app, w, h);
            assert!(text.contains("Icewire rover: Homer"), "{text}");
            app.design = crate::design::Design::from_bot("icewire bot 1\nname Dodger\nrun python3 dodger.py\n", "x", None).unwrap();
            let text = screen_text(&mut app, w, h);
            assert!(text.contains("python3 dodger.py"), "{text}");
            app.design = wired;
            for ch in 0..challenge::ALL.len() {
                app.ch = ch;
                app.run();
                for _ in 0..90 {
                    app.tick();
                }
                let text = screen_text(&mut app, w, h);
                assert!(text.contains(challenge::ALL[ch].name), "{text}");
                app.arena_view = false;
                let text = screen_text(&mut app, w, h);
                assert!(text.contains("LIVE"), "{text}");
                app.arena_view = true;
            }
        }
    }
}
