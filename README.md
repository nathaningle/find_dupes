# find_dupes

Identify duplicate files in a Linux/Unix filesystem hierarchy.  Tested on Debian and OpenBSD.
Outputs a HTML table to stdout.


## Usage

``` find_dupes Identify duplicate files

USAGE:
    find_dupes [OPTIONS] <PATH>

FLAGS:
    -h, --help       Prints help information -V, --version    Prints version information

OPTIONS:
        --min-size <MIN_SIZE>    Ignore files smaller than this (bytes) [default: 100000]

ARGS:
    <PATH>    Location to search
```


## How it works

1. Descend through the filesystem hierarchy rooted at the given directory, collecting information
   about only regular files (not directories, symlinks, block/character specials, sockets, named
   pipes, etc.).
2. Collate this information by *(device number, inode number)* to identify unique files on disk.
   This avoids checking the same file if it has multiple hard links pointing to it.
3. Group files on disk by size, as a cheap heuristic for duplicate files.
4. Compare the files within each group byte-by-byte.
5. Report the duplicates.

We assume that there will be few duplicates relative to the number of files, so instead of hashing
files we create a shortlist (e.g. files of different sizes are clearly not the same) then simply
compare their contents.

We also assume that disk I/O will limit performance, so we don't bother running in parallel.
