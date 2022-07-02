use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

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
fn calc_crc32(path: &Path) -> io::Result<u32> {
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
fn visit(path: &Path) -> io::Result<Vec<DedupFile>> {
    let metadata = fs::symlink_metadata(path)?;
    let mut dedup_files: Vec<DedupFile> = Vec::new();

    if metadata.is_dir() {
        // Recurse over directory contents.
        for dir_entry in fs::read_dir(path)? {
            let mut child_dedup_files: Vec<DedupFile> = visit(&dir_entry?.path())?;
            dedup_files.append(&mut child_dedup_files);
        }
    } else if metadata.is_file() {
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

fn main() -> io::Result<()> {
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
        .get_matches();

    let target = Path::new(
        matches
            .value_of("PATH")
            .expect("Failed to read PATH from command-line arguments"),
    );

    // Traverse the filesystem.  Since we expect to be limited by disk I/O, there may be no
    // performance benefit from parallelism.
    let dedup_files = visit(target)?;

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
