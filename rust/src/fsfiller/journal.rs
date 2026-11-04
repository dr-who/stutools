//! The journal of files that were written and closed without error. It lives in the target root, so it
//! travels with the data. Each line is `size<TAB>path`, with the path relative to the root.
//! A run re-verifies the recent entries when a device comes back, and a restart re-verifies the last
//! entries before it writes anything.
use std::{
    collections::{HashSet, VecDeque},
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const FILE: &str = "fsfiller-journal.txt";
/// How many of the most recent entries are kept in memory and re-verified.
const KEEP: usize = 1000;

pub struct Journal {
    root: PathBuf,
    file: Mutex<File>,
    recent: Mutex<VecDeque<(PathBuf, u64)>>,
}

impl Journal {
    /// Opens the journal in `root` for appending.
    pub fn open(root: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(FILE))?;
        Ok(Self {
            root: root.to_path_buf(),
            file: Mutex::new(file),
            recent: Mutex::new(VecDeque::with_capacity(KEEP)),
        })
    }

    /// Records a file that was written and closed without error.
    pub fn record(&self, path: &Path, size: u64) -> io::Result<()> {
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        writeln!(self.file.lock().unwrap(), "{size}\t{}", relative.display())?;
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

/// The last `KEEP` journal entries in `root` from earlier runs, one per path. A later run can rewrite
/// a path, so the newest entry for a path wins. A missing journal has no entries.
pub fn read_tail(root: &Path) -> io::Result<Vec<(PathBuf, u64)>> {
    let file = match File::open(root.join(FILE)) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut entries = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let Some((size, relative)) = line.split_once('\t') else {
            continue;
        };
        let Ok(size) = size.parse::<u64>() else {
            continue;
        };
        entries.push((root.join(relative), size));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_stored_under_their_root_and_read_back() {
        let root = std::env::temp_dir().join(format!("spit-journal-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("t000")).unwrap();
        let journal = Journal::open(&root).unwrap();
        journal.record(&root.join("t000/a.txt"), 10).unwrap();
        journal.record(&root.join("t000/b.txt"), 20).unwrap();
        // The file is in the root, and its paths are relative to it.
        let text = std::fs::read_to_string(root.join(FILE)).unwrap();
        assert_eq!(text, "10\tt000/a.txt\n20\tt000/b.txt\n");
        let back = read_tail(&root).unwrap();
        assert_eq!(
            back,
            vec![(root.join("t000/a.txt"), 10), (root.join("t000/b.txt"), 20)]
        );
        // A rewritten path keeps only its newest entry.
        journal.record(&root.join("t000/a.txt"), 30).unwrap();
        let back = read_tail(&root).unwrap();
        assert_eq!(back.last(), Some(&(root.join("t000/a.txt"), 30)));
        assert_eq!(back.len(), 2);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
