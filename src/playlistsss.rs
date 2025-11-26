use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::metadata::SongMetadata;

// Mapped exactly to your CSV headers
#[derive(Debug, Deserialize)]
struct SpotifyRow {
    #[serde(alias = "Track Name")]
    track_name: String,
    #[serde(alias = "Artist Name(s)")]
    artist_names: String,
    #[serde(alias = "Album Name")]
    album_name: String,
}

// Key for looking up songs: (Artist, Title)
// We use a Vec<LibraryTrack> because you might have the same song on 2 different albums
type LibraryIndex = HashMap<(String, String), Vec<SongMetadata>>;

fn main() -> Result<()> {
    // --- CONFIGURATION ---
    let csv_dir = "./playlists"; // Your CSV folder
    let music_lib_dir = "./music"; // Your Music folder
    let output_dir = "./playlists_m3u"; // Output folder
    // ---------------------

    fs::create_dir_all(output_dir)?;

    println!("Step 1: Indexing library from '{}'...", music_lib_dir);
    let library = index_library(music_lib_dir)?;
    println!("Found {} unique song titles in library.", library.len());

    println!("Step 2: Converting CSVs...");
    let paths = fs::read_dir(csv_dir).context("Could not read CSV directory")?;

    for path in paths {
        let path = path?.path();
        if path.extension().map_or(false, |e| e == "csv") {
            process_single_csv(&path, output_dir, &library)?;
        }
    }

    println!("All done.");
    Ok(())
}

fn index_library(root: &str) -> Result<LibraryIndex> {
    let mut index: LibraryIndex = HashMap::new();

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            // Probe the file for metadata
            if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
                if let Some(tag) = tagged_file.primary_tag() {
                    let title = tag.title().unwrap_or("").to_string();
                    let artist = tag.artist().unwrap_or("").to_string();
                    let album = tag.album().unwrap_or("").to_string();

                    if !title.is_empty() && !artist.is_empty() {
                        let key = (normalize(&artist), normalize(&title));

                        // Canonicalize path (absolute path)
                        if let Ok(abs_path) = fs::canonicalize(path) {
                            let track = LibraryTrack {
                                path: abs_path,
                                album: normalize(&album), // Store normalized album for comparison
                            };

                            // Insert into the vector for this song
                            index.entry(key).or_default().push(track);
                        }
                    }
                }
            }
        }
    }
    Ok(index)
}

fn process_single_csv(csv_path: &Path, output_dir: &str, library: &LibraryIndex) -> Result<()> {
    let file_stem = csv_path.file_stem().unwrap().to_str().unwrap();
    println!("  -> Processing: {}", file_stem);

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) // Trim whitespace around CSV fields
        .from_path(csv_path)?;

    let output_filename = format!("{}.m3u", file_stem);
    let output_path = Path::new(output_dir).join(output_filename);
    let mut m3u_file = File::create(output_path)?;

    // M3U Header
    writeln!(m3u_file, "#EXTM3U")?;

    let mut count = 0;

    for result in rdr.deserialize() {
        let row: SpotifyRow = match result {
            Ok(r) => r,
            Err(_) => continue, // Skip malformed rows
        };

        // 1. Prepare search keys
        let target_title = normalize(&row.track_name);
        let target_album = normalize(&row.album_name);

        // Spotify CSVs often have "Artist A; Artist B".
        // We try matching the full string, then just the first artist.
        let full_artist = normalize(&row.artist_names);
        let first_artist = row
            .artist_names
            .split(';')
            .next()
            .map(normalize)
            .unwrap_or_default();

        // 2. Look up in Library
        // Check Full Artist first, then First Artist
        let candidates = library
            .get(&(full_artist.clone(), target_title.clone()))
            .or_else(|| library.get(&(first_artist, target_title)));

        if let Some(tracks) = candidates {
            // 3. ALBUM PRIORITY LOGIC
            // We have a list of files that match Artist + Title.
            // Try to find one where the Album also matches.
            let best_match = tracks
                .iter()
                .find(|t| t.album == target_album) // Priority: Same Album
                .or_else(|| tracks.first()); // Fallback: Any version of the song

            if let Some(track) = best_match {
                // Write #EXTINF for display purposes
                writeln!(
                    m3u_file,
                    "#EXTINF:-1,{} - {}",
                    row.artist_names, row.track_name
                )?;
                writeln!(m3u_file, "{}", track.path.display())?;
                count += 1;
            }
        }
    }

    println!("     Matches found: {}", count);
    Ok(())
}

fn normalize(input: &str) -> String {
    input.trim().to_lowercase()
}
