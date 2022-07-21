use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

mod group_by_inode;
use group_by_inode::{group_by_inode, DedupFile};

mod group_by_content;
use group_by_content::group_by_content;

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
    let matches = App::new("find_dupes")
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
    //
    // Consolidate  by device number and inode -- i.e. find multiple hard links to the same file
    // on disk.  It's going to take some time to traverse the filesystem, so if we were to group
    // by size first, there's a risk the file could change as we're traversing.
    let mut files_by_inode: HashMap<(u64, u64), DedupFile> = HashMap::new();
    for f in group_by_inode(target, min_size) {
        let ino = (f.device, f.inode);
        match files_by_inode.get_mut(&ino) {
            Some(existing_f) => {
                // We found another hard link to a file on disk we've already seen.  Since we have
                // a single thread, we use the updated details from the new one.
                assert_eq!(f.paths.len(), 1);
                existing_f.paths.push(f.paths[0].to_path_buf());
                existing_f.size = f.size;
                existing_f.nlink = f.nlink;
            }
            None => {
                files_by_inode.insert(ino, f);
            }
        }
    }

    // Now group our consolidated list of files on disk by size.
    let mut dupes_by_size: HashMap<u64, Vec<DedupFile>> = HashMap::new();
    for f in files_by_inode.into_values() {
        match dupes_by_size.get_mut(&f.size) {
            Some(existing_f) => {
                existing_f.push(f);
            }
            None => {
                dupes_by_size.insert(f.size, vec![f]);
            }
        }
    }

    // Finally, check the list of files by size to find which are actually the same data.
    let shortlist: Vec<Vec<DedupFile>> = dupes_by_size
        .into_values()
        .filter(|grp| grp.len() > 1)
        .collect();
    let dupes_by_content: Vec<Vec<DedupFile>> = group_by_content(shortlist).collect();

    // Write results to stdout as JSON.
    println!("{}", serde_json::to_string(&dupes_by_content).unwrap());

    Ok(())
}
