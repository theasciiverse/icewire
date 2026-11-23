//! The parts a rover is wired from: what each one is called, what it does,
//! its ports and its one adjustable value.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Kind {
    Bumper,
    Whisker,
    Ranger,
    Compass,
    And,
    Or,
    Not,
    Compare,
    Timer,
    Toggle,
    Number,
    TrackLeft,
    TrackRight,
    Gyro,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Sensor,
    Logic,
    Drive,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Sensor => "Sensors",
            Category::Logic => "Logic",
            Category::Drive => "Drive",
        }
    }
}

/// Where a sensor looks from on the chassis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mount {
    Nose,
    Right,
    Tail,
    Left,
    All,
}

impl Mount {
    pub const SIDES: [Mount; 4] = [Mount::Nose, Mount::Right, Mount::Tail, Mount::Left];

    pub fn name(self) -> &'static str {
        match self {
            Mount::Nose => "nose",
            Mount::Right => "right",
            Mount::Tail => "tail",
            Mount::Left => "left",
            Mount::All => "all",
        }
    }

    pub fn parse(s: &str) -> Option<Mount> {
        [Mount::Nose, Mount::Right, Mount::Tail, Mount::Left, Mount::All].into_iter().find(|m| m.name() == s)
    }

    /// The direction it faces, in degrees clockwise from the nose.
    pub fn angle(self) -> f64 {
        match self {
            Mount::Nose | Mount::All => 0.0,
            Mount::Right => 90.0,
            Mount::Tail => 180.0,
            Mount::Left => 270.0,
        }
    }
}

pub const COMPASS_MODES: [&str; 3] = ["heading", "bearing", "distance"];
pub const COMPARE_MODES: [&str; 3] = [">", "<", "="];

pub const ALL: [Kind; 14] = [
    Kind::Bumper,
    Kind::Whisker,
    Kind::Ranger,
    Kind::Compass,
    Kind::And,
    Kind::Or,
    Kind::Not,
    Kind::Compare,
    Kind::Timer,
    Kind::Toggle,
    Kind::Number,
    Kind::TrackLeft,
    Kind::TrackRight,
    Kind::Gyro,
];

/// Every part box on the bench is this big, borders included.
pub const BOX_W: i32 = 13;
pub const BOX_H: i32 = 4;

/// The longest a ranger can see, in metres.
pub const RANGER_MAX: f64 = 8.0;

