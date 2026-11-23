//! Deterministic maths for the arena. Only `+`, `-`, `*`, `/` and `sqrt`
//! (all exactly rounded by IEEE 754) are used, so a run gives the same
//! result bit for bit on every platform. The platform's `sin`, `cos` and
//! `atan2` are not guaranteed to, so these replace them.

pub const PI: f64 = std::f64::consts::PI;
pub const TAU: f64 = 2.0 * PI;

/// Wraps an angle into -PI..=PI.
pub fn wrap(a: f64) -> f64 {
    let mut a = a - TAU * (a / TAU).round();
    if a > PI {
        a -= TAU;
    }
    if a < -PI {
        a += TAU;
    }
    a
}

/// Sine by Taylor series after folding into -PI/2..=PI/2.
pub fn sin(a: f64) -> f64 {
    let mut x = wrap(a);
    if x > PI / 2.0 {
        x = PI - x;
    } else if x < -PI / 2.0 {
        x = -PI - x;
    }
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    let mut n = 1.0;
    for _ in 0..9 {
        term = -term * x2 / ((n + 1.0) * (n + 2.0));
        sum += term;
        n += 2.0;
    }
    sum
}

pub fn cos(a: f64) -> f64 {
    sin(a + PI / 2.0)
}

/// Arctangent: two half-angle steps bring |x| under 0.2, then a series.
fn atan(x: f64) -> f64 {
    let (x, flip) = if x.abs() > 1.0 { (1.0 / x, true) } else { (x, false) };
    let mut y = x;
    for _ in 0..2 {
        y /= 1.0 + (1.0 + y * y).sqrt();
    }
    let y2 = y * y;
    let mut term = y;
    let mut sum = y;
    let mut k = 1.0;
    for _ in 0..12 {
        term = -term * y2;
        k += 2.0;
        sum += term / k;
    }
    let r = 4.0 * sum;
    if flip {
        if x > 0.0 { PI / 2.0 - r } else { -PI / 2.0 - r }
    } else {
        r
    }
}

pub fn atan2(y: f64, x: f64) -> f64 {
    if x > 0.0 {
        atan(y / x)
    } else if x < 0.0 {
        if y >= 0.0 { atan(y / x) + PI } else { atan(y / x) - PI }
    } else if y > 0.0 {
        PI / 2.0
    } else if y < 0.0 {
        -PI / 2.0
    } else {
        0.0
    }
}

pub fn deg(rad: f64) -> f64 {
    rad * 180.0 / PI
}

pub fn rad(deg: f64) -> f64 {
    deg * PI / 180.0
}

/// A unit vector for a heading: 0 is north (up the screen), clockwise.
pub fn dir(heading: f64) -> (f64, f64) {
    (sin(heading), -cos(heading))
}

/// The heading of a vector, in the same sense as `dir`.
pub fn heading_of(dx: f64, dy: f64) -> f64 {
    atan2(dx, -dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trig_matches_std_closely() {
        let mut a = -10.0;
        while a < 10.0 {
            assert!((sin(a) - a.sin()).abs() < 1e-12, "sin {a}");
            assert!((cos(a) - a.cos()).abs() < 1e-12, "cos {a}");
            a += 0.0137;
        }
        let mut y = -3.0;
        while y < 3.0 {
            let mut x = -3.0;
            while x < 3.0 {
                assert!((atan2(y, x) - y.atan2(x)).abs() < 1e-12, "atan2 {y} {x}");
                x += 0.173;
            }
            y += 0.191;
        }
    }

    #[test]
    fn headings() {
        let (x, y) = dir(0.0);
        assert!(x.abs() < 1e-12 && (y + 1.0).abs() < 1e-12);
        let (x, y) = dir(PI / 2.0);
        assert!((x - 1.0).abs() < 1e-12 && y.abs() < 1e-12);
        assert!((heading_of(1.0, 0.0) - PI / 2.0).abs() < 1e-12);
        assert!((heading_of(0.0, 1.0).abs() - PI).abs() < 1e-12);
    }
}
