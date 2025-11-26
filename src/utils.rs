use std::path::{Path, PathBuf};

pub fn recurse_dir(path: &Path, files: &mut Vec<PathBuf>, recursive: bool) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() && recursive {
                recurse_dir(&path, files, recursive);
            }
        }
    }
}
