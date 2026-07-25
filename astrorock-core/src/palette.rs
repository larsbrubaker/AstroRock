//! # Palette — the indexed → RGBA end of the pipeline
//!
//! Ports of `Palette.cpp`: the 256 x RGB table (from
//! `ART/palettes/*.pal`, raw 768 bytes, full 0-255 range), conversion
//! of an indexed [`Frame`](crate::frame::Frame) to RGBA for
//! presentation through agg-gui, the weighted closest-color match
//! (`GetColorIndex`), and the fade remap tables
//! (`InitializeFadeLookup` + the `FadeBlit[16]` build from
//! AstroRock.cpp) used by the spawn shimmer fade-in. Exact float
//! shapes preserved — the HSL round-trip IS the shipped look.

use std::rc::Rc;

use crate::frame::Frame;
use crate::sprite::SpriteBlit;

/// `#define NUMFADES 16`
pub const NUM_FADES: usize = 16;
/// `#define NO_FADE_VALUE 128` — the "no change" fade level.
const NO_FADE_VALUE: u32 = 128;
/// `RED_WEIGHT`/`GREEN_WIEGHT`/`BLUE_WEIGHT` (sic).
const RED_WEIGHT: u32 = 30;
const GREEN_WEIGHT: u32 = 59;
const BLUE_WEIGHT: u32 = 11;

#[derive(Clone)]
pub struct Palette {
    /// 256 RGB triples.
    pub rgb: [u8; 768],
}

impl Palette {
    /// Load from the raw 768-byte `.pal` layout.
    pub fn from_pal_bytes(bytes: &[u8]) -> Result<Self, String> {
        let rgb: [u8; 768] = bytes
            .try_into()
            .map_err(|_| format!("palette must be 768 bytes, got {}", bytes.len()))?;
        Ok(Self { rgb })
    }

    pub fn color(&self, index: u8) -> (u8, u8, u8) {
        let i = index as usize * 3;
        (self.rgb[i], self.rgb[i + 1], self.rgb[i + 2])
    }

    /// `CPalette::GetColorIndex` — weighted squared-distance closest
    /// color over the full palette (first win on ties, early exit on a
    /// perfect match).
    pub fn get_color_index(&self, r: u32, g: u32, b: u32) -> u8 {
        let mut closest_index = 0usize;
        let mut closest_dist = u32::MAX;
        for i in 0..256usize {
            let (pr, pg, pb) = self.color(i as u8);
            let dr = (r as i32 - pr as i32).unsigned_abs();
            let dg = (g as i32 - pg as i32).unsigned_abs();
            let db = (b as i32 - pb as i32).unsigned_abs();
            let delta = dr * dr * RED_WEIGHT + dg * dg * GREEN_WEIGHT + db * db * BLUE_WEIGHT;
            if delta < closest_dist {
                closest_index = i;
                if delta == 0 {
                    break;
                }
                closest_dist = delta;
            }
        }
        closest_index as u8
    }

    /// `CPalette::InitializeFadeLookup` (full remap/target ranges, no
    /// inverse table — the game's `FadeBlit` configuration): scale
    /// each color's HSL luminance by `level / 128`, pin, round, and
    /// map back to the closest palette index. `level == 128` is the
    /// identity; above brightens (the truncate pin catches overshoot).
    pub fn fade_lookup(&self, level: u32) -> [u8; 256] {
        let mut out = [0u8; 256];
        for (i, slot) in out.iter_mut().enumerate() {
            if level == NO_FADE_VALUE {
                *slot = i as u8;
                continue;
            }
            let (r, g, b) = self.color(i as u8);
            let fr = r as f32 / 255.0f32;
            let fg = g as f32 / 255.0f32;
            let fb = b as f32 / 255.0f32;

            let (h, s, mut l) = rgb_to_hsl(fr, fg, fb);
            l *= level as f32 / NO_FADE_VALUE as f32;
            let (mut fr, mut fg, mut fb) = hsl_to_rgb(h, s, l);

            // `ColorPinFunctionTruncate`.
            fr = fr.clamp(0.0, 1.0);
            fg = fg.clamp(0.0, 1.0);
            fb = fb.clamp(0.0, 1.0);

            let nr = (fr * 255.0f32 + 0.5f32) as u32;
            let ng = (fg * 255.0f32 + 0.5f32) as u32;
            let nb = (fb * 255.0f32 + 0.5f32) as u32;
            *slot = self.get_color_index(nr, ng, nb);
        }
        out
    }

    /// Convert an indexed frame to tightly packed RGBA8. Every pixel is
    /// opaque — the game's "transparency" (index 0 skipping) happened
    /// at blit time; by presentation the buffer is fully composed.
    pub fn frame_to_rgba(&self, frame: &Frame, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(frame.bits.len() * 4);
        for &index in &frame.bits {
            let i = index as usize * 3;
            out.extend_from_slice(&[self.rgb[i], self.rgb[i + 1], self.rgb[i + 2], 255]);
        }
    }
}

