//! # astrorock-tools
//!
//! One-time asset conversion tools for the AstroRock port. These read the
//! original game's data from the local (unpublished) reference tree and
//! emit the converted assets this repo commits. See CLAUDE.md "Assets".
//!
//! ```text
//! astrorock-tools extract-rez <rezfile> <REZFILE.hpp> <out-dir>
//! ```
//!
//! `extract-rez` unpacks every resource in the Burgerlib archive to
//! `<out-dir>/<id>_<name>.<ext>` (extension sniffed from content) and
//! prints a manifest. The tuning configs (`*Cfg`) are the entries with no
//! loose source file — they get copied into `assets/config/` by hand after
//! review.

mod rez;

use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, rezfile, hpp, out_dir] if cmd == "extract-rez" => {
            match extract_rez(Path::new(rezfile), Path::new(hpp), Path::new(out_dir)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: astrorock-tools extract-rez <rezfile> <REZFILE.hpp> <out-dir>");
            ExitCode::FAILURE
        }
    }
}

fn extract_rez(rez_path: &Path, hpp_path: &Path, out_dir: &Path) -> Result<(), String> {
    let data = fs::read(rez_path).map_err(|e| format!("read {}: {e}", rez_path.display()))?;
    let hpp =
        fs::read_to_string(hpp_path).map_err(|e| format!("read {}: {e}", hpp_path.display()))?;
    let names = rez::parse_rezfile_hpp(&hpp);
    let archive = rez::RezArchive::parse(&data)?;

    if names.len() != archive.entries.len() {
        eprintln!(
            "warning: {} names in {} but {} archive entries",
            names.len(),
            hpp_path.display(),
            archive.entries.len()
        );
    }

    fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    let mut total_bytes = 0usize;
    for entry in &archive.entries {
        let payload = archive.payload(entry)?;
        let name = names
            .get(&entry.id)
            .cloned()
            .unwrap_or_else(|| format!("unnamed{}", entry.id));
        let ext = sniff_extension(&name, &payload);
        let file_name = format!("{:03}_{}.{}", entry.id, name, ext);
        let out_path = out_dir.join(&file_name);
        fs::write(&out_path, &payload).map_err(|e| format!("write {}: {e}", out_path.display()))?;
        println!(
            "{:>3}  {:<26} {:>8} bytes  ({}, stored {})",
            entry.id,
            file_name,
            payload.len(),
            if entry.flags == rez::FLAG_LZSS {
                "lzss"
            } else {
                "stored"
            },
            entry.stored_len,
        );
        total_bytes += payload.len();
    }
    println!(
        "extracted {} resources, {} bytes total",
        archive.entries.len(),
        total_bytes
    );
    Ok(())
}

/// Pick a file extension from the resource name and payload magic.
fn sniff_extension(name: &str, payload: &[u8]) -> &'static str {
    if payload.starts_with(b"LBBSPR") {
        return "spr";
    }
    if payload.starts_with(b"RIFF") {
        return "wav";
    }
    if payload.starts_with(b"BM") {
        return "bmp";
    }
    if name.ends_with("Cfg") {
        return "cfg";
    }
    if name.ends_with("Pal") && payload.len() == 768 {
        return "pal";
    }
    "bin"
}

#[cfg(test)]
mod tests {
    use super::sniff_extension;

    #[test]
    fn sniffs_known_formats() {
        assert_eq!(sniff_extension("rAstBigSpr", b"LBBSPR\x04\x00"), "spr");
        assert_eq!(sniff_extension("rBonusSnd", b"RIFF1234WAVE"), "wav");
        assert_eq!(sniff_extension("rStatBarBmp", b"BM\x36\x00"), "bmp");
        assert_eq!(sniff_extension("rRocksCfg", b"01:04,\n"), "cfg");
        assert_eq!(sniff_extension("rScorePal", &[0u8; 768]), "pal");
        assert_eq!(sniff_extension("rMystery", b"\x00\x01"), "bin");
    }
}
