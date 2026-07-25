//! # Palette — the indexed → RGBA end of the pipeline
//!
//! Ports the piece of `Palette.cpp` the compositor needs today: a 256 x
//! RGB table (from `ART/palettes/*.pal`, raw 768 bytes, full 0-255
//! range) and conversion of an indexed [`Frame`](crate::frame::Frame)
//! to RGBA for presentation through agg-gui. Fades, remap-table
//! generation, and the inverse-color-table build land with the systems
//! that use them.

use crate::frame::Frame;

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
}
