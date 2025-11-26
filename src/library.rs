use crate::metadata::SongMetadata;
use crate::utils::recurse_dir;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Library {
    songs: Vec<SongMetadata>,
    lookup: HashMap<String, Vec<usize>>,
}

impl Library {
    pub fn new(paths: Vec<PathBuf>, recursive: bool) -> Self {
        let mut files = Vec::new();

        for path in &paths {
            recurse_dir(path, &mut files, recursive);
        }

        let songs: Vec<SongMetadata> = files.par_iter().map(SongMetadata::from).collect();

        println!("Library loaded with {} songs.", songs.len());

        let mut lookup: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, song) in songs.iter().enumerate() {
            let key = SongMetadata::normalize_str(&song.title);
            if !key.is_empty() {
                lookup.entry(key).or_default().push(i);
            }
        }

        Library { songs, lookup }
    }

    pub fn songs(&self) -> &[SongMetadata] {
        &self.songs
    }

    pub fn find_song(&self, query: &SongMetadata) -> Option<&SongMetadata> {
        let title_key = SongMetadata::normalize_str(&query.title);

        if let Some(indices) = self.lookup.get(&title_key) {
            for &idx in indices {
                let candidate = &self.songs[idx];
                if candidate.matches_metadata(query) {
                    return Some(candidate);
                }
            }
        }
        None
    }
}
