use crate::metadata::SongMetadata;
use crate::utils::recurse_dir;
use log::info;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// Manages the collection of songs and provides search capabilities.
pub struct Library {
    songs: HashMap<String, Vec<SongMetadata>>,
}

const SUPPORTED_EXTENSIONS: [&str; 6] = ["flac", "mp3", "wav", "m4a", "ogg", "aac"];

impl Library {
    /// Scans the given path for audio files and builds the library index.
    pub fn new(path: PathBuf, recursive: bool) -> Self {
        let mut files = Vec::new();

        recurse_dir(&path, &mut files, recursive);

        // Index by Title Only to solve the matching issue
        let songs: HashMap<String, Vec<SongMetadata>> = files
            .par_iter()
            .filter(|file_path| {
                file_path.extension().is_some_and(|ext| {
                    SUPPORTED_EXTENSIONS
                        .iter()
                        .any(|&s| ext.eq_ignore_ascii_case(s))
                })
            })
            .map(|file_path| {
                let song = SongMetadata::from(file_path);
                let title_key = SongMetadata::normalize_str(&song.title);
                (title_key, song)
            })
            .fold(HashMap::new, |mut acc, (key, song)| {
                acc.entry(key).or_insert_with(Vec::new).push(song);
                acc
            })
            .reduce(HashMap::new, |mut acc, map| {
                for (key, mut songs) in map {
                    acc.entry(key).or_insert_with(Vec::new).append(&mut songs);
                }
                acc
            });

        let total_files: usize = songs.values().map(|v| v.len()).sum();
        info!(
            "Library loaded with {} unique titles and {} total files.",
            songs.len(),
            total_files
        );

        Library { songs }
    }

    /// Returns a flat vector of all songs in the library.
    pub fn get_all_songs(&self) -> Vec<SongMetadata> {
        self.songs.values().flatten().cloned().collect()
    }

    /// Attempts to find a song in the library matching the query metadata.
    pub fn find_song(&self, query: &SongMetadata) -> Option<&SongMetadata> {
        let title_key = SongMetadata::normalize_str(&query.title);

        // 1. First lookup by normalized title
        if let Some(candidates) = self.songs.get(&title_key) {
            let query_artist = SongMetadata::normalize_str(&query.artist);

            // 2. If the query has an artist, try to fuzzy match it
            if !query_artist.is_empty() {
                for song in candidates {
                    let song_artist = SongMetadata::normalize_str(&song.artist);
                    // Check for exact match or substring match (e.g. "feat." handling)
                    if song_artist == query_artist
                        || (!song_artist.is_empty() && query_artist.contains(&song_artist))
                        || (!query_artist.is_empty() && song_artist.contains(&query_artist))
                    {
                        return Some(song);
                    }
                }
            }

            // 3. Fallback: If no artist provided or no match, but only one result exists, use it.
            if candidates.len() == 1 {
                return Some(&candidates[0]);
            }
        }
        None
    }
}
