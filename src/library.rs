use crate::metadata::SongMetadata;
use crate::utils::recurse_dir;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Library {
    songs: HashMap<(String, String), Vec<SongMetadata>>,
}

const SUPPORTED_EXTENSIONS: [&str; 1] = ["flac"];

impl Library {
    pub fn new(path: PathBuf, recursive: bool) -> Self {
        let mut files = Vec::new();

        recurse_dir(&path, &mut files, recursive);

        let songs: HashMap<(String, String), Vec<SongMetadata>> = files
            .par_iter()
            .filter(|file_path| {
                file_path.extension().map_or(false, |ext| {
                    SUPPORTED_EXTENSIONS
                        .iter()
                        .any(|&s| ext.eq_ignore_ascii_case(s))
                })
            })
            .map(|file_path| {
                let song = SongMetadata::from(file_path);

                // normalize once
                let title_key = SongMetadata::normalize_str(&song.title);
                let artist_key = SongMetadata::normalize_str(&song.artist);

                (song, (title_key, artist_key))
            })
            .fold(
                || HashMap::new(),
                |mut acc: HashMap<(String, String), Vec<SongMetadata>>, (song, key)| {
                    let entry = acc.entry(key.clone()).or_insert_with(Vec::new);

                    entry.push(song);
                    acc
                },
            )
            .reduce(
                || HashMap::new(),
                |mut acc, map| {
                    for (key, mut songs) in map {
                        acc.entry(key).or_insert_with(Vec::new).append(&mut songs);
                    }
                    acc
                },
            );

        println!("Library loaded with {} songs.", songs.len());
        println!(
            "Total unique song entries in library: {}",
            songs.values().map(|v| v.len()).sum::<usize>()
        );

        Library { songs }
    }

    pub fn songs(&self) -> Vec<SongMetadata> {
        self.songs
            .values()
            .flat_map(|v| v.iter())
            .cloned()
            .collect::<Vec<SongMetadata>>()
    }

    pub fn find_song(&self, query: &SongMetadata) -> Option<&SongMetadata> {
        let keys: Vec<(String, String)> = if let Some(artist_field) = &query.artist {
            artist_field
                .split(';')
                .map(|artist| {
                    (
                        SongMetadata::normalize_str(&query.title),
                        SongMetadata::normalize_str(&Some(artist.to_string())),
                    )
                })
                .collect()
        } else {
            vec![(SongMetadata::normalize_str(&query.title), String::new())]
        };

        let mut results: Option<&SongMetadata> = None;

        for key in keys {
            if let Some(songs) = self.songs.get(&key) {
                results = songs.first();
                break;
            }
        }

        if results.is_none() {
            let title_key = (SongMetadata::normalize_str(&query.title), String::new());
            if let Some(songs) = self.songs.get(&title_key) {
                results = songs.first();
            }
        }

        if results.is_none() {
            eprintln!(
                "Song not found in library: artist='{:?}', title='{:?}'",
                query.artist, query.title
            );
        }
        results
    }
}
