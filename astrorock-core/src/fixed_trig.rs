//! # Degree trig tables — port of `FixedTrig.cpp`
//!
//! The shipped game compiles `CFixed` as `float` (`USE_AS_FIXED 0` in
//! `Fixed.hpp`), so these are f32 tables built once at startup:
//!
//! - sin/cos: 450 entries (`cos(a) = sin_table[a + 90]`), from
//!   `sin((float)i * (Pi_f32 / 180.0))` in double precision
//! - tan: 360 entries, zeroed at 0/90/180/270
//! - atan: triangular octant table, 64 rows, `atan2(j, i) * (180f/Pi_f32)`
//! - sqrt: 256-entry mantissa table driving the bit-twiddling `FSqrt`
//!
//! Determinism: table generation uses the `libm` crate, not `std`, so
//! native and wasm builds produce identical f32 bits (std's sin/cos go
//! to platform libm on native but compiler-rt on wasm). The lock-in
//! tests pin exact bit patterns. Whether these match the 1997 MSVC/x87
//! binary bit-for-bit is settled empirically by demo replay (Phase 9).

use std::sync::OnceLock;

/// `#define Pi ((CFixed)3.1415927)` — the C literal rounds to the same
/// f32 bits (0x40490FDB) as `f32::consts::PI`, so the std constant is
/// bit-identical to the original.
const PI_F32: f32 = std::f32::consts::PI;

const ATBL_SIZE_BITS: u32 = 6;
const ATBL_ROWS: usize = 1 << ATBL_SIZE_BITS;

struct Tables {
    /// 450 entries: sin 0..=359 plus 90 more so cos indexes past 359.
    sin: [f32; 450],
    tan: [f32; 360],
    /// Triangular: row i holds i+1 entries for j = 0..=i.
    angle: Vec<f32>,
    sqrt: [u32; 256],
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(build_tables)
}

/// Force table construction (the original required `FTrigInit()` before
/// use; in Rust the tables are lazy, this just front-loads the cost).
pub fn ftrig_init() {
    let _ = tables();
}

fn build_tables() -> Tables {
    let mut sin = [0f32; 450];
    for (i, s) in sin.iter_mut().enumerate() {
        // sin((CFixed)i * (Pi / 180.0)) — float i and float Pi promoted
        // to double, sin in double, result truncated to float.
        *s = libm::sin((i as f32) as f64 * (PI_F32 as f64 / 180.0)) as f32;
    }

    let mut tan = [0f32; 360];
    for (i, t) in tan.iter_mut().enumerate() {
        if i != 0 && i != 90 && i != 180 && i != 270 {
            *t = libm::tan((i as f32) as f64 * (PI_F32 as f64 / 180.0)) as f32;
        }
    }

    // (CFixed)atan2((double)j, (double)i) * (TOFLOAT(180) / Pi) —
    // atan2 in double truncated to float, then an f32 multiply.
    let deg_per_rad: f32 = 180.0f32 / PI_F32;
    let mut angle = Vec::with_capacity(ATBL_ROWS * (ATBL_ROWS + 1) / 2);
    for i in 0..ATBL_ROWS {
        for j in 0..=i {
            angle.push(libm::atan2(j as f64, i as f64) as f32 * deg_per_rad);
        }
    }

    let mut sqrt = [0u32; 256];
    for i in 0..=0x7Fu32 {
        // Mantissa bits i<<16 with exponent 0 (stored 127) then 1 (128);
        // sqrt in double (C promotes float sqrt arg), truncated to float,
        // top 7 mantissa bits harvested.
        for (half, exponent) in [(0usize, 127u32), (0x80, 128)] {
            let f = f32::from_bits((i << 16) | (exponent << 23));
            let root = libm::sqrt(f as f64) as f32;
            sqrt[half + i as usize] = (root.to_bits() & 0x7F_FFFF) >> 16;
        }
    }

    Tables {
        sin,
        tan,
        angle,
        sqrt,
    }
}

