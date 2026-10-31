//! The journal of files that were written and closed without error. Each line is `size<TAB>path`.
//! A run re-verifies the recent entries when a device comes back, and a restart re-verifies the last
//! entries for the same root before it writes anything.
use std::{
    collections::{HashSet, VecDeque},
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const FILE: &str = "fsfiller-journal.txt";
/// How many of the most recent entries are kept in memory and re-verified.
pub const KEEP: usize = 1000;

pub struct Journal {
    file: Mutex<File>,
    recent: Mutex<VecDeque<(PathBuf, u64)>>,
}

impl Journal {
    /// Opens the journal in the working directory for appending.
    pub fn open() -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(FILE)?;
        Ok(Self {
            file: Mutex::new(file),
            recent: Mutex::new(VecDeque::with_capacity(KEEP)),
        })
    }

    /// Records a file that was written and closed without error.
    pub fn record(&self, path: &Path, size: u64) -> io::Result<()> {
        writeln!(self.file.lock().unwrap(), "{size}\t{}", path.display())?;
        let mut recent = self.recent.lock().unwrap();
        if recent.len() == KEEP {
            recent.pop_front();
        }
        recent.push_back((path.to_path_buf(), size));
        Ok(())
    }

    /// The most recent entries from this run, oldest first.
    pub fn recent(&self) -> Vec<(PathBuf, u64)> {
        self.recent.lock().unwrap().iter().cloned().collect()
    }
}

/// The last `KEEP` journal entries under `root` from earlier runs, one per path. A later run can
/// rewrite a path, so the newest entry for a path wins. A missing journal has no entries.
pub fn read_tail(root: &Path) -> io::Result<Vec<(PathBuf, u64)>> {
    let file = match File::open(FILE) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut entries = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let Some((size, path)) = line.split_once('\t') else {
            continue;
        };
        let (Ok(size), path) = (size.parse::<u64>(), PathBuf::from(path)) else {
            continue;
        };
        if path.starts_with(root) {
            entries.push((path, size));
        }
    }
    let mut seen = HashSet::new();
    let mut tail: Vec<_> = entries
        .into_iter()
        .rev()
        .filter(|(path, _)| seen.insert(path.clone()))
        .take(KEEP)
        .collect();
    tail.reverse();
    Ok(tail)
}
