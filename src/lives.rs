use crate::library::Library;
use crate::metadata::SongMetadata;
use log::{error, info};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

struct LiveAlbum {
    name: String,
    paths: Vec<PathBuf>,
}

pub fn run(library: &Library, dry_run: bool) {
    let all_songs = library.get_all_songs();
    let mut artists: HashMap<String, Vec<LiveAlbum>> = HashMap::new();

    // 1. Filter and Group Live Albums
    // We group by Artist -> List of Albums
    let mut temp_grouping: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();

    for song in all_songs {
        if let (Some(artist), Some(album), Some(path)) =
            (&song.artist, &song.album, &song.file_path)
        {
            if is_live_album(album) {
                let artist_norm = SongMetadata::normalize_str(&Some(artist.clone()));
                let album_norm = SongMetadata::normalize_str(&Some(album.clone())); // Normalize for grouping key

                // Store using original names for display, normalized for grouping
                let key = (artist_norm, album_norm);

                // We need to store the Display Name of the album somewhere.
                // For simplicity in this loop, we'll re-attach it later.
                temp_grouping.entry(key).or_default().push(path.clone());
            }
        }
    }

    // Convert flat list to structured hierarchy
    // We need to fetch the original Album Name for display purposes
    // (We accept that we might pick the casing from the first track found)
    for ((artist_norm, _), paths) in temp_grouping {
        // Find a "Nice" display name from the library for this album
        // (This is a bit expensive but UI needs to look good)
        // Since we don't have direct access to the song struct here easily without re-querying,
        // we'll just assume the user knows what "live at donington" means even if lowercase.
        // OR we can rely on the path's parent folder name usually.

        let display_name = paths[0]
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown Album".to_string());

        let entry = LiveAlbum {
            name: display_name,
            paths,
        };

        artists.entry(artist_norm).or_default().push(entry);
    }

    // 2. Interactive Process
    let mut sorted_artists: Vec<_> = artists.into_iter().collect();
    sorted_artists.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by artist name

    for (artist_key, albums) in sorted_artists {
        // Try to get a nicer artist name for display (first letter uppercase heuristic)
        let display_artist = titlecase(&artist_key);

        if albums.len() == 1 {
            process_single_live(&display_artist, &albums[0], dry_run);
        } else if albums.len() > 1 {
            process_multi_live(&display_artist, albums, dry_run);
        }
    }
}

fn process_single_live(artist: &str, album: &LiveAlbum, dry_run: bool) {
    println!("\n🎸 Artist: {}", artist);
    println!("   Found 1 Live Album: '{}'", album.name);

    if !dry_run {
        print!("   Delete this album? [y/N]: ");
        if read_yes_no() {
            delete_album(album, dry_run);
        }
    } else {
        info!("[Dry Run] Would ask to delete '{}'", album.name);
    }
}

fn process_multi_live(artist: &str, albums: Vec<LiveAlbum>, dry_run: bool) {
    println!("\n🎸 Artist: {} ({} Live Albums)", artist, albums.len());
    for (i, album) in albums.iter().enumerate() {
        println!("   {}. {}", i + 1, album.name);
    }

    if dry_run {
        info!("[Dry Run] Would show menu for {}", artist);
        return;
    }

    println!("   -------------------------------------------------");
    print!("   [k]Keep All | [d]Delete All | [s]Select One-by-One | [q]Quit: ");

    match read_char() {
        'd' => {
            println!("   Deleting ALL live albums for {}...", artist);
            for album in albums {
                delete_album(&album, dry_run);
            }
        }
        's' => {
            for album in albums {
                print!("   Delete '{}'? [y/N]: ", album.name);
                if read_yes_no() {
                    delete_album(&album, dry_run);
                }
            }
        }
        'q' => {
            println!("Bye!");
            std::process::exit(0);
        }
        _ => println!("   Keeping all."),
    }
}

// --- Helpers ---

fn is_live_album(album: &str) -> bool {
    let s = album.to_lowercase();
    // Keywords that strongly suggest a live album
    s.contains("live") || 
    s.contains("concert") || 
    s.contains("tour") || 
    s.contains("performance") || 
    s.contains("unplugged") || 
    s.contains("sessions") || 
    s.contains("bbc") ||
    s.contains(" at the ") || // e.g. "Live at the Apollo"
    s.contains(" in ") // e.g. "Live in Paris" (Weak check, relies on "Live" usually)
}

fn delete_album(album: &LiveAlbum, dry_run: bool) {
    info!("Deleting Live Album: {}", album.name);
    for path in &album.paths {
        if !dry_run {
            if let Err(e) = std::fs::remove_file(path) {
                error!("Failed to remove file: {}", e);
            }
        } else {
            info!("  [Dry Run] rm {:?}", path);
        }
    }

    // Clean up empty dirs
    if !dry_run {
        if let Some(first_path) = album.paths.first() {
            if let Some(parent) = first_path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

fn titlecase(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn read_yes_no() -> bool {
    let _ = io::stdout().flush();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let s = input.trim().to_lowercase();
    s == "y" || s == "yes"
}

fn read_char() -> char {
    let _ = io::stdout().flush();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_lowercase().chars().next().unwrap_or(' ')
}
