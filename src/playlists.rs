use csv::ReaderBuilder;
use serde::Deserialize;
use std::path::PathBuf;

use crate::{libman::Library, metadata::SongMetadata};

pub struct Playlist {
    pub name: String,
    path: PathBuf,
    pub songs: Vec<SongMetadata>,
}

impl Playlist {
    pub fn new(path: PathBuf) -> Self {
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_path(&path)
            .unwrap();

        let mut songs = Vec::new();

        for result in reader.deserialize::<BasicTrackInfo>() {
            match result {
                Ok(record) => {
                    let meta: SongMetadata = record.into();
                    songs.push(meta);
                }
                Err(e) => {
                    eprintln!("Warning: Skipping invalid row in {:?}: {}", path, e);
                }
            }
        }

        Playlist {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            path,
            songs,
        }
    }
    pub fn save_to_m3u(&self, output_dir: &PathBuf) {
        let m3u_path = output_dir.join(format!("{}.m3u8", self.name));
        let mut m3u_file = std::fs::File::create(m3u_path).unwrap();

        use std::io::Write;
        writeln!(m3u_file, "#EXTM3U").unwrap();

        for song in &self.songs {
            if let Some(path) = &song.file_path {
                writeln!(m3u_file, "{}", path.to_string()).unwrap();
            }
        }
    }

    /// Filter the songs on the playlist to only include those present in the library.
    /// Replaces the sparse CSV metadata with the full metadata (file paths) from the library.
    pub fn filter_and_complete_from_library(&mut self, library: &Library) {
        let mut completed_songs = Vec::new();
        let mut missing_songs = Vec::new();

        println!("Matching {} songs against library...", self.songs.len());

        for csv_song in &self.songs {
            // Find the first matching song in the library
            let match_found = library
                .songs()
                .iter()
                .find(|lib_song| lib_song.matches_metadata(csv_song));

            match match_found {
                Some(full_song) => {
                    // We found it! Add the library version (with file_path) to our new list
                    completed_songs.push(full_song.clone());
                }
                None => {
                    // Log it so the user knows what's missing
                    missing_songs.push(csv_song);
                }
            }
        }

        // Feedback to user
        if !missing_songs.is_empty() {
            println!("---------------------------------------------------");
            println!(
                "⚠️  Could not find {} songs in your library:",
                missing_songs.len()
            );
            for s in &missing_songs {
                println!(
                    "   ❌ {} - {}",
                    s.artist.as_deref().unwrap_or("Unknown"),
                    s.title.as_deref().unwrap_or("Unknown")
                );
            }
            println!("---------------------------------------------------");
        } else {
            println!("✅ All songs matched successfully!");
        }

        // Update the playlist with only the valid, completed songs
        self.songs = completed_songs;
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
