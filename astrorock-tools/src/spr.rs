//! # LBBSPR sprite sequence parser
//!
//! Reads the original game's `.spr` files (`CFrameSequence::Save` /
//! `LoadFromBuffer` in the reference `sequence.cpp`). All 32 shipped
//! sprites are version 1; the format grew through version 4:
//!
//! ```text
//! "LBBSPR" (6 bytes)
//! u32 version            (<= 4)
//! u32 bits per pixel     (always 8)
//! u32 num_frames
//! u32 num_rotations      (version >= 2 only; else 1)
//! i32 bounds l,t,r,b     (version >= 4 only)
//! u8  palette[768]       (RGB, full 0-255 range)
//! per frame (num_frames * num_rotations):
//!   u32 width, u32 height
//!   u8  bits[width * height]      (scan width == width, no padding —
//!                                  CFrame::Initialize(x,y) sets
//!                                  SetStates(Width))
//!   i32 hotspot_x, i32 hotspot_y
//!   version >= 3: u8 has_alpha, then alpha payload if set
//! ```
//!
//! Alpha payloads (version >= 3) never occur in the shipped data, so this
//! parser rejects them loudly rather than carrying an untestable decoder.

#[derive(Debug)]
pub struct SprFrame {
    pub width: u32,
    pub height: u32,
    pub hot_x: i32,
    pub hot_y: i32,
    /// `width * height` palette indices, row-major.
    pub bits: Vec<u8>,
}

#[derive(Debug)]
pub struct SprSequence {
    pub version: u32,
    pub num_frames: u32,
    pub num_rotations: u32,
    /// l, t, r, b — only meaningful for version >= 4 files.
    pub original_bounds: [i32; 4],
    /// 256 RGB triples.
    pub palette: [u8; 768],
    pub frames: Vec<SprFrame>,
}

pub fn parse_spr(data: &[u8]) -> Result<SprSequence, String> {
    let mut r = Reader { data, pos: 0 };

    if r.take(6)? != b"LBBSPR" {
        return Err("not an LBBSPR file".into());
    }
    let version = r.u32()?;
    if version > 4 {
        return Err(format!("unsupported sprite version {version}"));
    }
    let bpp = r.u32()?;
    if bpp != 8 {
        return Err(format!("expected 8 bpp, found {bpp}"));
    }
    let num_frames = r.u32()?;
    let num_rotations = if version >= 2 { r.u32()? } else { 1 };
    let mut original_bounds = [0i32; 4];
    if version >= 4 {
        for b in &mut original_bounds {
            *b = r.u32()? as i32;
        }
    }
    let mut palette = [0u8; 768];
    palette.copy_from_slice(r.take(768)?);

    let count = num_frames
        .checked_mul(num_rotations)
        .ok_or("frame count overflow")?;
    let mut frames = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let width = r.u32()?;
        let height = r.u32()?;
        // The original debug build warns above 1024; treat it as
        // corruption here so a bad walk can't allocate gigabytes.
        if width > 1024 || height > 1024 {
            return Err(format!("implausible frame size {width}x{height}"));
        }
        let bits = r.take((width * height) as usize)?.to_vec();
        let hot_x = r.u32()? as i32;
        let hot_y = r.u32()? as i32;
        if version >= 3 {
            let has_alpha = r.take(1)?[0];
            if has_alpha != 0 {
                return Err("alpha channels are not present in any shipped sprite".into());
            }
        }
        frames.push(SprFrame {
            width,
            height,
            hot_x,
            hot_y,
            bits,
        });
    }
    if r.pos != data.len() {
        return Err(format!(
            "trailing bytes: parsed {} of {}",
            r.pos,
            data.len()
        ));
    }
    Ok(SprSequence {
        version,
        num_frames,
        num_rotations,
        original_bounds,
        palette,
        frames,
    })
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or("unexpected end of file")?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes(b.try_into().expect("length 4")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic v1 sprite: two 2x1 frames with distinct hotspots.
    fn v1_fixture() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(b"LBBSPR");
        d.extend_from_slice(&1u32.to_le_bytes()); // version
        d.extend_from_slice(&8u32.to_le_bytes()); // bpp
        d.extend_from_slice(&2u32.to_le_bytes()); // frames
        d.extend_from_slice(&[7u8; 768]); // palette
        for (bits, hx, hy) in [([1u8, 2u8], 1i32, 0i32), ([3, 4], 0, 1)] {
            d.extend_from_slice(&2u32.to_le_bytes()); // width
            d.extend_from_slice(&1u32.to_le_bytes()); // height
            d.extend_from_slice(&bits);
            d.extend_from_slice(&hx.to_le_bytes());
            d.extend_from_slice(&hy.to_le_bytes());
        }
        d
    }

    #[test]
    fn parses_version1_sequence() {
        let seq = parse_spr(&v1_fixture()).unwrap();
        assert_eq!(seq.version, 1);
        assert_eq!(seq.num_frames, 2);
        assert_eq!(seq.num_rotations, 1);
        assert_eq!(seq.frames.len(), 2);
        assert_eq!(seq.frames[0].bits, vec![1, 2]);
        assert_eq!(seq.frames[0].hot_x, 1);
        assert_eq!(seq.frames[1].hot_y, 1);
        assert_eq!(seq.palette[0], 7);
    }

    #[test]
    fn rejects_trailing_garbage() {
        let mut d = v1_fixture();
        d.push(0xAB);
        assert!(parse_spr(&d).unwrap_err().contains("trailing"));
    }

    #[test]
    fn rejects_bad_magic_and_depth() {
        assert!(parse_spr(b"NOTSPR").is_err());
        let mut d = v1_fixture();
        d[10] = 16; // bpp -> 16
        assert!(parse_spr(&d).unwrap_err().contains("8 bpp"));
    }
}
