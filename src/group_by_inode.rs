use std::collections::HashSet;
use std::fs::{self, DirEntry, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Serialize;

// Vital stats of a file.
#[derive(Debug, Serialize)]
pub struct DedupFile {
    pub paths: Vec<PathBuf>,
    pub size: u64,
    pub device: u64,
    pub inode: u64,
    pub nlink: u64,
}

pub struct GroupByInodeIter {
    min_size: u64,
    file_queue: Vec<DedupFile>,
    dir_queue: Vec<PathBuf>,
    seen_dirs: HashSet<(u64, u64)>,
}

impl GroupByInodeIter {
    // True iff the metadata belongs to a directory we would like to traverse.
    fn is_wanted_dir(&self, metadata: &Metadata) -> bool {
        metadata.is_dir() && !self.seen_dirs.contains(&(metadata.dev(), metadata.ino()))
    }

    // True iff the metadata belongs to a file we would like to consider.
    fn is_wanted_file(&self, metadata: &Metadata) -> bool {
        metadata.is_file() && metadata.len() >= self.min_size
    }

    // Push a file/directory to the appropriate queue (if we want to).
    fn push_child(&mut self, path: &Path, metadata: &Metadata) {
        if self.is_wanted_dir(metadata) {
            self.dir_queue.push(path.to_path_buf());
        } else if self.is_wanted_file(metadata) {
            self.file_queue.push(DedupFile {
                paths: vec![path.to_path_buf()],
                size: metadata.len(),
                device: metadata.dev(),
                inode: metadata.ino(),
                nlink: metadata.nlink(),
            });
        }
    }

    // Read a directory's children, ignoring failures.
    fn read_dir_optimistically(path: &Path) -> Vec<DirEntry> {
        match fs::read_dir(path) {
            Err(_) => Vec::new(),
            Ok(read_dir) => read_dir.filter_map(|d| d.ok()).collect(),
        }
    }
}

impl Iterator for GroupByInodeIter {
    type Item = DedupFile;

    fn next(&mut self) -> Option<Self::Item> {
        while !(self.file_queue.is_empty() && self.dir_queue.is_empty()) {
            // If we have some files from a previous dir read, return those first.  This results in
            // a breadth-first traversal of the filesystem hierarchy.
            let f = self.file_queue.pop();
            if f.is_some() {
                return f;
            }

            // If we have a candidate directory from a previous dir read, push its children onto
            // the queues.
            if let Some(dir_path) = self.dir_queue.pop() {
                for child_entry in GroupByInodeIter::read_dir_optimistically(&dir_path) {
                    if let Ok(child_metadata) = child_entry.metadata() {
                        self.push_child(&child_entry.path(), &child_metadata);
                        // Don't return a result here -- do that on the next iteration of the
                        // outer loop.
                    }
                }
            }
        }

        assert!(self.dir_queue.is_empty());
        assert!(self.file_queue.is_empty());
        None
    }
}

// Recursively descend through a filesystem hierarchy, collecting information about only regular
// files.
pub fn group_by_inode(root: &Path, min_size: u64) -> GroupByInodeIter {
    let root_absolute = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    GroupByInodeIter {
        min_size,
        file_queue: Vec::new(),
        dir_queue: vec![root_absolute],
        seen_dirs: HashSet::new(),
    }
}
