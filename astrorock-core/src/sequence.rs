//! # Frame sequences — port of `CFrameSequence` (`sequence.cpp`)
//!
//! Loads the converted sprite sheets (indexed PNG + JSON sidecar from
//! `astrorock-tools`) back into per-frame indexed [`Frame`]s with their
//! hotspots. Shared between sprites via `Rc` (the original refcounted
//! by hand).

use std::rc::Rc;

use serde::Deserialize;

use crate::assets::frame_from_indexed_png;
use crate::frame::Frame;

#[derive(Deserialize)]
struct SheetFrameMeta {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    hot_x: i32,
    hot_y: i32,
}

#[derive(Deserialize)]
struct SheetMeta {
    num_frames: u32,
    num_rotations: u32,
    original_bounds: [i32; 4],
    frames: Vec<SheetFrameMeta>,
}

pub struct FrameSequence {
    pub frames: Vec<Frame>,
    pub num_frames: u32,
    pub num_rotations: u32,
    /// l, t, r, b of the uncropped art (`OrigonalBounds`).
    pub original_bounds: [i32; 4],
}

impl FrameSequence {
    /// Rebuild a sequence from a converted sheet. Panics on malformed
    /// embedded data — assets ship inside the binary.
    pub fn from_sheet(png_bytes: &[u8], json_bytes: &[u8]) -> Rc<Self> {
        let sheet = frame_from_indexed_png(png_bytes);
        let meta: SheetMeta = serde_json::from_slice(json_bytes).expect("sheet json");

        let frames = meta
            .frames
            .iter()
            .map(|f| {
                let mut frame = Frame::new(f.w as i32, f.h as i32);
                frame.hot_x = f.hot_x;
                frame.hot_y = f.hot_y;
                for row in 0..f.h {
                    let src_start = ((f.y + row) * sheet.width as u32 + f.x) as usize;
                    let dst_start = (row * f.w) as usize;
                    frame.bits[dst_start..dst_start + f.w as usize]
                        .copy_from_slice(&sheet.bits[src_start..src_start + f.w as usize]);
                }
                frame
            })
            .collect();

        Rc::new(Self {
            frames,
            num_frames: meta.num_frames,
            num_rotations: meta.num_rotations,
            original_bounds: meta.original_bounds,
        })
    }

    /// `NumFrames / NumRotations` — frames in one rotation's animation.
    pub fn frames_per_rotation(&self) -> u32 {
        self.num_frames / self.num_rotations
    }
}

/// Embedded sprite sheets, loaded on demand. Add entries as gameplay
/// systems come online.
macro_rules! embedded_sequence {
    ($fn_name:ident, $stem:literal) => {
        pub fn $fn_name() -> Rc<FrameSequence> {
            FrameSequence::from_sheet(
                include_bytes!(concat!("../../assets/sprites/", $stem, ".png")),
                include_bytes!(concat!("../../assets/sprites/", $stem, ".json")),
            )
        }
    };
}

embedded_sequence!(ship, "ship");
embedded_sequence!(ast_big, "astb");
embedded_sequence!(ast_med, "astm");
embedded_sequence!(ast_small, "asts");
embedded_sequence!(shot01, "shot01");
embedded_sequence!(shot02, "shot02");
embedded_sequence!(shot03, "shot03");
embedded_sequence!(bomb, "bomb");
embedded_sequence!(shield, "shield");
embedded_sequence!(thrust0, "thrust0");
embedded_sequence!(thrust1, "thrust1");
embedded_sequence!(thrust2, "thrust2");
embedded_sequence!(thrust3, "thrust3");
embedded_sequence!(thrust4, "thrust4");
embedded_sequence!(explo, "explo");
embedded_sequence!(bg_explo, "bgexp");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ship_sheet_roundtrips() {
        let seq = ship();
        assert_eq!(seq.num_frames, 32);
        assert_eq!(seq.num_rotations, 1);
        assert_eq!(seq.frames.len(), 32);
        assert_eq!(seq.frames_per_rotation(), 32);
        // Every frame has real pixels and a hotspot inside itself
        // (the converter centered them via CenterHotSpot at save time).
        for (i, f) in seq.frames.iter().enumerate() {
            assert!(f.bits.iter().any(|&b| b != 0), "frame {i} empty");
            assert!(f.hot_x >= 0 && f.hot_x <= f.width, "frame {i} hot_x");
            assert!(f.hot_y >= 0 && f.hot_y <= f.height, "frame {i} hot_y");
        }
    }

    #[test]
    fn asteroid_sheets_load() {
        assert_eq!(ast_big().frames.len(), 45);
        assert_eq!(ast_med().frames.len(), 35);
        assert_eq!(ast_small().frames.len(), 30);
    }
}
