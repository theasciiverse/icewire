//! The three challenges of the night league at Conamara Station.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Goal {
    /// Every checkpoint in order.
    Race,
    /// Reach the exit hatch.
    Maze,
    /// Push the other rover out of the ring.
    Sumo,
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
    /// Seconds to beat for two and three stars.
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

pub const ALL: [Challenge; 3] = [
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
two minutes.",
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
rover, which has never been pushed out by a newcomer. Push it over the \
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
];

pub fn by_id(id: &str) -> Option<usize> {
    ALL.iter().position(|c| c.id == id)
}

impl Challenge {
    /// Stars for a winning time: one for finishing, more for beating par.
    pub fn stars(&self, secs: f64) -> u8 {
        1 + (secs <= self.par[0]) as u8 + (secs <= self.par[1]) as u8
    }
}