/// One adjustable value: label, unit, default, smallest, largest, step.
pub struct Value {
    pub label: &'static str,
    pub unit: &'static str,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Bumper => "Bumper",
            Kind::Whisker => "Whisker",
            Kind::Ranger => "Ranger",
            Kind::Compass => "Compass",
            Kind::And => "AND",
            Kind::Or => "OR",
            Kind::Not => "NOT",
            Kind::Compare => "Compare",
            Kind::Timer => "Timer",
            Kind::Toggle => "Toggle",
            Kind::Number => "Number",
            Kind::TrackLeft => "Left track",
            Kind::TrackRight => "Right track",
            Kind::Gyro => "Gyro",
        }
    }

    /// The word used in rover files.
    pub fn key(self) -> &'static str {
        match self {
            Kind::Bumper => "bumper",
            Kind::Whisker => "whisker",
            Kind::Ranger => "ranger",
            Kind::Compass => "compass",
            Kind::And => "and",
            Kind::Or => "or",
            Kind::Not => "not",
            Kind::Compare => "compare",
            Kind::Timer => "timer",
            Kind::Toggle => "toggle",
            Kind::Number => "number",
            Kind::TrackLeft => "track-left",
            Kind::TrackRight => "track-right",
            Kind::Gyro => "gyro",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        ALL.into_iter().find(|k| k.key() == s)
    }

    pub fn category(self) -> Category {
        match self {
            Kind::Bumper | Kind::Whisker | Kind::Ranger | Kind::Compass => Category::Sensor,
            Kind::TrackLeft | Kind::TrackRight | Kind::Gyro => Category::Drive,
            _ => Category::Logic,
        }
    }

    pub fn inputs(self) -> &'static [&'static str] {
        match self {
            Kind::And | Kind::Or | Kind::Compare => &["a", "b"],
            Kind::Gyro => &["right", "left"],
            Kind::TrackLeft | Kind::TrackRight => &["fwd", "back"],
            Kind::Not | Kind::Timer | Kind::Toggle => &["in"],
            _ => &[],
        }
    }

    pub fn has_output(self) -> bool {
        self.category() != Category::Drive
    }

    /// Sensors sit on a mount; the bumper can also feel all round.
    pub fn mounts(self) -> &'static [Mount] {
        match self {
            Kind::Bumper => &[Mount::All, Mount::Nose, Mount::Right, Mount::Tail, Mount::Left],
            Kind::Whisker | Kind::Ranger => &[Mount::Nose, Mount::Right, Mount::Tail, Mount::Left],
            _ => &[],
        }
    }

    pub fn modes(self) -> &'static [&'static str] {
        match self {
            Kind::Compass => &COMPASS_MODES,
            Kind::Compare => &COMPARE_MODES,
            _ => &[],
        }
    }

    pub fn value(self) -> Option<Value> {
        let v = |label, unit, default, min, max, step| Some(Value { label, unit, default, min, max, step });
        match self {
            Kind::Whisker => v("reach", "m", 1.0, 0.1, 4.0, 0.1),
            Kind::Compare => v("b", "", 0.0, -999.0, 999.0, 0.5),
            Kind::Timer => v("hold", "s", 0.5, 0.1, 30.0, 0.1),
            Kind::Number => v("value", "", 1.0, -999.0, 999.0, 1.0),
            Kind::TrackLeft | Kind::TrackRight => v("power", "%", 60.0, -100.0, 100.0, 5.0),
            Kind::Gyro => v("power", "%", 100.0, -100.0, 100.0, 5.0),
            _ => None,
        }
    }

    pub fn default_mount(self) -> Mount {
        match self {
            Kind::Bumper => Mount::All,
            _ => Mount::Nose,
        }
    }

    /// One line for lists and the status bar.
    pub fn summary(self) -> &'static str {
        match self {
            Kind::Bumper => "1 while the rover touches something",
            Kind::Whisker => "1 when anything is within reach",
            Kind::Ranger => "metres to the nearest thing ahead",
            Kind::Compass => "heading, or bearing and distance to the beacon",
            Kind::And => "on when a and b are both on",
            Kind::Or => "on when a or b is on",
            Kind::Not => "1 when its input is off",
            Kind::Compare => "1 when a > b, a < b or a = b",
            Kind::Timer => "holds 1 for a while after its input turns on",
            Kind::Toggle => "flips between 0 and 1 each time its input turns on",
            Kind::Number => "always gives its value",
            Kind::TrackLeft => "drives the left side; turns the rover right",
            Kind::TrackRight => "drives the right side; turns the rover left",
            Kind::Gyro => "spins the rover right or left on the spot",
        }
    }

    /// The parts library's longer text.
    pub fn about(self) -> &'static str {
        match self {
            Kind::Bumper => {
                "A sprung steel ring round the hull with a contact in each quarter. Gives 1 for as long as that quarter of the rover is pressed against a wall or another rover, 0 otherwise. Mount it on one side, or on all of them at once."
            }
            Kind::Whisker => {
                "Three fine sensing hairs fanned across a quarter of the view. Gives 1 when a wall, a rover or the edge of the ice is within its reach, so the rover can turn before it touches. Set the reach in metres."
            }
            Kind::Ranger => {
                "A thin light beam straight out from its mount. Gives the distance in metres to the first wall, rover or edge it meets, up to 8. Wire it to a compare to ask how close is too close."
            }
            Kind::Compass => {
                "Knows where the rover points and where the challenge's beacon is: the next checkpoint in a race, the hatch in the Cold Stores, the other rover in the Puck. Heading gives 0 to 360 degrees, 0 up the map. Bearing gives -180 to 180: how far the beacon is to the right of the nose. Distance gives metres to the beacon."
            }
            Kind::And => "On when both inputs are on (anything but 0), else 0. It passes b along: with 1s in, 1 comes out, and with a 1 in a and 0.4 in b, 0.4 comes out. So an AND is also a gate that lets a number through only while a is on.",
            Kind::Or => "On when either input is on, else 0. It gives a when a is on, else b: with 1s and 0s that is plain OR, and with a 1 from a test in a and 0.4 in b, it picks full power or 0.4.",
            Kind::Not => "Gives 1 when its input is off (0 or unwired), else 0.",
            Kind::Compare => {
                "Compares a with b and gives 1 when the test holds. With nothing wired to b, b is the value you set. The = test rounds both to one decimal."
            }
            Kind::Timer => {
                "When its input turns on, gives 1 for the hold time, then 0 until the input turns on again. Good for backing away from a wall for half a second."
            }
            Kind::Toggle => "Starts at 0 and flips between 0 and 1 every time its input turns on. A one-bit memory.",
            Kind::Number => "Gives its value, always. Wire it to a track to keep the rover moving.",
            Kind::TrackLeft => {
                "The left drive track. Whatever comes into fwd drives it forward and whatever comes into back drives it backward, at input times power: with 1 into fwd and power 60, sixty percent ahead. Run it harder than the right track to turn right."
            }
            Kind::TrackRight => {
                "The right drive track, the left one's twin. Run it harder than the left track to turn left."
            }
            Kind::Gyro => {
                "A spinning wheel inside the hull that turns the rover on the spot. It has two inputs: whatever comes into right turns it right, whatever comes into left turns it left, at input times power. A negative number into right turns it left, so a compass bearing wired to right with a small power steers the rover toward the beacon."
            }
        }
    }
}

/// How many of each part one Mule chassis holds.
pub const SENSOR_SLOTS: usize = 8;
pub const LOGIC_SLOTS: usize = 30;

/// Formats a signal or a value compactly: "1", "0.5", "-12.3".
pub fn fmt_num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r == r.trunc() { format!("{}", r as i64) } else { format!("{r:.1}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for k in ALL {
            assert_eq!(Kind::parse(k.key()), Some(k));
        }
        for m in [Mount::Nose, Mount::Right, Mount::Tail, Mount::Left, Mount::All] {
            assert_eq!(Mount::parse(m.name()), Some(m));
        }
    }

    #[test]
    fn numbers_format() {
        assert_eq!(fmt_num(1.0), "1");
        assert_eq!(fmt_num(0.25), "0.3");
        assert_eq!(fmt_num(-12.34), "-12.3");
    }
}
