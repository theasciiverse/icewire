# Icewire

**Nobody on Conamara Station is supposed to be racing the work rovers.
Everybody is.**

Under the ice of Europa, with Jupiter filling the long window in Ring Hall,
the night shift has a league. You get a Mule: a squat ice-crawler with two
drive tracks, a gyro, eight sensor slots and a logic board. You do not
drive it. You wire it. Sensors feel the world, logic decides, the tracks
move, and then you let go and watch what you built try its best.

Every part is a box on the bench, drawn in line characters, and every wire
finds its own way round them. Press Tab while your rover runs and the bench
lights up: each wire glows with the signal running through it, so you can
see the moment the ranger's number drops under 3 and the compare flips on
and the tracks ease off for the corner. The arena is top-down in Braille,
with your rover's sensor beams and trail drawn as it slides across the ice.

## The night league

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

Each one gives a star for finishing and two more for beating the times on
the wall. Every star is in reach with the parts you have; the third ones
take a little thought.

## The parts

- **Sensors:** a bumper that feels a touch, a whisker that feels what is
  near, a ranger that measures how far it is to the first thing ahead, and
  a compass that knows your heading, or the bearing and distance to the
  challenge's beacon.
- **Logic:** AND, OR, NOT, compare, timer, toggle and a number. Signals
  are numbers, 0 is off and anything else is on. AND passes its second
  input along while the first is on, so it doubles as a gate.
- **Drive:** left track, right track (each with forward and back inputs)
  and the gyro (right and left).

The parts library (P on the bench) has the whole story of each.

## Playing

From the title, *Wire a rover* puts Homer on the bench. Press R to run him
in Ring Hall, Tab to watch his wiring live, Esc to stop and rewire.

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

## Rovers are text

Every rover is a plain text file in `$XDG_STATE_HOME/icewire/rovers/`
(`~/.local/state/icewire/rovers/` on most systems), saved as you work.
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

The same folder holds `records.txt` (your best times), `settings.txt`
and `session.log`. The arena runs at a fixed 60 ticks a second with its
own arithmetic, so the same rover always does exactly the same run, on
any machine.

## Options

```
icewire                      the title screen
icewire --challenge puck     straight to the bench with a challenge picked
icewire --demo               a short tour; nothing is saved
```

## Building

Rust 2024 (1.85 or newer):

```
cargo build --release
cargo test
```

The session log level comes from `ICEWIRE_LOG` (default `info`;
`debug` logs every key and a snapshot each second).

## Licence

MIT. Made for Asciiverse by David Sandquist, 2026, with Ratatui.
