//! # astrorock-tools
//!
//! One-time asset conversion tools for the AstroRock port. These read the
//! original game's data from the local (unpublished) reference tree and
//! emit the converted assets this repo commits. See CLAUDE.md "Assets".
//!
//! ```text
//! astrorock-tools extract-rez      <rezfile> <REZFILE.hpp> <out-dir>
//! astrorock-tools convert-sprites  <spr-dir> <out-dir>
//! astrorock-tools convert-bmps     <bmp-dir> <out-dir>
//! ```
//!
//! `extract-rez` unpacks every resource in the Burgerlib archive to
//! `<out-dir>/<id>_<name>.<ext>` (extension sniffed from content) and
//! prints a manifest. The tuning configs (`*Cfg`) are the entries with no
//! loose source file — they get copied into `assets/config/` by hand after
//! review.
//!
//! `convert-sprites` turns every `.spr` in a directory into an indexed
//! PNG sheet + JSON sidecar; `convert-bmps` turns every 8-bit `.bmp`
//! into an indexed PNG. Palette indices survive both conversions.

mod bmp;
mod rez;
mod sheet;
mod spr;

use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [cmd, rezfile, hpp, out_dir] if cmd == "extract-rez" => {
            extract_rez(Path::new(rezfile), Path::new(hpp), Path::new(out_dir))
        }
        [cmd, in_dir, out_dir] if cmd == "convert-sprites" => {
            convert_sprites(Path::new(in_dir), Path::new(out_dir))
        }
        [cmd, in_dir, out_dir] if cmd == "convert-bmps" => {
            convert_bmps(Path::new(in_dir), Path::new(out_dir))
        }
        [cmd, rezfile, out_dir] if cmd == "dump-rez" => {
            dump_rez(Path::new(rezfile), Path::new(out_dir))
        }
        _ => {
            eprintln!(
                "usage: astrorock-tools extract-rez <rezfile> <REZFILE.hpp> <out-dir>\n\
                        astrorock-tools convert-sprites <spr-dir> <out-dir>\n\
                        astrorock-tools convert-bmps <bmp-dir> <out-dir>\n\
                        astrorock-tools dump-rez <rezfile> <out-dir>"
            );
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Dump every resource verbatim as `<out-dir>/<id>.bin` — the payload
/// store for the instrumented C++ reference build, whose stub
/// `LoadAResource` freads by numeric ID.
fn dump_rez(rezfile: &Path, out_dir: &Path) -> Result<(), String> {
    let data = fs::read(rezfile).map_err(|e| format!("read {}: {e}", rezfile.display()))?;
    let archive = rez::RezArchive::parse(&data)?;
    fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    let mut dumped = 0;
    for entry in &archive.entries {
        let payload = archive.payload(entry)?;
        let path = out_dir.join(format!("{}.bin", entry.id));
        fs::write(&path, &payload).map_err(|e| format!("write {}: {e}", path.display()))?;
        dumped += 1;
    }
    println!("dumped {dumped} resources to {}", out_dir.display());
    Ok(())
}

/// Convert every `.spr` in `in_dir` to `<stem>.png` + `<stem>.json`.
fn convert_sprites(in_dir: &Path, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    let mut converted = 0;
    for path in files_with_extension(in_dir, "spr")? {
        let data = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let seq = spr::parse_spr(&data).map_err(|e| format!("{}: {e}", path.display()))?;
        let stem = normalized_stem(&path);
        let source = file_name(&path);
        sheet::write_sprite_sheet(&seq, &source, out_dir, &stem)?;
        println!(
            "{source:14} -> {stem}.png  ({} frames x {} rotations)",
            seq.num_frames, seq.num_rotations
        );
        converted += 1;
    }
    println!("converted {converted} sprite sequences");
    Ok(())
}

/// Convert every 8-bit `.bmp` in `in_dir` to an indexed `<stem>.png`.
fn convert_bmps(in_dir: &Path, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    let mut converted = 0;
    for path in files_with_extension(in_dir, "bmp")? {
        let data = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let image = bmp::parse_bmp(&data).map_err(|e| format!("{}: {e}", path.display()))?;
        let stem = normalized_stem(&path);
        sheet::write_indexed_png(
            &out_dir.join(format!("{stem}.png")),
            image.width,
            image.height,
            &image.palette,
            &image.bits,
        )?;
        println!(
            "{:14} -> {stem}.png  ({}x{})",
            file_name(&path),
            image.width,
            image.height
        );
        converted += 1;
    }
    println!("converted {converted} bitmaps");
    Ok(())
}

/// All files in `dir` with the given extension (case-insensitive),
/// sorted by name for stable output.
fn files_with_extension(dir: &Path, ext: &str) -> Result<Vec<std::path::PathBuf>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read dir {}: {e}", dir.display()))?;
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// Lower-cased file stem — the original tree mixes cases freely
/// (`ASTB.spr`, `astroteg.bmp`); the converted tree is uniform.
fn normalized_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_ascii_lowercase()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string()
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
