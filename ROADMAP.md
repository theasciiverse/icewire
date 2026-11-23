# Icewire roadmap

Plan of 2026-11-23 (job 6 in `ASSIGNMENTS-2026-11-23.md`, design notes in
`CLAUDE.md`). Our own game: Rust + Ratatui, Braille for the arena, MIT.
Versions are 0.x.0 while unfinished; 1.0.0 waits on David playing it.
Every version ends with a report saying what to try first.

## 0.1.0: the bench and the arena

- [ ] **Chassis.** One rover, the Mule: eight sensor slots on four mounts
      (nose, left, right, tail), one left track, one right track, one gyro,
      and room for thirty logic parts.
- [ ] **Parts.** Sensors: bumper, whisker (proximity), ranger (forward
      range finder), compass (heading, or bearing and distance to the
      challenge's beacon). Logic: AND, OR, NOT, compare, timer, toggle,
      number. Drive: left track, right track, gyro (steering).
      Signals are numbers; zero is off, anything else is on.
- [ ] **The wiring bench.** Parts as boxes in box-drawing characters,
      wires routed around them in line characters, every live signal
      coloured while the rover runs. Fully keyboard driven: add, move,
      connect, delete, set a value, undo. Mouse as a bonus: click, drag,
      wire, right-click to remove.
- [ ] **The arena.** Top-down, Braille rovers, sensor rays and trails,
      deterministic fixed-step physics (60 ticks a second, our own sine and
      arctangent so every platform gets the same run). Pause, single step,
      fast forward, and Tab to flip to the live bench while it runs.
- [ ] **Three challenges.** Ring Hall Run (checkpoints round a habitat
      corridor), the Cold Stores (a maze with one hatch out) and the Puck
      (push Tumbler out of the ring). Best results and stars kept.
- [ ] **Rovers as plain text** in `$XDG_STATE_HOME/icewire/rovers/`, one
      `.rover` file each, readable and hand-editable. A starter rover on
      first run.
- [ ] **Parts library** screen, help, session log, `--version`, `--demo`.
- [ ] **Packaging.** README, release workflow (Linux static, macOS,
      Windows), a 100x30 preview, hub entry once it is fun for ten minutes.

## 0.2.0: the league

- More parts: weapons with simple ballistics, a radar that finds the
  nearest rover, counters, memory.
- A battle challenge.
- Computer-wired opponents of rising skill.
- The Europa story as a short campaign through the station's hallways,
  with our own characters.

## 0.3.0: Lua

- Every wiring compiles to readable Lua the player can open, edit and run
  instead; a rover can be pure Lua.
- Sandboxed (no io, no os), with a per-tick instruction budget.

## 0.4.0: the open bot port

- Any program in any language drives a rover: Icewire writes sensor lines
  to its stdin each tick and reads actuator lines from its stdout.
- A versioned, documented line protocol (a header line names the version).
- Simultaneous resolution per tick (already how the arena works).
- `icewire --tournament` runs bot against bot headless and saves replays.
- Two example bots in `examples/`: a shell script and a Python program.

## 1.0.0

Waits on David playing it.
