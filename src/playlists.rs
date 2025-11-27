use crate::library::Library;
use crate::metadata::SongMetadata;
use csv::ReaderBuilder;
use log::{info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Playlist {
    pub name: String,
    pub songs: Vec<SongMetadata>,
    pub missing_songs: Vec<SongMetadata>,
}

impl Playlist {
    pub fn new(path: PathBuf) -> Self {
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_path(&path)
            .expect("Unable to open CSV");

        let mut songs = Vec::new();
        for record in reader.deserialize::<BasicTrackInfo>().flatten() {
            songs.push(record.into());
        }

        Playlist {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            songs,
            missing_songs: Vec::new(),
        }
    }

    pub fn save_to_m3u(&self, output_dir: &Path) {
        let m3u_path = output_dir.join(format!("{}.m3u8", self.name));
        if let Ok(mut m3u_file) = std::fs::File::create(&m3u_path) {
            let _ = writeln!(m3u_file, "#EXTM3U");
            for song in &self.songs {
                if let Some(path) = &song.file_path {
                    let _ = writeln!(m3u_file, "{}", path.to_string_lossy());
                }
            }
            info!("Saved playlist: {:?}", m3u_path);
        }
    }

    pub fn filter_and_complete_from_library(&mut self, library: &Library) {
        let mut completed_songs = Vec::with_capacity(self.songs.len());

        info!("Processing playlist: {}", self.name);

        for csv_song in &self.songs {
            match library.find_song(csv_song) {
                Some(full_song) => completed_songs.push(full_song.clone()),
                None => self.missing_songs.push(csv_song.clone()),
            }
        }

        self.songs = completed_songs;
    }
}

/// Generates a report of songs missing from the library across all playlists.
pub fn generate_missing_report(playlists: &[Playlist], output_dir: &Path) {
    let mut missing_counts = HashMap::new();
    let mut missing_artists = HashMap::new();

    for pl in playlists {
        for song in &pl.missing_songs {
            *missing_counts.entry(String::from(song)).or_insert(0) += 1;
            if let Some(artist) = &song.artist {
                *missing_artists.entry(artist.clone()).or_insert(0) += 1;
            }
        }
    }

    if missing_counts.is_empty() {
        return;
    }

    let report_path = output_dir.join("missing_report.log");
    if let Ok(mut file) = std::fs::File::create(&report_path) {
        writeln!(file, "--- MISSING SONGS REPORT ---").unwrap();

        let mut sorted_songs: Vec<_> = missing_counts.iter().collect();
        sorted_songs.sort_by(|a, b| b.1.cmp(a.1));

        for (song, count) in sorted_songs {
            writeln!(file, "[{}] {}", count, song).unwrap();
        }

        writeln!(file, "\n--- MISSING ARTISTS SUMMARY ---").unwrap();
        let mut sorted_artists: Vec<_> = missing_artists.iter().collect();
        sorted_artists.sort_by(|a, b| b.1.cmp(a.1));

        for (artist, count) in sorted_artists {
            writeln!(file, "[{}] {}", count, artist).unwrap();
        }

        warn!("Missing songs report saved to {:?}", report_path);
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct BasicTrackInfo {
    #[serde(rename = "Track Name")]
    pub track_name: String,

    #[serde(rename = "Album Name")]
    pub album_name: String,

    #[serde(rename = "Artist Name(s)")]
    pub artist_names: String,
}
