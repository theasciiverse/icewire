//! The challenges of the night league at Conamara Station: races, a
//! maze, a shoving match and battles.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Goal {
    /// Every checkpoint in order.
    Race,
    /// Reach the exit hatch.
    Maze,
    /// Push the other rover out of the ring.
    Sumo,
    /// Stall the other rover before it stalls you.
    Battle,
}

pub struct Challenge {
    pub id: &'static str,
    pub name: &'static str,
    /// Who runs it and what it is, in a line.
    pub tagline: &'static str,
    pub brief: &'static str,
    pub goal: Goal,
    pub map: &'static str,
    /// Seconds before the clock runs out.
    pub limit: f64,
    /// Seconds to beat for two and three stars; in a battle, the charge
    /// to have left instead.
    pub par: [f64; 2],
    /// Starting headings in degrees, player then opponent.
    pub headings: [f64; 2],
    /// The ring's radius in metres, for the Puck.
    pub ring: Option<f64>,
    /// The other rover, as rover-file text.
    pub opponent: Option<&'static str>,
    /// What the compass beacon is here.
    pub beacon: &'static str,
}

pub const ALL: [Challenge; 7] = [
    Challenge {
        id: "ring-hall",
        name: "Ring Hall Run",
        tagline: "One lap of the lower deck, eight checkpoints",
        brief: "Ring Hall runs right round Conamara Station's lower deck, \
past the laundry, the pump columns and the long window where Jupiter \
hangs over the ice. After the night shift it is empty, and Hana from \
hydroponics keeps the stopwatch. Eight checkpoints, in order, one lap. \
The compass bearing always points at the next one.",
        goal: Goal::Race,
        map: include_str!("../challenges/ring-hall.txt"),
        limit: 120.0,
        par: [50.0, 39.0],
        headings: [90.0, 0.0],
        ring: None,
        opponent: None,
        beacon: "the next checkpoint",
    },
    Challenge {
        id: "cold-stores",
        name: "The Cold Stores",
        tagline: "Aisles of seed racks and one hatch out",
        brief: "Racks of frozen seed stock, aisles that all look alike, \
frost on every rail. Somewhere at the far corner is the hatch to the \
loading bay. Your compass knows exactly where it is. It knows nothing \
about the shelves in between. Oskar swears his rover found it in under \
three minutes, and the old hands only say: keep one side to the racks \
and never let go.",
        goal: Goal::Maze,
        map: include_str!("../challenges/cold-stores.txt"),
        limit: 300.0,
        par: [210.0, 175.0],
        headings: [90.0, 0.0],
        ring: None,
        opponent: None,
        beacon: "the hatch",
    },
    Challenge {
        id: "puck",
        name: "The Puck",
        tagline: "A ring on the pump room floor, and Tumbler",
        brief: "A circle scraped into the pump room floor and a bucket of \
meltwater to make it slick. In the other corner waits Tumbler, Sami's \
rover, which has never been pushed out by a newcomer. Sami runs its \
tracks at eighty-five percent and says nobody needs more. Push it over the \
edge of the ring. Stay inside yourself. The compass points at Tumbler, \
and whiskers and rangers see the edge.",
        goal: Goal::Sumo,
        map: include_str!("../challenges/puck.txt"),
        limit: 60.0,
        par: [35.0, 21.0],
        headings: [90.0, 270.0],
        ring: Some(5.0),
        opponent: Some(crate::design::TUMBLER),
        beacon: "Tumbler",
    },
    Challenge {
        id: "boiler-room",
        name: "Sparks in the Boiler Room",
        tagline: "First battle: Ruben's Sprocket among the pipes",
        brief: "The boiler room is warm, loud and full of pipes, which is \
why the league fights there: nobody upstairs hears a thing. Ruben from \
comms built Sprocket out of spare parts and a pellet gun meant for \
knocking frost off the antenna. Every rover starts with 100 charge. A \
pellet takes 8, a slush ball 20, and a rover at 0 stalls where it \
stands. Stall Sprocket before it stalls you. Radar finds it through the \
pipes; the compass points at it too.",
        goal: Goal::Battle,
        map: include_str!("../challenges/boiler-room.txt"),
        limit: 90.0,
        par: [50.0, 90.0],
        headings: [90.0, 270.0],
        ring: None,
        opponent: Some(crate::design::SPROCKET),
        beacon: "Sprocket",
    },
    Challenge {
        id: "long-window",
        name: "The Long Window",
        tagline: "Six checkpoints under Jupiter, one hairpin",
        brief: "The observation corridor on the upper deck, where Jupiter \
fills half the sky and a sign says NO RUNNING. Hana's second course: \
the whole length of the window, down past the old airlock, a hairpin \
round the stores lift and home along the lower hall. Six checkpoints. \
Hana says it is the fastest lap on Europa if you dare.",
        goal: Goal::Race,
        map: include_str!("../challenges/long-window.txt"),
        limit: 120.0,
        par: [50.0, 37.5],
        headings: [90.0, 0.0],
        ring: None,
        opponent: None,
        beacon: "the next checkpoint",
    },
    Challenge {
        id: "laundry",
        name: "Laundry Night",
        tagline: "Kettle, among the washers and the dryers",
        brief: "Thursday is laundry night, and the drum of the dryers \
covers anything. Ines, who runs the league and keeps its results in a \
paper notebook, fields Kettle: patient, steady, and never in a hurry to \
get close. It keeps its distance, turns to face you and keeps firing. \
Somebody once beat it by going round the machines.",
        goal: Goal::Battle,
        map: include_str!("../challenges/laundry.txt"),
        limit: 90.0,
        par: [30.0, 55.0],
        headings: [135.0, 315.0],
        ring: None,
        opponent: Some(crate::design::KETTLE),
        beacon: "Kettle",
    },
    Challenge {
        id: "dome",
        name: "Lights Out in the Dome",
        tagline: "The last match, under the stars",
        brief: "The observation dome, after the lights go out at midnight, \
with nothing overhead but Jupiter and the stars. Waiting in the far \
side is Nightjar, a rover nobody has seen win or lose, wired by \
somebody nobody will name. It lobs from range, closes in when you are \
weak and backs off when it is. This is the match the whole league comes \
to watch.",
        goal: Goal::Battle,
        map: include_str!("../challenges/dome.txt"),
        limit: 120.0,
        par: [10.0, 20.0],
        headings: [90.0, 270.0],
        ring: None,
        opponent: Some(crate::design::NIGHTJAR),
        beacon: "Nightjar",
    },
];

pub fn by_id(id: &str) -> Option<usize> {
    ALL.iter().position(|c| c.id == id)
}

impl Challenge {
    /// Stars for a win: one for winning, more for beating par (a time,
    /// or in a battle the charge left).
    pub fn stars(&self, secs: f64, charge: f64) -> u8 {
        if self.goal == Goal::Battle {
            1 + (charge >= self.par[0]) as u8 + (charge >= self.par[1]) as u8
        } else {
            1 + (secs <= self.par[0]) as u8 + (secs <= self.par[1]) as u8
        }
    }

    /// What the second and third stars ask for, in words.
    pub fn par_text(&self) -> [String; 2] {
        if self.goal == Goal::Battle {
            [format!("with {} charge left", self.par[0]), format!("with {} charge left", self.par[1])]
        } else {
            [format!("under {} s", self.par[0]), format!("under {} s", self.par[1])]
        }
    }
}
