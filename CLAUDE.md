# Icewire

Asciiverse robot programming game. Rust + Ratatui + Drawille.
Spiritual successor to MindRover: The Europa Project (CogniToy, 1999).
Originally ported to Linux by Loki Software.

**Naming note:** Previously "RoverMind (working title)." Renamed to Icewire —
one word doing double duty across both halves of the concept: the Europa
ice-moon setting, and the visual wiring/circuit interface at the heart of the
gameplay. The original title MindRover is the ideal but belongs to CogniToy;
Icewire sidesteps that entirely rather than reworking a near-collision.

## The original game

MindRover: The Europa Project, CogniToy, 1999.
- Assemble robots from stock components (sensors, weapons, thrusters, engines)
- Program robot behaviour via a visual wiring interface — node graph style,
  more like circuit construction than traditional programming
- Compete in races, battles, and maze challenges
- Set on Europa, a moon of Jupiter — researchers hacking work rovers for
  unofficial underground robot combat leagues
- Add-on allowed control of real Lego Mindstorms robots via same interface
- Ported to Linux by Loki Software, to Mac by MacPlay

Personal connection: David played this extensively on Linux via Loki.

## Why a TUI version is compelling

The core mechanic — visual wiring of robot behaviour — is actually more legible
in a terminal than it sounds.

The wiring interface is a node graph. Node graphs render beautifully in
box-drawing characters: each component a labelled box, connections as
line-drawing characters between them, signal flow visible as directional arrows.
The terminal is arguably a more honest medium for this kind of logical wiring
than a 3D graphical interface — the abstraction is cleaner, the logic more visible.

The arena — top-down view, robots competing — is Braille sub-pixel rendering
for the robots themselves, physics simulation underneath, Ratatui for the
competition HUD and event feed.

## The Europa setting

Scientists on an ice moon of Jupiter, reprogramming work rovers for underground
illegal robot combat leagues in the habitat hallways. Jupiter constantly on the
horizon. Homesickness and obsession in equal measure.

That is a story, not just a game premise. The setting deserves full development —
the social world of the Europa research station, the different factions of
rover enthusiasts, the escalating stakes of the underground league. ASCII cinema
companion pieces are possible here.

## Rendering approach

**Wiring interface:**
- Box-drawing characters for component boxes — clean, readable, functional
- Line-drawing characters for connections between components
- Signal state visualised as colour on active connections (true colour terminals)
- Component properties as inline text fields
- The whole interface navigable by keyboard — no mouse required, though mouse
  support (as in Rusty Pinball) would be valuable here

**Arena:**
- Top-down view, Braille sub-pixel rendering for rover sprites and movement
- Physics simulation for movement, collisions, weapon impacts
- Braille particle effects for weapon fire, impacts, explosions
- Ratatui for competition HUD — health bars, score, event feed, timer

**Component library screen:**
- Ratatui grid layout, category browsing, component statistics
- Clean information display — this is where the terminal aesthetic shines

## The Lua connection

The wiring interface IS a visual programming language. In the Asciiverse TUI
version the wiring interface could optionally expose the underlying logic as
Lua — advanced players graduate from visual wiring to scripting the same
behaviour directly. The visual wiring generates Lua that can be inspected,
modified, and shared. This makes Icewire one of the most natural entry
points into the Asciiverse Lua scripting layer.

A player who started with visual wiring and graduated to Lua scripting has
learned to program without it ever feeling like a programming tutorial.
That is genuinely valuable and genuinely rare in game design.

This could also become a genuinely strong educational programming tool in its
own right — most programming-education games are either too abstract (moving
blocks on grids that don't feel like real programming) or too steep (bare text
editor, figure it out). Icewire's wiring interface is real logic — sensors
feeding conditionals feeding actuators, signal flow, state management — but
it feels like building a machine rather than writing code. Understanding
arrives through tinkering, not instruction.

## Reference: TTRTS, the open bot port

TTRTS, Marc Di Luzio's Tiny Terminal RTS (C++, 2014 to 2015,
github.com/mdiluz/ttrts, archived; sample players in
github.com/mdiluz/ttrts-players, Perl and Lua), is a small terminal game
played entirely by programs. Each turn the game writes the state as a plain
text file (`Turn_N.txt`: map size, walls, then one line per unit with
owner, facing and position) and waits for one orders file per player
(`Player_P_Turn_N.txt`: one `ORDER:<char> id:<unit>` line per unit, then
`END`). All orders resolve at once, units leave walls behind as they move,
and v0.4.0 carries the same text over TCP (port 11715, a one-line
handshake, then state out and orders back) so a bot can be a network
client. Its README sums it up: any user, program or cat that can read and
write text files can play.

What Icewire takes from it, as ideas only:

- **An open bot port next to the wiring.** Besides wired rovers and Lua,
  a rover can be driven by any outside program: Icewire writes the rover's
  sensor readings as plain text lines each tick and reads actuator
  commands back, over stdin/stdout of a child process (the default) or a
  local socket. Any language can enter a rover in the league, which is
  the TTRTS idea in Icewire's world.
- **A documented, versioned, line-based protocol**, with a header line
  that names the protocol version, so old bots keep working.
- **Simultaneous resolution** for tournament mode: every rover's commands
  for a tick are applied together, so no bot gains by being faster.
- **Bot-vs-bot tournaments** run headless (`icewire --tournament`), with
  replays saved, as the natural endgame for players who outgrow the
  wiring.

Rights: TTRTS has no licence on any branch (checked 2026-11-23), so none
of its code, text or maps may be used. A licence request to Marc is
drafted in `../ttrts/licence-request.md`; if he grants one, TTRTS can also
be packaged as a catalog title of its own. Icewire needs nothing from it
but the idea.

## Rights situation

**CogniToy LLC** — original developer, owns the IP. Appears dormant;
cognitoy.com is gone. Research whether the company still exists legally as a
first step. If dissolved, the IP situation is complex but potentially
resolvable — worth a thorough search for the original founders before
anything else.

(Cross-reference: this contact is also tracked in the Asciiverse outreach
CLAUDE.md — research needed on original founders' current whereabouts and
other projects since 2000, LinkedIn most likely path. David's Loki-era
personal history with the game is worth mentioning in any outreach.)

## Status

Design/ideation stage only. No code yet.
Conceived 2026-07-02, scout camp. Renamed from RoverMind to Icewire, 2026-07-08.

2026-11-23: assigned to rover as an original game (job 6 in
`ASSIGNMENTS-2026-11-23.md`), with the TTRTS reference above added.
Icewire uses no MindRover name, text, art, component names or levels, so
it does not wait on the CogniToy rights research; that research matters
only if we ever want the MindRover name or content itself.
