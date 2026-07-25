//! # Indexed PNG output for sprite sheets and interface art
//!
//! Sprites become one 8-bit **indexed** PNG per sequence (frames laid out
//! on a uniform grid) plus a JSON sidecar with per-frame rects and
//! hotspots. Keeping the palette indices intact is load-bearing: the
//! game composites in palette space (remaps, fades, translucency), so
//! RGBA baking would destroy information. Index 0 is declared
//! transparent via tRNS purely so previews look right — pixel data is
//! unaffected.

use std::fs;
use std::io::BufWriter;
use std::path::Path;

use serde::Serialize;

use crate::spr::SprSequence;

#[derive(Serialize)]
pub struct SheetFrame {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub hot_x: i32,
    pub hot_y: i32,
}

#[derive(Serialize)]
pub struct SheetMeta {
    pub source: String,
    pub version: u32,
    pub num_frames: u32,
    pub num_rotations: u32,
    pub original_bounds: [i32; 4],
    pub cell_w: u32,
    pub cell_h: u32,
    pub columns: u32,
    pub frames: Vec<SheetFrame>,
}

/// Lay the sequence out on a `columns x rows` grid of `cell_w x cell_h`
/// cells (near-square sheet) and write `<stem>.png` + `<stem>.json`.
pub fn write_sprite_sheet(
    seq: &SprSequence,
    source_name: &str,
    out_dir: &Path,
    stem: &str,
) -> Result<(), String> {
    let count = seq.frames.len() as u32;
    if count == 0 {
        return Err(format!("{source_name}: no frames"));
    }
    let cell_w = seq.frames.iter().map(|f| f.width).max().expect("non-empty");
    let cell_h = seq
        .frames
        .iter()
        .map(|f| f.height)
        .max()
        .expect("non-empty");
    let columns = (count as f64).sqrt().ceil() as u32;
    let rows = count.div_ceil(columns);

    let sheet_w = columns * cell_w;
    let sheet_h = rows * cell_h;
    let mut pixels = vec![0u8; (sheet_w * sheet_h) as usize];
    let mut frames = Vec::with_capacity(seq.frames.len());
    for (i, f) in seq.frames.iter().enumerate() {
        let cx = (i as u32 % columns) * cell_w;
        let cy = (i as u32 / columns) * cell_h;
        for row in 0..f.height {
            let dst = ((cy + row) * sheet_w + cx) as usize;
            let src = (row * f.width) as usize;
            pixels[dst..dst + f.width as usize]
                .copy_from_slice(&f.bits[src..src + f.width as usize]);
        }
        frames.push(SheetFrame {
            x: cx,
            y: cy,
            w: f.width,
            h: f.height,
            hot_x: f.hot_x,
            hot_y: f.hot_y,
        });
    }

    write_indexed_png(
        &out_dir.join(format!("{stem}.png")),
        sheet_w,
        sheet_h,
        &seq.palette,
        &pixels,
    )?;

    let meta = SheetMeta {
        source: source_name.to_string(),
        version: seq.version,
        num_frames: seq.num_frames,
        num_rotations: seq.num_rotations,
        original_bounds: seq.original_bounds,
        cell_w,
        cell_h,
        columns,
        frames,
    };
    let json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    let json_path = out_dir.join(format!("{stem}.json"));
    fs::write(&json_path, json).map_err(|e| format!("write {}: {e}", json_path.display()))?;
    Ok(())
}

/// Write an 8-bit indexed PNG with the given 768-byte RGB palette.
/// Index 0 is marked fully transparent via tRNS (preview nicety only).
pub fn write_indexed_png(
    path: &Path,
    width: u32,
    height: u32,
    palette: &[u8; 768],
    pixels: &[u8],
) -> Result<(), String> {
    debug_assert_eq!(pixels.len(), (width * height) as usize);
    let file = fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette.to_vec());
    encoder.set_trns(vec![0u8]);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .write_image_data(pixels)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spr::{SprFrame, SprSequence};

    fn tiny_sequence() -> SprSequence {
        let mut palette = [0u8; 768];
        palette[3] = 255; // index 1 = red
        SprSequence {
            version: 1,
            num_frames: 3,
            num_rotations: 1,
            original_bounds: [0, 0, 2, 2],
            palette,
            frames: (0..3)
                .map(|i| SprFrame {
                    width: 2,
                    height: 2,
                    hot_x: i,
                    hot_y: -i,
                    bits: vec![i as u8; 4],
                })
                .collect(),
        }
    }

    #[test]
    fn sheet_layout_and_sidecar_roundtrip() {
        let dir = std::env::temp_dir().join("astrorock_sheet_test");
        std::fs::create_dir_all(&dir).unwrap();
        write_sprite_sheet(&tiny_sequence(), "tiny.spr", &dir, "tiny").unwrap();

        // 3 frames -> 2 columns x 2 rows of 2x2 cells.
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("tiny.json")).unwrap()).unwrap();
        assert_eq!(json["columns"], 2);
        assert_eq!(json["frames"].as_array().unwrap().len(), 3);
        assert_eq!(json["frames"][2]["x"], 0);
        assert_eq!(json["frames"][2]["y"], 2);
        assert_eq!(json["frames"][1]["hot_x"], 1);

        // PNG decodes back to the same indices in the right cells.
        let decoder = png::Decoder::new(std::fs::File::open(dir.join("tiny.png")).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (4, 4));
        assert_eq!(reader.info().color_type, png::ColorType::Indexed);
        assert_eq!(buf[0], 0); // frame 0, index 0
        assert_eq!(buf[2], 1); // frame 1 starts at x=2
        assert_eq!(buf[4 * 2], 2); // frame 2 at row 2
    }
}
