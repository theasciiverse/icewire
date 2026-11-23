# The Icewire bot protocol, version 1

Any program, in any language, can drive an Icewire rover. Icewire starts
the program, writes the rover's readings to its standard input as plain
text lines, sixty times a second, and reads commands back from its
standard output. If your program can read a line and print a line, it can
enter the league.

Two example bots come with Icewire, in `examples/`: `shellby.sh` (POSIX
shell, builtins only) and `dodger.py` (Python 3).

## Starting a bot

A bot rover is a `.bot` file in the rovers folder
(`$XDG_STATE_HOME/icewire/rovers/`):

```
icewire bot 1
name Dodger
run python3 dodger.py
```

The `run` line is the command, split on spaces; put a path with spaces in
"double quotes". It runs in the folder the `.bot` file is in. **B** in the
garage makes one for the Dodger example.

Bots also take part in tournaments straight from the command line:

```
icewire --tournament "python3 examples/dodger.py" "sh examples/shellby.sh" kettle
```

## Lines

Every message is ASCII text, one item per line, ending with a line
holding only `end`. Numbers are decimal, with a `.` and no exponent
(`-12.5`, `3`, `0.021`). Words are separated by single spaces. Lines are
at most 1024 characters; Icewire cuts longer ones.

## Hello

Icewire speaks first:

```
icewire-bot 1
rover Dodger
challenge boiler-room
end
```

The first line names the protocol and its version. `rover` is the rover's
name (spaces become `_`), `challenge` the challenge's id: `ring-hall`,
`cold-stores`, `puck`, `boiler-room`, `long-window`, `laundry` or `dome`.
Later versions may add lines before `end`; skip any you do not know.

The bot answers with one line, the version it speaks:

```
icewire-bot 1
```

It has five seconds. A bot that answers with another version is not
started, and the player is told which version it spoke.

## Every tick: readings

Sixty times a second Icewire writes the rover's readings:

```
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
```

| Line | Meaning |
|---|---|
| `tick N` | the tick number, from 0 |
| `time S` | seconds since the start |
| `ranger SIDE M ...` | metres from the hull to the nearest wall, rover or edge, looking out from the nose, right, tail and left; at most 8 |
| `bumper SIDE B ...` | 1 while that quarter of the rover touches something, else 0 |
| `compass heading H bearing B distance D` | heading in degrees, 0 up the map, clockwise to 360; bearing to the challenge's beacon, -180 to 180, positive to the right of the nose; distance to the beacon in metres |
| `radar bearing B distance D` | the same for the nearest other rover still in play; distance 99 and bearing 0 when there is none |
| `charge C` | the rover's own charge, 0 to 100 (it matters in battles) |
| `loaded pellet P lobber L` | 1 when that weapon is ready to fire |
| `speed V` | metres a second |

The beacon is the next checkpoint in a race, the hatch in the Cold
Stores, and the other rover in the Puck and in battles. Later versions
may add lines; skip any you do not know.

## Every tick: commands

The bot answers with any of these lines, then `end`:

| Line | Meaning |
|---|---|
| `tracks L R` | left and right track, each -1 (full back) to 1 (full ahead) |
| `gyro S` | spin, -1 to 1; positive turns right |
| `fire` | fire the pellet gun, if it is loaded |
| `lob M` | lob slush M metres ahead (1.5 to 8), if the lobber is loaded |
| `say ...` | ignored; handy for leaving notes in a log |

Anything not given counts as 0 (or not fired) for that tick, so an empty
answer (just `end`) stops the rover. Values outside their range are
clamped. A line Icewire does not understand is ignored, and the player is
shown it once.

Everything the bot writes to **standard error** is shown beside the arena
during a run: print your debugging there.

## Time

A bot has 100 milliseconds to answer each tick (a tournament can set
another limit with `--timeout`). Icewire sends every bot its readings
before reading any answer, and moves every rover together after, so no
bot gains by being quicker than another.

A bot that misses a tick does nothing that tick, and its late answer is
skipped when it comes, so it never acts on old readings. A bot that
misses 30 ticks in a row, ends, or stops reading is out for the rest of
the run: its rover stands still. When the run ends, Icewire stops the bot
and anything the bot started (on Windows, the bot itself).

## Versions

This is version 1. A later version will change the number on the first
line, and Icewire will keep speaking version 1 to bots that answer with
it.
