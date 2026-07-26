//! # The tilt joystick — SDF metaball rendering + steering state
//!
//! The mobile steering indicator: an outer ring at the pad's edge, a
//! thin inner ring marking the dead zone, and a dot at the current
//! tilt (or thumb) position. The dot and the outer ring are drawn as
//! one signed-distance field combined with a smooth-min, so the dot
//! MERGES into the ring as it approaches the edge — a 2D metaball.
//!
//! Rendered on the CPU into a small RGBA buffer each frame (the pad
//! is ~100 px square — a few thousand pixels) and blitted through
//! `DrawCtx::draw_image_rgba_arc` by chrome.rs.

/// Full deflection, in degrees of device lean: the dot reaches the
/// outer ring at this much tilt, and a thumb at the pad edge maps to
/// the same steering strength.
pub const MAX_TILT_DEG: f64 = 24.0;
/// Degrees of lean (from the calibrated rest plane) before steering
/// engages — mirrored by the thin inner ring.
pub const DEAD_ZONE_DEG: f64 = 8.0;
/// Fraction of full deflection where thrust kicks in. With the
/// heading snapping instantly, aiming happens near the center — so
/// thrust can start well before the rim without accidental burns
/// (tuned by feel: 0.95 read as "a bit too far").
pub const THRUST_FRAC: f64 = 0.8;

/// Outer ring center radius as a fraction of the half-size.
const RING_R: f64 = 0.86;
/// Outer ring half-thickness.
const RING_T: f64 = 0.05;
/// Dot radius.
const DOT_R: f64 = 0.17;
/// Metaball blend width — how early the dot and ring start merging.
const BLEND_K: f64 = 0.22;

/// Polynomial smooth-min (Inigo Quilez): blends two SDFs so their
/// union grows a smooth neck instead of a hard crease.
fn smin(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b * (1.0 - h) + a * h - k * h * (1.0 - h)
}

/// Rasterize the pad into an RGBA buffer, `size` x `size`.
/// `pos` is the dot position in steering units (-1..1 per axis,
/// length 1 = full deflection at the outer ring); it is clamped to
/// the ring. `active` lights the shape (steering outside the dead
/// zone or a thumb on the pad).
pub fn render(size: usize, pos: (f64, f64), active: bool) -> Vec<u8> {
    let mut rgba = vec![0u8; size * size * 4];
    if size < 8 {
        return rgba;
    }
    let half = size as f64 / 2.0;
    // Clamp the dot inside the ring circle.
    let len = (pos.0 * pos.0 + pos.1 * pos.1).sqrt();
    let (dx, dy) = if len > 1.0 {
        (pos.0 / len, pos.1 / len)
    } else {
        pos
    };
    let dot_c = (dx * RING_R, dy * RING_R);
    let dead_r = (DEAD_ZONE_DEG / MAX_TILT_DEG) * RING_R;

    // ~1.5px anti-alias band in SDF units.
    let aa = 1.5 / half;
    let (body, dead_ring) = if active {
        ([140u8, 170, 230], [90u8, 100, 124])
    } else {
        ([96u8, 106, 128], [70u8, 78, 96])
    };

    for py in 0..size {
        for px in 0..size {
            // Pixel center in [-1, 1].
            let x = (px as f64 + 0.5) / half - 1.0;
            let y = (py as f64 + 0.5) / half - 1.0;
            let r = (x * x + y * y).sqrt();

            // The metaball pair: outer ring annulus + dot disk.
            let d_ring = (r - RING_R).abs() - RING_T;
            let ddx = x - dot_c.0;
            let ddy = y - dot_c.1;
            let d_dot = (ddx * ddx + ddy * ddy).sqrt() - DOT_R;
            let d = smin(d_ring, d_dot, BLEND_K);
            let body_a = (0.5 - d / (2.0 * aa)).clamp(0.0, 1.0);

            // The thin dead-zone ring, faded where the body covers.
            let d_dz = (r - dead_r).abs() - 0.014;
            let dz_a = (0.5 - d_dz / (2.0 * aa)).clamp(0.0, 1.0) * (1.0 - body_a) * 0.8;

            let a = body_a + dz_a;
            if a <= 0.003 {
                continue;
            }
            let idx = (py * size + px) * 4;
            // Premix the two layers, straight alpha out.
            let mix = |b: u8, d: u8| -> u8 { ((b as f64 * body_a + d as f64 * dz_a) / a) as u8 };
            rgba[idx] = mix(body[0], dead_ring[0]);
            rgba[idx + 1] = mix(body[1], dead_ring[1]);
            rgba[idx + 2] = mix(body[2], dead_ring[2]);
            rgba[idx + 3] = (a.min(1.0) * 255.0) as u8;
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(rgba: &[u8], size: usize, fx: f64, fy: f64) -> u8 {
        let px = ((fx + 1.0) / 2.0 * size as f64) as usize;
        let py = ((fy + 1.0) / 2.0 * size as f64) as usize;
        rgba[(py.min(size - 1) * size + px.min(size - 1)) * 4 + 3]
    }

    #[test]
    fn ring_dot_and_dead_zone_render() {
        let size = 96;
        let img = render(size, (0.0, 0.0), true);
        // Dot at the center, ring at the edge, dead-zone ring between.
        assert!(alpha_at(&img, size, 0.0, 0.0) > 200, "center dot");
        assert!(alpha_at(&img, size, RING_R, 0.0) > 200, "outer ring");
        let dead_r = (DEAD_ZONE_DEG / MAX_TILT_DEG) * RING_R;
        assert!(alpha_at(&img, size, dead_r, 0.0) > 60, "dead-zone ring");
        // Empty gap between the dead-zone ring and the outer ring.
        let gap = (dead_r + RING_R) / 2.0;
        assert!(alpha_at(&img, size, gap, 0.0) < 40, "gap stays clear");
    }

    #[test]
    fn dot_merges_into_the_ring_near_the_edge() {
        let size = 96;
        // Dot fully deflected right: the neck between dot and ring
        // must be filled (metaball merge) — sample halfway between
        // the dot center and the ring.
        let img = render(size, (1.0, 0.0), true);
        let dot_x = RING_R; // clamped dot center
        let neck_x = (dot_x - DOT_R + (RING_R - RING_T)) / 2.0;
        assert!(
            alpha_at(&img, size, neck_x, 0.0) > 150,
            "neck between dot and ring should be merged"
        );
        // Centered dot leaves that same spot empty.
        let img = render(size, (0.0, 0.0), true);
        assert!(alpha_at(&img, size, neck_x, 0.0) < 40);
    }

    #[test]
    fn out_of_range_positions_clamp() {
        let size = 64;
        // A wild tilt vector must not panic or leave the buffer.
        let img = render(size, (5.0, -7.0), false);
        assert_eq!(img.len(), size * size * 4);
    }
}
