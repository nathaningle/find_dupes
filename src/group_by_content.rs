use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::DedupFile;

const BUFFER_LEN: usize = 1024 * 1024; // 1 MiB

// Group a list of files by their content.  We assume that the candidates have already been
// shortlisted, i.e. there are no duplicates (by inode) and all are the same size.
//
// The scenarios for each input group of files are:
//
//   - the group is empty
//   - all the files are the same
//   - all the files are different
//   - some files are the same but others are different
//   - multiple groups of files that are the same
//   - multiple groups of files that are the same and some that are different
//
#[derive(Debug)]
pub struct GroupByContentIter {
    input_queue: Vec<Vec<DedupFile>>,
    output_queue: Vec<Vec<DedupFile>>,
}

impl Iterator for GroupByContentIter {
    type Item = Vec<DedupFile>;

    fn next(&mut self) -> Option<Self::Item> {
        while !(self.input_queue.is_empty() && self.output_queue.is_empty()) {
            while let Some(output_group) = self.output_queue.pop() {
                if !output_group.is_empty() {
                    return Some(output_group);
                }
            }

            if let Some(input_group) = self.input_queue.pop() {
                self.output_queue.append(&mut regroup(input_group));
            }
        }

        None
    }
}

fn regroup(mut candidates: Vec<DedupFile>) -> Vec<Vec<DedupFile>> {
    // The algorithm here works like this: Consider a stack of coloured dinner plates.  To group
    // them by colour:
    //
    //   1. If the stack is empty, then finish.
    //   2. Pick up a plate from the stack.
    //   3. If there is a group of plates that is the same colour as this plate, add this plate to
    //      that group then go back to step 1.
    //   4. Place the plate as a new group to the right of the existing groups.
    //   5. Go back to step 1.
    //
    let mut groups: Vec<Vec<DedupFile>> = Vec::new();

    'candidate: while let Some(candidate) = candidates.pop() {
        for group in &mut groups {
            if let Ok(true) = compare_file_bytes(&candidate.paths[0], &group[0].paths[0]) {
                group.push(candidate);
                continue 'candidate;
            }
        }
        groups.push(vec![candidate]);
    }

    groups.retain(|g| g.len() > 1);
    groups
}

// Compare the content of two files.
fn compare_file_bytes(path1: &Path, path2: &Path) -> io::Result<bool> {
    let mut file1 = File::open(path1)?;
    let mut file2 = File::open(path2)?;
    let mut buf1 = [0; BUFFER_LEN];
    let mut buf2 = [0; BUFFER_LEN];

    loop {
        let read_count1 = file1.read(&mut buf1)?;
        let read_count2 = file2.read(&mut buf2)?;

        if read_count1 != read_count2 || buf1 != buf2 {
            return Ok(false);
        }

        if read_count1 != BUFFER_LEN {
            break;
        }
    }

    Ok(true)
}

pub fn group_by_content(groups_by_size: Vec<Vec<DedupFile>>) -> GroupByContentIter {
    GroupByContentIter {
        input_queue: groups_by_size,
        output_queue: Vec::new(),
    }
}