/// Offset of triangular row `i` (row i holds i+1 entries).
fn angle_row(i: usize) -> usize {
    i * (i + 1) / 2
}

/// `FTrigCosD` — cosine of a whole-degree angle; 0.0 outside 0..360.
pub fn cos_d(angle: u32) -> f32 {
    if angle < 360 {
        tables().sin[angle as usize + 90]
    } else {
        0.0
    }
}

/// `FTrigSinD` — sine of a whole-degree angle; 0.0 outside 0..360.
pub fn sin_d(angle: u32) -> f32 {
    if angle < 360 {
        tables().sin[angle as usize]
    } else {
        0.0
    }
}

/// `FTrigTanD` — tangent of a whole-degree angle; 0.0 outside 0..360
/// and at the poles (0/90/180/270, as the original table stores).
pub fn tan_d(angle: u32) -> f32 {
    if angle < 360 {
        tables().tan[angle as usize]
    } else {
        0.0
    }
}

/// `FTrigTanDXY` — unit direction vector for a whole-degree angle.
pub fn tan_d_xy(angle: u32) -> (f32, f32) {
    if angle < 360 {
        let t = tables();
        (t.sin[angle as usize + 90], t.sin[angle as usize])
    } else {
        (0.0, 0.0)
    }
}

/// `calcAngle` — table lookup for the first octant. `x` and `y` must be
/// non-negative with `y <= x` after the caller's octant folding.
fn calc_angle(mut x: u32, mut y: u32) -> f32 {
    if x != 0 {
        while x >= (1 << ATBL_SIZE_BITS) {
            x >>= 1;
            y >>= 1;
        }
    }
    tables().angle[angle_row(x as usize) + y as usize]
}

/// `FTrigATanD` — angle of the vector (x, y) in whole-ish degrees,
/// counter-clockwise, 0 = +x. Exact port of the octant fold.
pub fn atan_d(x: i32, y: i32) -> f32 {
    if y == 0 {
        return if x >= 0 { 0.0 } else { 180.0 };
    }
    if x == 0 {
        return if y >= 0 { 90.0 } else { 270.0 };
    }
    if x.abs() == y.abs() {
        return if x > 0 {
            if y > 0 {
                45.0
            } else {
                135.0
            }
        } else if y > 0 {
            225.0
        } else {
            315.0
        };
    }
    if x > 0 {
        if y > 0 {
            if x > y {
                return calc_angle(x as u32, y as u32);
            }
            return 90.0 - calc_angle(y as u32, x as u32);
        }
        let y = -y;
        if x > y {
            return 360.0 - calc_angle(x as u32, y as u32);
        }
        return 270.0 + calc_angle(y as u32, x as u32);
    }
    let x = -x;
    if y > 0 {
        if x > y {
            return 180.0 - calc_angle(x as u32, y as u32);
        }
        return 90.0 + calc_angle(y as u32, x as u32);
    }
    let y = -y;
    if x > y {
        return 180.0 + calc_angle(x as u32, y as u32);
    }
    270.0 - calc_angle(y as u32, x as u32)
}

/// `FTrigATanDRelative`.
pub fn atan_d_relative(x: i32, y: i32, origin_x: i32, origin_y: i32) -> f32 {
    atan_d(x - origin_x, y - origin_y)
}

/// `FTrigDistance` — projected distance between two points via the
/// angle tables (the original's cheap length approximation).
pub fn distance(x1: i32, y1: i32, x2: i32, y2: i32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let angle = atan_d(dx, dy) as i32; // TOINT: truncate toward zero
    let mut value = cos_d(angle as u32) * dx as f32;
    value += sin_d(angle as u32) * dy as f32;
    if value < 0.0 {
        value = -value;
    }
    value
}

