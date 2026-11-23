# Icewire roadmap

Plan of 2026-11-23 (job 6 in `ASSIGNMENTS-2026-11-23.md`, design notes in
`CLAUDE.md`). Our own game: Rust + Ratatui, Braille for the arena, MIT.
Versions are 0.x.0 while unfinished; 1.0.0 waits on David playing it.
Every version ends with a report saying what to try first.

## 0.1.0: the bench and the arena (built 2026-11-23)

- [x] **Chassis.** One rover, the Mule: eight sensor slots on four mounts
      (nose, left, right, tail), one left track, one right track, one gyro,
      and room for thirty logic parts.
- [x] **Parts.** Sensors: bumper, whisker (proximity), ranger (forward
      range finder), compass (heading, or bearing and distance to the
      challenge's beacon). Logic: AND, OR, NOT, compare, timer, toggle,
      number. Drive: left track and right track (fwd and back inputs),
      gyro (right and left inputs). Signals are numbers; zero is off,
      anything else is on; AND and OR pass values through, so they gate.
- [x] **The wiring bench.** Parts as boxes in box-drawing characters,
      wires routed around them in line characters, every live signal
      coloured while the rover runs. Fully keyboard driven: add, move,
      connect, delete, set a value, undo. Mouse as a bonus: click, drag,
      wire, right-click to remove.
- [x] **The arena.** Top-down, Braille rovers, sensor rays and trails,
      deterministic fixed-step physics (60 ticks a second, our own sine and
      arctangent so every platform gets the same run). Pause, single step,
      fast forward, and Tab to flip to the live bench while it runs.
- [x] **Three challenges.** Ring Hall Run (checkpoints round a habitat
      corridor), the Cold Stores (a maze with one hatch out) and the Puck
      (push Tumbler out of the ring). Best results and stars kept.
- [x] **Rovers as plain text** in `$XDG_STATE_HOME/icewire/rovers/`, one
      `.rover` file each, readable and hand-editable. A starter rover on
      first run.
- [x] **Parts library** screen, help, session log, `--version`, `--demo`.
- [x] **Packaging.** README, release workflow (Linux static, macOS,
      Windows), a 100x30 preview, hub entry once it is fun for ten minutes.

## 0.2.0: the league (built 2026-11-23)

- [x] **More parts.** Pellet gun (straight shots with flight time, stopped
      by walls) and lobber (arcs over walls to a range you wire in, splash
      on landing); radar (bearing and distance to the nearest rover, through
      walls); gauge (own charge); counter; memory.
- [x] **Battles.** 100 charge each, pellets 5, slush 15, stall at 0;
      stars from the charge left. Three arenas: the Boiler Room, Laundry
      Night and the Dome.
- [x] **Computer-wired opponents of rising skill.** Sprocket (wanders,
      fires when aimed), Kettle (keeps its distance, fires, lobs), Nightjar
      (presses in when strong, backs off when weak, lobs and fires). Pinned
      by tests: a plain gunner beats only Sprocket, a gunner with a lobber
      beats Kettle but not Nightjar, and a tuned fighter beats all three
      with three stars.
- [x] **The league.** A first season on Conamara Station, seven nights
      through the station's halls, each a scene before and after its
      challenge, with Ines, Hana, Oskar, Sami, Ruben and the director. A
      second race, the Long Window, under Jupiter.

## 0.3.0: Lua (built 2026-11-23)

- [x] **Wiring compiles to Lua.** L on the bench turns any wiring into a
      readable script, one line or two per part in the order the wiring
      runs, as a new rover beside the wired one. Tests prove the compiled
      script drives every built-in rover bit for bit the same as its
      wiring.
- [x] **Open, edit and run.** A Lua rover's bench shows its script with
      line numbers and colour; e opens it in the player's editor, r runs
      it, print() shows in the live view next to any error.
- [x] **Pure Lua rovers.** `.lua` files in the rovers folder are rovers;
      l in the garage starts one from a short example. The script calls
      bumper, whisker, ranger, compass, radar, gauge and time, and sets
      tracks, gyro, fire and lob.
- [x] **Sandboxed.** Lua 5.4 built into the binary with only math, string,
      table and utf8; no io, os, package, debug, load or dofile; 16 MB of
      memory; math.random seeded the same every run; 50000 instructions a
      tick, and a tick over budget drives nothing while the run goes on.

## 0.4.0: the open bot port (built 2026-11-23)

- [x] **Any program drives a rover.** Icewire writes each tick's readings
      to the program's standard input and reads commands from its standard
      output; standard error shows beside the arena. Bot rovers are `.bot`
      files naming a command; B in the garage makes one.
- [x] **A versioned, documented line protocol.** `PROTOCOL.md`: a hello
      both ways carrying `icewire-bot 1`, then a block of readings and a
      block of commands each tick, every block ending with `end`.
- [x] **Simultaneous resolution.** Every bot hears a tick before any
      answer is read, under one shared deadline, and all rovers move
      together.
- [x] **Safe with bad bots.** 100 ms a tick (`--timeout` in tournaments),
      late answers skipped, out after 30 silent ticks, when its output ends
      or when it stops reading; lines capped and queues bounded; each bot
      in its own process group, killed and reaped with everything it
      started. Tests cover silent, quitting, flooding, babbling and
      wrong-version bots and check nothing is left running.
- [x] **Tournaments and replays.** `icewire --tournament ROVER...` plays
      every pair twice, from both sides, headless, with standings; each
      match is saved as a text replay of states, and `icewire --replay
      FILE` watches one in the arena view.
- [x] **Two example bots** in `examples/`: Shellby (POSIX shell, builtins
      only) and Dodger (Python 3, beats Sprocket and Kettle).

## 0.5.0: watch and learn (built 2026-11-23)

- [x] **A lesson in the game.** *Watch and learn* on the title screen (or
  `--learn`): Ines wires First Light on the real bench, every step a real
  key press at a pace a person can follow, and runs it round Ring Hall
  three times: a number and a compass for one star, full power for two,
  a distance compass, a compare and an OR to ease off at the checkpoints
  for three. A caption says what each step does and why and shows the keys
  as they are pressed; Space pauses, the arrows step forward and back
  (going back rebuilds the bench exactly), Esc leaves and puts your own
  rover back, and at the end Enter keeps First Light in the garage.
  Nothing is saved while it plays. A test plays the whole lesson and
  checks every lap time and star the captions promise.

- [x] **Part boxes by category.** Sensors rounded, logic double, drive
  heavy, weapons heavy top and bottom: parts tell apart by shape as well
  as colour, and no box shares the wires' thin line.
- [x] **Arrange (A).** The layered layout: sources left, each logic part
  one column right of what feeds it (loops do not push), drive and
  weapons in the last column in a fixed order; then barycentre sweeps and
  neighbour swaps to cross as few wires as it can, counting crossings
  geometrically with each wire ending on its input's row. Long chains wrap
  into bands; a rover too big to arrange is left as it is. u undoes it.

## 1.0.0

Waits on David playing it.