/// `RGB_to_HSL` — exact float port (h in degrees, s/l in 0..1).
fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0f32;
    if max == min {
        // Achromatic: hue undefined (the original leaves 0).
        return (0.0, 0.0, l);
    }
    let s = if l <= 0.5f32 {
        (max - min) / (max + min)
    } else {
        (max - min) / (2.0f32 - max - min)
    };
    let delta = max - min;
    let mut h = if r == max {
        (g - b) / delta
    } else if g == max {
        2.0f32 + (b - r) / delta
    } else {
        4.0f32 + (r - g) / delta
    };
    h *= 60.0f32;
    if h < 0.0f32 {
        h += 360.0f32;
    }
    (h, s, l)
}

/// `HSLSubValue`.
fn hsl_sub_value(n1: f32, n2: f32, mut hue: f32) -> f32 {
    if hue > 360.0f32 {
        hue -= 360.0f32;
    } else if hue < 0.0f32 {
        hue += 360.0f32;
    }
    if hue < 60.0f32 {
        n1 + (n2 - n1) * hue / 60.0f32
    } else if hue < 180.0f32 {
        n2
    } else if hue < 240.0f32 {
        n1 + (n2 - n1) * (240.0f32 - hue) / 60.0f32
    } else {
        n1
    }
}

/// `HSL_to_RGB`.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let m2 = if l <= 0.5f32 {
        l * (1.0f32 + s)
    } else {
        l + s - l * s
    };
    let m1 = 2.0f32 * l - m2;
    if s == 0.0f32 {
        (l, l, l)
    } else {
        (
            hsl_sub_value(m1, m2, h + 120.0f32),
            hsl_sub_value(m1, m2, h),
            hsl_sub_value(m1, m2, h - 120.0f32),
        )
    }
}

/// The game's `FadeBlit[NUMFADES]` (AstroRock.cpp init): index 0 is
/// the plain transparent blit ("index 0 is just a normal blit so we
/// don't need to set it"); 1..15 remap through fade levels
/// `256 - 256*i/15` — so low indices brighten past white and high
/// indices sink to black, which is exactly what the spawn shimmer
/// walks through as its countdown halves into this table.
pub struct FadeBlits {
    tables: Vec<Rc<[u8; 256]>>,
}

impl FadeBlits {
    pub fn new(palette: &Palette) -> Self {
        let tables = (1..NUM_FADES)
            .map(|i| {
                let level = 256 - ((256 * i as u32) / (NUM_FADES as u32 - 1));
                Rc::new(palette.fade_lookup(level))
            })
            .collect();
        Self { tables }
    }

    /// `&FadeBlit[index]` as a sprite blit.
    pub fn blit(&self, index: usize) -> SpriteBlit {
        if index == 0 || index >= NUM_FADES {
            SpriteBlit::Trans
        } else {
            SpriteBlit::RemapSource(self.tables[index - 1].clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_length() {
        assert!(Palette::from_pal_bytes(&[0u8; 767]).is_err());
        assert!(Palette::from_pal_bytes(&[0u8; 768]).is_ok());
    }

    #[test]
    fn converts_indexed_to_rgba() {
        let mut rgb = [0u8; 768];
        rgb[3] = 10; // index 1 = (10, 20, 30)
        rgb[4] = 20;
        rgb[5] = 30;
        let pal = Palette::from_pal_bytes(&rgb).unwrap();

        let frame = Frame::from_bits(2, 1, vec![0, 1]);
        let mut rgba = Vec::new();
        pal.frame_to_rgba(&frame, &mut rgba);
        assert_eq!(rgba, vec![0, 0, 0, 255, 10, 20, 30, 255]);
    }

    #[test]
    fn closest_color_finds_exact_palette_entries() {
        let pal = crate::assets::game_palette();
        // Every palette color must map to an index with that exact
        // color (perfect matches short-circuit; duplicates may return
        // an earlier index with identical RGB).
        for i in 0..256u32 {
            let (r, g, b) = pal.color(i as u8);
            let found = pal.get_color_index(r as u32, g as u32, b as u32);
            assert_eq!(pal.color(found), (r, g, b), "index {i}");
        }
    }

    #[test]
    fn fade_level_128_is_identity_and_0_is_black() {
        let pal = crate::assets::game_palette();
        let identity = pal.fade_lookup(128);
        for (i, &v) in identity.iter().enumerate() {
            assert_eq!(v as usize, i);
        }
        // Luminance scaled to zero -> everything lands on a black
        // entry (the game palette keeps black at index 0).
        let black = pal.fade_lookup(0);
        for &v in black.iter() {
            assert_eq!(pal.color(v), (0, 0, 0));
        }
    }

    #[test]
    fn fade_blits_darken_monotonically_at_the_top() {
        let pal = crate::assets::game_palette();
        let fades = FadeBlits::new(&pal);
        // Index 0 is the plain blit.
        assert!(matches!(fades.blit(0), SpriteBlit::Trans));
        // The last table (level 0) sends bright white to black; an
        // early table keeps it bright.
        let bright = 15u8; // white in the game palette
        let SpriteBlit::RemapSource(last) = fades.blit(NUM_FADES - 1) else {
            panic!("expected remap");
        };
        assert_eq!(pal.color(last[bright as usize]), (0, 0, 0));
        let SpriteBlit::RemapSource(first) = fades.blit(1) else {
            panic!("expected remap");
        };
        let (r, g, b) = pal.color(first[bright as usize]);
        assert!(
            r as u32 + g as u32 + b as u32 > 600,
            "level-239 white should stay bright, got ({r},{g},{b})"
        );
    }
}
