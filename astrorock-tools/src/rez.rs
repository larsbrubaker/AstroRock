//! # Burgerlib BRGR resource archive reader
//!
//! Parses the built `rezfile` that shipped with AstroRock and inflates its
//! LZSS-compressed entries. Reverse-engineered from the archive itself and
//! validated against the loose `ART/`/`SOUND/` source trees; the tuning
//! configs it recovers are the only assets with no loose source.
//!
//! Layout (all little-endian):
//! ```text
//! 0x00  "BRGR" magic
//! 0x04  u32 version (1)
//! 0x08  u32 (unused by us; 0x09e4 in the shipped file)
//! 0x0c  u32 (1)
//! 0x10  u32 (1)
//! 0x14  u32 entry count
//! 0x18  entries[count], 12 bytes each:
//!         u32 offset(24-bit) | flags(top 8 bits) — 0x20 = LZSS, 0x00 = stored
//!         u32 stored length
//!         u32 reserved (0)
//! ```
//! Entries are contiguous; resource IDs are 1-based table indices, named by
//! the original `REZFILE.hpp` `#define rXxx N` list.
//!
//! LZSS entries begin with a u32 decompressed size, then an Okumura-style
//! stream: control byte read LSB-first, set bit = literal byte, clear bit =
//! 2-byte copy token `{lo, hi}` where `offset = ((hi & 0x0F) << 8) | lo`,
//! `length = (hi >> 4) + 3`, copying from `4096 - offset` bytes back in the
//! output (window pre-fill is never referenced by the shipped archive — we
//! error if an entry tries).

use std::collections::BTreeMap;

pub const FLAG_LZSS: u8 = 0x20;
pub const FLAG_STORED: u8 = 0x00;
const WINDOW: usize = 4096;

pub struct RezEntry {
    /// 1-based resource ID (`REZFILE.hpp` value).
    pub id: u32,
    pub flags: u8,
    pub offset: usize,
    pub stored_len: usize,
}

pub struct RezArchive<'a> {
    data: &'a [u8],
    pub entries: Vec<RezEntry>,
}

impl<'a> RezArchive<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, String> {
        if data.len() < 0x18 || &data[0..4] != b"BRGR" {
            return Err("not a BRGR archive".into());
        }
        let count = read_u32(data, 0x14) as usize;
        let table_end = 0x18 + count * 12;
        if data.len() < table_end {
            return Err(format!("truncated entry table (count {count})"));
        }

        let mut entries = Vec::with_capacity(count);
        let mut expected_offset: Option<usize> = None;
        for i in 0..count {
            let base = 0x18 + i * 12;
            let off_flags = read_u32(data, base);
            let stored_len = read_u32(data, base + 4) as usize;
            let offset = (off_flags & 0x00FF_FFFF) as usize;
            let flags = (off_flags >> 24) as u8;
            if flags != FLAG_LZSS && flags != FLAG_STORED {
                return Err(format!("entry {}: unknown flags 0x{flags:02x}", i + 1));
            }
            if offset + stored_len > data.len() {
                return Err(format!("entry {}: extends past end of file", i + 1));
            }
            // The shipped archive is fully contiguous; treat gaps as
            // corruption so we notice if an unexpected variant appears.
            if let Some(expected) = expected_offset {
                if offset != expected {
                    return Err(format!("entry {}: gap in archive layout", i + 1));
                }
            }
            expected_offset = Some(offset + stored_len);
            entries.push(RezEntry {
                id: (i + 1) as u32,
                flags,
                offset,
                stored_len,
            });
        }
        if expected_offset != Some(data.len()) {
            return Err("archive data does not end at file end".into());
        }
        Ok(Self { data, entries })
    }

    /// Decode entry payload (inflating LZSS entries).
    pub fn payload(&self, entry: &RezEntry) -> Result<Vec<u8>, String> {
        let raw = &self.data[entry.offset..entry.offset + entry.stored_len];
        match entry.flags {
            FLAG_STORED => Ok(raw.to_vec()),
            FLAG_LZSS => decompress_lzss(raw).map_err(|e| format!("resource {}: {e}", entry.id)),
            other => Err(format!(
                "resource {}: unknown flags 0x{other:02x}",
                entry.id
            )),
        }
    }
}

