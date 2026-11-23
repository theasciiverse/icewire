//! The league: a first season on Conamara Station, seven nights, each a
//! scene, a challenge and what came after. Progress is one number in
//! `$XDG_STATE_HOME/icewire/league.txt`: how many nights are won.

use crate::store::state_dir;

pub struct Chapter {
    pub title: &'static str,
    pub challenge: &'static str,
    pub before: &'static str,
    pub after: &'static str,
}

pub const CHAPTERS: [Chapter; 7] = [
    Chapter {
        title: "First Night",
        challenge: "ring-hall",
        before: "Your first week on Conamara Station, and you still can't sleep through the hum of the heaters. At two in the morning you follow a strange whirring down to Ring Hall and find a dozen people in dressing gowns standing round a work rover.

A woman with a paper notebook looks you up and down. \"Ines,\" she says. \"Night league. You're the new glaciologist? Good. You know about ice.\"

She pushes a rover into your hands. Somebody has painted HOMER on its side. \"One lap, eight checkpoints. Hana has the stopwatch. Don't hit the laundry.\"",
        after: "Hana clicks the stopwatch and shows it to Ines, who writes something in the notebook without a word. Then she turns the page round to face you: your name, in pencil, at the bottom of a long list.

\"Tomorrow night,\" she says. \"Oskar has been boasting again.\"",
    },
    Chapter {
        title: "Seed Stock",
        challenge: "cold-stores",
        before: "Oskar runs the stores and talks the way the heaters hum: all the time, and mostly to himself. \"Under three minutes,\" he tells you, for the fourth time, leading you into the cold.

Rows of seed racks vanish into the frost, every aisle the same. \"The hatch is at the far end. Your compass will tell you exactly where it is. That's the trap.\" He grins. \"Keep one side to the racks, the old hands say. I never listen to the old hands.\"",
        after: "The hatch clanks open and your rover rolls out into the loading bay, frosted white. Oskar checks his watch, then checks it again.

\"Mine was faster,\" he says. Nobody believes him, and he doesn't seem to mind at all.",
    },
    Chapter {
        title: "Meltwater",
        challenge: "puck",
        before: "Sami from the pump room has scraped a circle on the floor and is pouring a bucket of warm water over it. \"The Puck,\" Sami says, as the water turns to glass. \"Two rovers in. One rover out.\"

Tumbler sits on the far side, dented and proud. \"Tumbler has never been pushed out by a newcomer. Eighty-five percent on the tracks, every time. Nobody needs more.\"

Behind you, somebody laughs.",
        after: "Tumbler spins slowly on the slick ice beyond the line, and Sami picks it up like a cat that has fallen off a shelf. \"Fine,\" Sami says. \"Fine. Next time I'm trying ninety.\"

Ines writes in the notebook. \"Next time,\" she says, \"there's the boiler room.\"",
    },
    Chapter {
        title: "Sparks",
        challenge: "boiler-room",
        before: "Down in the boiler room the pipes knock and steam, and the league watches from the gantry. Ruben from comms is fitting a pellet gun to his rover, Sprocket. \"It's for clearing frost off the dish,\" he says, not very convincingly.

He talks about home while he works: his daughter's school play, the snow there, the forty minutes every message takes to cross the dark. \"Out here you need something to think about at night. This is mine.\"

Ines hands you a pellet gun and a lobber. \"Every rover starts with a hundred charge. Run out and you stall.\"",
        after: "Sprocket stalls by a boiler, its lights going dim. Ruben laughs and gathers it up.

\"She'd have loved that,\" he says, meaning his daughter, and goes off to record a message telling her all about it. Forty minutes there. Forty minutes back.",
    },
    Chapter {
        title: "The Long Window",
        challenge: "long-window",
        before: "Hana takes you up to the observation corridor on the upper deck, where a sign says NO RUNNING and Jupiter fills half the window, its great storm turning slowly.

\"My second course,\" she says. \"The whole length of the window, down past the old airlock, round the stores lift and home. Fastest lap on Europa.\" She looks out at the planet for a long moment. \"I grew tomatoes once, in a garden, with sky over it. Go on, then.\"",
        after: "Hana stops the watch and reads it twice. \"Hm,\" she says, which from Hana is a medal.

On the way back she shows you the tomatoes in hydroponics, red under the grow lamps, and gives you one to take to bed.",
    },
    Chapter {
        title: "Laundry Night",
        challenge: "laundry",
        before: "Thursday is laundry night, and under the drum of the dryers the whole league turns out. Ines rolls Kettle out from behind the folding table. It is the plainest rover you have seen: no paint, no name on the side, one dent.

\"Kettle doesn't hurry,\" Ines says. \"It keeps its distance, it faces you, and it keeps firing. It lobs when you're far. Most rovers beat themselves against it.\" She opens the notebook. \"Somebody once beat it by going round the machines.\"",
        after: "Kettle stalls between two washers. Ines looks at it for a long time, then at you, then writes a single line in the notebook and closes it.

\"There's one more match,\" she says quietly. \"In the dome, at midnight. Nobody says who wires the other rover.\"",
    },
    Chapter {
        title: "Lights Out",
        challenge: "dome",
        before: "At midnight the lights in the dome go out, and there is nothing overhead but Jupiter and the stars. The league sits round the edge in the dark.

On the far side waits Nightjar: matte black, no markings, wired by somebody nobody will name. It lobs from range, it presses in when you are weak, and it backs off when it is.

\"Whoever wins,\" Ines says, \"keeps the notebook for a year.\"",
        after: "Nightjar stalls under the stars. Somebody turns the lights up, and a tall woman in a station director's jacket steps out of the dark to pick it up. Director Okafor. The league goes very quiet.

\"Twelve years,\" she says, \"I've been waiting for somebody to beat it.\" She takes the notebook from Ines and writes your name at the top of a fresh page. \"Same time tomorrow.\"

Out in the window, Jupiter rises over the ice.",
    },
];

/// How many nights are won; the next one is open.
pub fn load() -> usize {
    state_dir().and_then(|d| std::fs::read_to_string(d.join("league.txt")).ok()).and_then(|t| t.trim().strip_prefix("won ").and_then(|n| n.trim().parse().ok())).unwrap_or(0).min(CHAPTERS.len())
}

pub fn save(won: usize) {
    if let Some(d) = state_dir() {
        let _ = std::fs::write(d.join("league.txt"), format!("won {won}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chapter_has_its_challenge() {
        for c in &CHAPTERS {
            assert!(crate::challenge::by_id(c.challenge).is_some(), "{}", c.challenge);
        }
        assert_eq!(load(), 0, "no state folder in tests");
    }
}