/// `FSqrt` — fast square root by mantissa table lookup, bit-exact port.
/// Callers only pass non-negative values (as in the original).
pub fn fsqrt(n: f32) -> f32 {
    if n == 0.0 {
        return 0.0;
    }
    let mut num = n.to_bits();
    // `short e = (*num >> 23) - 127` — sign bit folds into e for
    // negative inputs, exactly as the C did (garbage in, garbage out).
    let mut e = (num >> 23) as i32 - 127;
    num &= 0x7F_FFFF;
    if e & 0x01 != 0 {
        num |= 0x80_0000;
    }
    e >>= 1; // arithmetic shift, sign-preserving like C
    num = (tables().sqrt[(num >> 16) as usize] << 16) | (((e + 127) as u32) << 23);
    f32::from_bits(num)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact f32 bit patterns computed by evaluating the original C
    /// expressions (double-precision libm, truncation points included)
    /// offline. These pin cross-platform determinism: native and wasm
    /// must both produce these bits.
    #[test]
    fn sin_table_lock_in() {
        assert_eq!(sin_d(0).to_bits(), 0x0000_0000);
        assert_eq!(sin_d(1).to_bits(), 0x3C8E_F85A);
        assert_eq!(sin_d(30).to_bits(), 0x3F00_0000); // exactly 0.5
        assert_eq!(sin_d(45).to_bits(), 0x3F35_04F3);
        assert_eq!(sin_d(90).to_bits(), 0x3F80_0000); // exactly 1.0
        assert_eq!(sin_d(180).to_bits(), 0xB3BB_BD2E); // not quite 0 — float Pi
        assert_eq!(sin_d(359).to_bits(), 0xBC8E_F7FC);
        // cos table is the sin table shifted 90 (entry 449 = sin 449).
        assert_eq!(cos_d(359).to_bits(), 0x3F7F_F605);
        assert_eq!(cos_d(0).to_bits(), 0x3F80_0000);
    }

    #[test]
    fn angle_table_lock_in() {
        assert_eq!(atan_d(1, 0).to_bits(), 0x0000_0000);
        assert_eq!(atan_d(2, 1).to_bits(), 0x41D4_8539); // 26.565°
        assert_eq!(atan_d(3, 1).to_bits(), 0x4193_7AC6); // 18.435°
        assert_eq!(atan_d(63, 20).to_bits(), 0x418C_E68F); // 17.613°
    }

    #[test]
    fn atan_octants_and_axes() {
        assert_eq!(atan_d(5, 0), 0.0);
        assert_eq!(atan_d(-5, 0), 180.0);
        assert_eq!(atan_d(0, 5), 90.0);
        assert_eq!(atan_d(0, -5), 270.0);
        assert_eq!(atan_d(7, 7), 45.0);
        assert_eq!(atan_d(7, -7), 135.0); // original's quirky quadrant labels
        assert_eq!(atan_d(-7, 7), 225.0);
        assert_eq!(atan_d(-7, -7), 315.0);
        // Large vectors fold down through the >>1 loop: (126, 40)
        // halves once to (63, 20).
        let a = atan_d(126, 40);
        let b = atan_d(63, 20);
        assert_eq!(a.to_bits(), b.to_bits());
    }

    #[test]
    fn fsqrt_lock_in() {
        assert_eq!(fsqrt(0.0), 0.0);
        assert_eq!(fsqrt(2.0), 1.4140625);
        assert_eq!(fsqrt(100.0), 10.0);
        assert_eq!(fsqrt(0.25), 0.5);
    }

    #[test]
    fn distance_uses_projection() {
        // 3-4-5 triangle: the table projection lands near 5.
        let d = distance(0, 0, 3, 4);
        assert!((d - 5.0).abs() < 0.05, "distance = {d}");
        // Axis-aligned distances are exact.
        assert_eq!(distance(10, 0, 22, 0), 12.0);
    }

    #[test]
    fn out_of_range_angles_return_zero() {
        assert_eq!(sin_d(360), 0.0);
        assert_eq!(cos_d(360), 0.0);
        assert_eq!(tan_d(1000), 0.0);
        assert_eq!(tan_d_xy(360), (0.0, 0.0));
    }
}
