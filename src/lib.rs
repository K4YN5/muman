use crate::fs::Cache;

const ALLOWED_EXTENSIONS: &[&str] = &["flac"];

mod album;
mod artist;
mod fs;
mod library;
mod track;

pub fn run() {
    let library =
        library::DirtyLibrary::new(std::path::PathBuf::from("./tests/songs/"), Cache::new());
    for track in &library.tracks {
        println!("{:?}", track);
    }
    println!("Total tracks found: {}", library.tracks.len());
}
