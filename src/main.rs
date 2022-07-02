use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

const BUFFER_LEN: usize = 1024 * 1024; // 1 MiB

// Vital stats of a file.
//
// TODO: hashing files only makes sense if we expect to find files that are duplicated 3 or more
// times.  If there are 2 copies of a file, it's faster to compare them byte-by-byte.  If there is
// only one copy of a file, calculating a hash is a wasted operation.  CRC32 is fast but weak, so
// we may wish to compare files byte-by-byte anyway.
#[derive(Debug)]
struct DedupFile {
    path: PathBuf,
    size: u64,
    dev: u64,
    ino: u64,
    nlink: u64,
    hash: Option<u32>,
}

// Calculate the CRC32 checksum of a file.
fn calc_crc32(path: &Path) -> Result<u32> {
    use crc32fast::Hasher;

    let mut file = File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buffer = [0; BUFFER_LEN];

    loop {
        let read_count = file.read(&mut buffer)?;
        hasher.update(&buffer[..read_count]);

        if read_count != BUFFER_LEN {
            break;
        }
    }

    Ok(hasher.finalize())
}

// Recursively descend through a filesystem hierarchy, collecting information about only regular
// files.
fn visit(path: &Path, min_size: u64) -> Result<Vec<DedupFile>> {
    let metadata = fs::symlink_metadata(path)?;
    let mut dedup_files: Vec<DedupFile> = Vec::new();

    if metadata.is_dir() {
        // Recurse over directory contents.
        for dir_entry in fs::read_dir(path)? {
            let mut child_dedup_files: Vec<DedupFile> = visit(&dir_entry?.path(), min_size)?;
            dedup_files.append(&mut child_dedup_files);
        }
    } else if metadata.is_file() && metadata.len() >= min_size {
        // Record info about this file.
        let dedup_file = DedupFile {
            path: fs::canonicalize(path)?,
            size: metadata.len(),
            dev: metadata.dev(),
            ino: metadata.ino(),
            nlink: metadata.nlink(),
            hash: Some(calc_crc32(path)?),
        };
        dedup_files.push(dedup_file);
    }

    // If it's not a directory and it's not a file, then we don't care.  Perhaps it's a symlink,
    // socket, pipe or block/character special.
    Ok(dedup_files)
}

// Parse a string describing the size of a file, with optional SI or IEC unit prefix.
fn parse_file_size_spec(s: &str) -> Result<u64> {
    let mut t: String = s.to_owned();
    t.make_ascii_lowercase();
    let (num_str, suffix) = t
        .find(|c: char| c.is_ascii_alphabetic())
        .map(|i| t.split_at(i))
        .unwrap_or((&t, ""));
    let multiplier: u64 = match suffix {
        "" => 1,
        "k" | "kb" => 1_000,
        "m" | "mb" => 1_000_000,
        "g" | "gb" => 1_000_000_000,
        "t" | "tb" => 1_000_000_000_000,
        "ki" | "kib" => 1_024,
        "mi" | "mib" => 1_024 * 1_024,
        "gi" | "gib" => 1_024 * 1_024 * 1_024,
        "ti" | "tib" => 1_024 * 1_024 * 1_024 * 1_024,
        _ => bail!("Failed to parse file size (bad multiplier -- got {:?})", s),
    };
    num_str
        .parse()
        .map(|num: u64| num * multiplier)
        .with_context(|| format!("Failed to parse file size (bad number -- got {:?})", s))
}

fn main() -> Result<()> {
    use clap::{App, Arg};

    // Parse command-line arguments.
    let matches = App::new("file-dedup")
        .about("Identify duplicate files")
        .arg(
            Arg::with_name("PATH")
                .help("Location to search")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("MIN_SIZE")
                .long("min-size")
                .help("Ignore files smaller than this (bytes)")
                .default_value("100000"),
        )
        .get_matches();

    let target = Path::new(
        matches
            .value_of("PATH")
            .expect("Failed to read PATH from command-line arguments"),
    );

    let min_size_str = matches
        .value_of("MIN_SIZE")
        .expect("Failed to find MIN_SIZE argument despite clap default_value");
    let min_size: u64 = parse_file_size_spec(min_size_str)?;

    // Traverse the filesystem.  Since we expect to be limited by disk I/O, there may be no
    // performance benefit from parallelism.
    let dedup_files = visit(target, min_size)?;

    // For now, we output results in CSV format to placate Rust's dead code analysis.
    for d in dedup_files {
        let d_path = d.path.to_str().unwrap();
        match d.hash {
            Some(d_hash) => println!(
                "{},{},{},{},{},{}",
                d_path, d.size, d.dev, d.ino, d.nlink, d_hash
            ),
            None => println!("{},{},{},{},{},None", d_path, d.size, d.dev, d.ino, d.nlink),
        }
    }
    Ok(())
}
