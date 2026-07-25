//! # 8-bit indexed BMP parser
//!
//! Reads the interface art (`ART/interfac/*.bmp`) the way the original
//! `BMPFileIO.cpp` does: uncompressed 8-bit BITMAPINFOHEADER files with a
//! 256-entry BGRA palette, bottom-up rows padded to 4 bytes. Palette
//! indices are preserved — the game composites in palette space.

#[derive(Debug)]
pub struct IndexedBmp {
    pub width: u32,
    pub height: u32,
    /// 256 RGB triples (converted from the file's BGRA quads).
    pub palette: [u8; 768],
    /// `width * height` palette indices, row-major, top-down.
    pub bits: Vec<u8>,
}

pub fn parse_bmp(data: &[u8]) -> Result<IndexedBmp, String> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return Err("not a BMP file".into());
    }
    let pixel_offset = u32_at(data, 0x0A)? as usize;
    let header_size = u32_at(data, 0x0E)?;
    if header_size < 40 {
        return Err(format!("unsupported BMP header size {header_size}"));
    }
    let width_raw = u32_at(data, 0x12)? as i32;
    let height_raw = u32_at(data, 0x16)? as i32;
    let bpp = u16_at(data, 0x1C)?;
    let compression = u32_at(data, 0x1E)?;
    if bpp != 8 {
        return Err(format!("expected 8 bpp BMP, found {bpp}"));
    }
    if compression != 0 {
        return Err(format!(
            "expected uncompressed BMP, found method {compression}"
        ));
    }
    if width_raw <= 0 || width_raw > 4096 || height_raw == 0 || height_raw.unsigned_abs() > 4096 {
        return Err(format!("implausible BMP size {width_raw}x{height_raw}"));
    }
    let width = width_raw as u32;
    // Negative height = top-down rows (never produced by the original
    // tools, but cheap to honor).
    let top_down = height_raw < 0;
    let height = height_raw.unsigned_abs();

    let colors_used = u32_at(data, 0x2E)?;
    let palette_entries = if colors_used == 0 {
        256
    } else {
        colors_used as usize
    };
    if palette_entries > 256 {
        return Err(format!("too many palette entries: {palette_entries}"));
    }
    let palette_offset = 14 + header_size as usize;
    let mut palette = [0u8; 768];
    for i in 0..palette_entries {
        let at = palette_offset + i * 4;
        let quad = data
            .get(at..at + 4)
            .ok_or("palette extends past end of file")?;
        // File order is B, G, R, reserved.
        palette[i * 3] = quad[2];
        palette[i * 3 + 1] = quad[1];
        palette[i * 3 + 2] = quad[0];
    }

    let row_stride = ((width as usize) + 3) & !3;
    let mut bits = vec![0u8; (width * height) as usize];
    for row in 0..height as usize {
        let src_row = if top_down {
            row
        } else {
            height as usize - 1 - row
        };
        let at = pixel_offset + src_row * row_stride;
        let src = data
            .get(at..at + width as usize)
            .ok_or("pixel data extends past end of file")?;
        bits[row * width as usize..(row + 1) * width as usize].copy_from_slice(src);
    }

    Ok(IndexedBmp {
        width,
        height,
        palette,
        bits,
    })
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().expect("length 4")))
        .ok_or_else(|| "unexpected end of file".into())
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes(b.try_into().expect("length 2")))
        .ok_or_else(|| "unexpected end of file".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal 2x2 8-bit BMP: indices [[0,1],[2,3]] top-down, stored
    /// bottom-up with 4-byte row padding.
    fn fixture() -> Vec<u8> {
        let mut d = Vec::new();
        let palette_bytes = 256 * 4;
        let pixel_offset = 14 + 40 + palette_bytes;
        d.extend_from_slice(b"BM");
        d.extend_from_slice(&((pixel_offset + 8) as u32).to_le_bytes()); // file size
        d.extend_from_slice(&0u32.to_le_bytes()); // reserved
        d.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
        d.extend_from_slice(&40u32.to_le_bytes()); // header size
        d.extend_from_slice(&2i32.to_le_bytes()); // width
        d.extend_from_slice(&2i32.to_le_bytes()); // height (bottom-up)
        d.extend_from_slice(&1u16.to_le_bytes()); // planes
        d.extend_from_slice(&8u16.to_le_bytes()); // bpp
        d.extend_from_slice(&0u32.to_le_bytes()); // compression
        d.extend_from_slice(&[0u8; 20]); // image size + resolutions + colors
        for i in 0..256u32 {
            // B, G, R, reserved — make R = index so the swap is testable.
            d.extend_from_slice(&[0, 0, i as u8, 0]);
        }
        d.extend_from_slice(&[2, 3, 0, 0]); // bottom row (padded)
        d.extend_from_slice(&[0, 1, 0, 0]); // top row (padded)
        d
    }

    #[test]
    fn parses_bottom_up_indexed_bmp() {
        let bmp = parse_bmp(&fixture()).unwrap();
        assert_eq!((bmp.width, bmp.height), (2, 2));
        assert_eq!(bmp.bits, vec![0, 1, 2, 3]);
        // BGR -> RGB swap: entry 5's red channel carries the index.
        assert_eq!(bmp.palette[5 * 3], 5);
        assert_eq!(bmp.palette[5 * 3 + 2], 0);
    }

    #[test]
    fn rejects_non_8bpp() {
        let mut d = fixture();
        d[0x1C] = 24;
        assert!(parse_bmp(&d).unwrap_err().contains("8 bpp"));
    }
}
