use std::path::PathBuf;

use muman::recurse_dir;

use crate::metadata::SongMetadata;

pub struct Library {
    path: Vec<PathBuf>,
    songs: Vec<SongMetadata>,
}

impl Library {
    pub fn get_library(path: Vec<PathBuf>, recursive: bool) -> Self {
        let mut files = Vec::<std::path::PathBuf>::new();

        for path in &path {
            recurse_dir(path, &mut files, recursive);
        }

        let songs: Vec<SongMetadata> = files.iter().map(SongMetadata::from).collect();

        println!("Library loaded with {} songs.", songs.len());

        Library { songs, path }
    }

    pub fn songs(&self) -> &Vec<SongMetadata> {
        &self.songs
    }
}