/// Inflate one LZSS entry (`u32` decompressed size + token stream).
pub fn decompress_lzss(raw: &[u8]) -> Result<Vec<u8>, String> {
    if raw.len() < 4 {
        return Err("LZSS entry shorter than its size header".into());
    }
    let out_len = read_u32(raw, 0) as usize;
    let src = &raw[4..];
    let mut out = Vec::with_capacity(out_len);
    let mut i = 0;

    while out.len() < out_len {
        let Some(&ctrl) = src.get(i) else {
            return Err("LZSS stream truncated (control byte)".into());
        };
        i += 1;
        for bit in 0..8 {
            if out.len() >= out_len {
                break;
            }
            if ctrl & (1 << bit) != 0 {
                let Some(&lit) = src.get(i) else {
                    return Err("LZSS stream truncated (literal)".into());
                };
                out.push(lit);
                i += 1;
            } else {
                let (Some(&lo), Some(&hi)) = (src.get(i), src.get(i + 1)) else {
                    return Err("LZSS stream truncated (copy token)".into());
                };
                i += 2;
                let offset = (((hi & 0x0F) as usize) << 8) | lo as usize;
                let length = ((hi >> 4) as usize) + 3;
                let dist = WINDOW - offset;
                if dist > out.len() {
                    return Err("LZSS copy references window pre-fill".into());
                }
                for _ in 0..length {
                    let b = out[out.len() - dist];
                    out.push(b);
                }
            }
        }
    }
    Ok(out)
}

/// Parse `#define rXxx N` lines from the original `REZFILE.hpp`.
pub fn parse_rezfile_hpp(text: &str) -> BTreeMap<u32, String> {
    let mut names = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("#define ") else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let (Some(name), Some(value)) = (parts.next(), parts.next()) else {
            continue;
        };
        if let (true, Ok(id)) = (name.starts_with('r'), value.parse::<u32>()) {
            names.insert(id, name.to_string());
        }
    }
    names
}

fn read_u32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().expect("bounds checked"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a raw LZSS entry from (control, bytes) fragments.
    fn lzss_entry(out_len: u32, stream: &[u8]) -> Vec<u8> {
        let mut v = out_len.to_le_bytes().to_vec();
        v.extend_from_slice(stream);
        v
    }

    #[test]
    fn lzss_all_literals() {
        // Control 0xFF = 8 literals.
        let raw = lzss_entry(8, &[0xFF, b'0', b'1', b':', b'0', b'4', b',', b'\n', b'0']);
        assert_eq!(decompress_lzss(&raw).unwrap(), b"01:04,\n0");
    }

    #[test]
    fn lzss_copy_token_matches_shipped_archive_semantics() {
        // Mirrors the first tokens of the shipped rRocksCfg entry:
        // 8 literals "01:04,\n0", then control 0xF5 = literal '2',
        // copy {0xF9,0x3F} (offset 0xFF9 → distance 7, length 6),
        // literal '3', copy {0xF2,0x3F} (distance 14, length 6),
        // literals "4:05".
        let raw = lzss_entry(
            26,
            &[
                0xFF, b'0', b'1', b':', b'0', b'4', b',', b'\n', b'0', //
                0xF5, b'2', 0xF9, 0x3F, b'3', 0xF2, 0x3F, b'4', b':', b'0', b'5',
            ],
        );
        assert_eq!(
            decompress_lzss(&raw).unwrap(),
            b"01:04,\n02:04,\n03:04,\n04:05"
        );
    }

    #[test]
    fn lzss_rejects_prefill_reference() {
        // Copy token before any output exists must error, not fabricate.
        let raw = lzss_entry(3, &[0x00, 0x00, 0x30]);
        assert!(decompress_lzss(&raw).is_err());
    }

    #[test]
    fn parses_rezfile_hpp_defines() {
        let hpp = "/* header */\n#define rLogicWareLogoBmp 1\n#define rRocksCfg 15\n#define NOT_A_RES 3\n";
        let names = parse_rezfile_hpp(hpp);
        assert_eq!(names.get(&1).map(String::as_str), Some("rLogicWareLogoBmp"));
        assert_eq!(names.get(&15).map(String::as_str), Some("rRocksCfg"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn parses_synthetic_archive() {
        // Two stored entries, contiguous, IDs 1..=2.
        let mut data = Vec::new();
        data.extend_from_slice(b"BRGR");
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes());
        let data_start = 0x18 + 2 * 12;
        for (off, len) in [(data_start, 3usize), (data_start + 3, 2usize)] {
            data.extend_from_slice(&(off as u32).to_le_bytes()); // flags 0x00
            data.extend_from_slice(&(len as u32).to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
        }
        data.extend_from_slice(b"abcde");

        let rez = RezArchive::parse(&data).unwrap();
        assert_eq!(rez.entries.len(), 2);
        assert_eq!(rez.payload(&rez.entries[0]).unwrap(), b"abc");
        assert_eq!(rez.payload(&rez.entries[1]).unwrap(), b"de");
    }
}
