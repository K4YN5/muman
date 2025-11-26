pub fn recurse_dir(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>, recursive: bool) {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() && recursive {
            recurse_dir(&path, files, recursive);
        }
    }
}
