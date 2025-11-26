use crate::library::Library;
use crate::metadata::SongMetadata;
use csv::ReaderBuilder;
use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Playlist {
    pub name: String,
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
        }
    }

    pub fn save_to_m3u(&self, output_dir: &Path) {
        let m3u_path = output_dir.join(format!("{}.m3u8", self.name));
        if let Ok(mut m3u_file) = std::fs::File::create(m3u_path) {
            let _ = writeln!(m3u_file, "#EXTM3U");
            for song in &self.songs {
                if let Some(path) = &song.file_path {
                    let _ = writeln!(m3u_file, "{}", path.to_string_lossy());
                }
            }
        }
    }

    pub fn filter_and_complete_from_library(&mut self, library: &Library) {
        let mut completed_songs = Vec::with_capacity(self.songs.len());
        let mut missing_songs = Vec::new();

        println!("Matching {} songs against library...", self.songs.len());

        for csv_song in &self.songs {
            match library.find_song(csv_song) {
                Some(full_song) => completed_songs.push(full_song.clone()),
                None => missing_songs.push(csv_song),
            }
        }

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
