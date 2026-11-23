# Icewire

**Nobody on Conamara Station is supposed to be racing the work rovers.
Everybody is.**

Under the ice of Europa, with Jupiter filling the long window on the upper
deck, the night shift has a league. You get a Mule: a squat ice-crawler
with two drive tracks, a gyro, eight sensor slots, two weapon mounts and a
logic board. You do not drive it. You wire it. Sensors feel the world, logic decides, the tracks
move, and then you let go and watch what you built try its best.

Every part is a box on the bench, drawn in line characters, and every wire
finds its own way round them. Press Tab while your rover runs and the bench
lights up: each wire glows with the signal running through it, so you can
see the moment the ranger's number drops under 3 and the compare flips on
and the tracks ease off for the corner. The arena is top-down in Braille,
with your rover's sensor beams and trail drawn as it slides across the ice.

## The night league

On your first night, Ines, who runs the league from a paper notebook,
pushes a rover into your hands. Seven nights later you are under the dome
at midnight, facing a matte black rover nobody will admit to wiring. In
between: Hana and her stopwatch, Oskar who is always faster, Sami and the
meltwater, and Ruben from comms, who fights in the boiler room and sends
his daughter forty-minute messages about it. Each night is a scene, a
challenge and what came after. Free play opens every challenge at once.

- **Ring Hall Run.** One lap of the lower deck, past the laundry and the
  pump columns, eight checkpoints in order. Homer, the rover waiting in
  your garage, can do it at a steady trot. Hana from hydroponics keeps the
  stopwatch and knows how much faster it can go. The ice is slippery:
  full power slides you into the walls.
- **The Cold Stores.** Aisles of frozen seed racks that all look alike,
  and one hatch out. Your compass knows exactly where the hatch is. It
  knows nothing about the shelves in between.
- **The Puck.** A circle scraped on the pump room floor, a bucket of
  meltwater to make it slick, and Tumbler, Sami's rover, which has never
  been pushed out by a newcomer.
- **Sparks in the Boiler Room.** The first battle, among the pipes,
  against Sprocket: it wanders, and it fires when you cross its nose.
- **The Long Window.** Hana's second course, the whole length of the
  window with Jupiter turning beyond it, and a hairpin round the stores
  lift.
- **Laundry Night.** Kettle keeps its distance, faces you and keeps
  firing. It lobs when you are far.
- **Lights Out in the Dome.** Nightjar presses in when you are weak and
  backs away when it is.

In a battle every rover starts with 100 charge; a pellet takes 5, a ball
of slush 15, and a rover at 0 stalls where it stands. Each challenge gives
a star for finishing or winning and two more for beating the times on the
wall, or in a battle for the charge you still have. Every star is in
reach with the parts you have; the third ones take some thought.

## The parts

- **Sensors:** a bumper that feels a touch, a whisker that feels what is
  near, a ranger that measures how far it is to the first thing ahead, a
  compass that knows your heading, or the bearing and distance to the
  challenge's beacon, a radar that finds the nearest rover through walls,
  and a gauge that reads your own charge.
- **Logic:** AND, OR, NOT, compare, timer, toggle, counter, memory and a
  number. Signals are numbers, 0 is off and anything else is on. AND
  passes its second input along while the first is on, so it doubles as a
  gate.
- **Drive:** left track, right track (each with forward and back inputs)
  and the gyro (right and left).
- **Weapons:** a pellet gun that fires frozen pellets straight off the
  nose, and a lobber that throws slush over walls to whatever range you
  wire into it.

The parts library (P on the bench) has the whole story of each.

## Playing

From the title, *The league* starts the season: read the night's scene,
press Enter for the bench, R to run, Tab to watch the wiring live, Esc to
stop and rewire. *Wire a rover* goes straight to the bench, and *Free
play* opens every challenge.

| Key | On the bench |
|---|---|
| arrows, hjkl | move the cursor (Shift: faster) |
| Tab, Shift-Tab | jump from part to part |
| a, Enter | add a part |
| Enter, e | set a part's mount, mode or value (arrows, or type a number) |
| w | wire from the part under the cursor, then Enter on the part it feeds |
| m | move a part |
| x, Delete | remove the part or wire under the cursor |
| u | undo |
| r, F5 | run |
| c | next challenge |
| L | compile the wiring to a Lua rover |
| n, g, p, ? | rename, garage, parts library, help |

The mouse works too: drag a part, click its output to start a wire,
right-click to remove.

| Key | In the arena |
|---|---|
| Space | pause |
| . | one tick at a time |
| f, - | faster, slower (up to eight times) |
| Tab | the live bench |
| r | run again |
| Esc | stop and rewire |

## When the wiring grows up: Lua

Press **L** on any wired rover and Icewire writes its wiring out as Lua: a
new rover beside the old one, a line or two per part, in the order the
wiring runs, driving exactly as the wiring did. Read it, then make it
yours. **e** opens the script in your editor (`$VISUAL` or `$EDITOR`, else
nano, vi or Notepad); save, close, and you are back on the bench with the
script checked. **r** runs it, and **Tab** during the run shows the script
beside whatever it `print`s.

Or start from nothing: **l** in the garage makes a rover in plain Lua.

```lua
-- Icewire rover: Scribbler
function tick()
  -- turn toward the beacon, ease off when a wall is close
  gyro(clamp(compass("bearing") / 30))
  local speed = clamp(ranger("nose") / 3, 0.3, 1.0)
  tracks(speed, speed)
end
```

Icewire calls `tick()` sixty times a second. A script reads
`bumper(side)`, `whisker(side, reach)`, `ranger(side)`,
`compass("heading" | "bearing" | "distance")`, `radar("bearing" |
"distance")`, `gauge()` and `time()`, and sets `tracks(left, right)` and
`gyro(spin)` (each -1 to 1), `fire()` and `lob(metres)`; `loaded("pellet"
| "lobber")` says what is ready. Scripts run sandboxed in Lua 5.4: math,
string, table and utf8, nothing that touches files or the system, and at
most 50,000 instructions a tick. A tick over budget drives nothing, and
the run goes on.

## Rovers are text

Every rover is a plain text file in `$XDG_STATE_HOME/icewire/rovers/`
(`~/.local/state/icewire/rovers/` on most systems), saved as you work: a
`.rover` file for a wired rover, a `.lua` file for a Lua one.
Copy one to a friend, or open it in an editor:

```
icewire rover 1
name Homer
part 1 compass at 4 3 mode bearing
part 2 number at 4 10 value 1
part 3 track-left at 34 2 value 60
part 4 track-right at 34 8 value 60
part 5 gyro at 34 14 value 2
wire 2 to 3 fwd
wire 2 to 4 fwd
wire 1 to 5 right
```

The same folder holds `records.txt` (your best results), `league.txt`
(how far into the season you are), `settings.txt` and `session.log`. The arena runs at a fixed 60 ticks a second with its
own arithmetic, so the same rover always does exactly the same run, on
any machine.

## Options

```
icewire                      the title screen
icewire --challenge puck     straight to the bench with a challenge picked
icewire --demo               a short tour; nothing is saved
```

## Building

Rust 2024 (1.85 or newer) and a C compiler, for the Lua that is built
into the binary:

```
cargo build --release
cargo test
```

The session log level comes from `ICEWIRE_LOG` (default `info`;
`debug` logs every key and a snapshot each second).

## Licence

MIT. Made for Asciiverse by David Sandquist, 2026, with Ratatui.
