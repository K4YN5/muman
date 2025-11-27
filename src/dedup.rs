use crate::library::Library;
use crate::metadata::SongMetadata;
use log::{error, info};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// --- Structures for Hierarchical Analysis ---

struct ArtistEntry {
    name: String,
    albums: HashMap<String, AlbumEntry>, // Key: Normalized Album Name
}

struct AlbumEntry {
    name: String, // Original Album Name
    songs: Vec<SongMetadata>,
}

#[derive(Debug)]
enum AlbumRelation {
    Identical,      // A and B have the Exact Same songs
    Subset,         // A is completely contained inside B (B is Deluxe)
    Superset,       // A contains all of B plus more (A is Deluxe)
    PartialOverlap, // Share some songs, but both have unique tracks
    Disjoint,       // No shared songs (shouldn't happen in this logic flow)
}

// --- Main Entry Point ---

pub fn run(library: &Library, dry_run: bool, use_hard_links: bool) {
    let all_songs = library.get_all_songs();
    let mut artists: HashMap<String, ArtistEntry> = HashMap::new();

    // 1. Build Hierarchy: Artist -> Album -> Songs
    for song in all_songs {
        let artist_norm = SongMetadata::normalize_str(&song.artist);
        let album_norm = SongMetadata::normalize_str(&song.album);

        // Skip songs with missing metadata
        if artist_norm.is_empty() || album_norm.is_empty() {
            continue;
        }

        let artist_entry = artists
            .entry(artist_norm.clone())
            .or_insert_with(|| ArtistEntry {
                name: song.artist.clone().unwrap_or(artist_norm),
                albums: HashMap::new(),
            });

        let album_entry = artist_entry
            .albums
            .entry(album_norm.clone())
            .or_insert_with(|| AlbumEntry {
                name: song.album.clone().unwrap_or(album_norm),
                songs: Vec::new(),
            });

        album_entry.songs.push(song);
    }

    // 2. Process per Artist
    for (artist_name, mut artist_data) in artists {
        info!("Analyzing artist: {}", artist_data.name);

        // A. Handle Singles (Case 1) - Automated
        remove_redundant_singles(&mut artist_data, dry_run);

        // B. Handle Album Duplicates (Cases 2, 3, 4) - Interactive
        process_albums(&artist_data, dry_run, use_hard_links);
    }
}

// --- Logic: Singles ---

fn remove_redundant_singles(artist: &mut ArtistEntry, dry_run: bool) {
    // Identify potential "Singles" albums (1-2 tracks, Album Name ~= Track Name)
    let mut singles_to_remove: Vec<PathBuf> = Vec::new();
    let mut albums_to_check: Vec<String> = artist.albums.keys().cloned().collect();

    for album_key in &albums_to_check {
        if let Some(album) = artist.albums.get(album_key) {
            // Heuristic: It's a single if <= 2 songs
            if album.songs.len() <= 2 {
                for song in &album.songs {
                    // Check if this song exists in ANY other "Main" album (more than 2 tracks)
                    if let Some(parent_album) = find_song_in_other_albums(song, artist, album_key) {
                        info!(
                            "Found Single '{:?}' included in Album '{:?}'",
                            song.title, parent_album
                        );
                        if let Some(p) = &song.file_path {
                            singles_to_remove.push(p.clone());
                        }
                    }
                }
            }
        }
    }

    // Execute Deletion
    for path in singles_to_remove {
        delete_file(&path, dry_run);
    }
}

fn find_song_in_other_albums<'a>(
    query: &SongMetadata,
    artist: &'a ArtistEntry,
    exclude_album_key: &str,
) -> Option<&'a str> {
    for (key, album) in &artist.albums {
        if key == exclude_album_key || album.songs.len() <= 2 {
            continue;
        }

        for album_song in &album.songs {
            if is_same_song(query, album_song) {
                return Some(&album.name);
            }
        }
    }
    None
}

// --- Logic: Albums ---

fn process_albums(artist: &ArtistEntry, dry_run: bool, use_hard_links: bool) {
    let album_keys: Vec<&String> = artist.albums.keys().collect();
    let mut processed_pairs: HashSet<(String, String)> = HashSet::new();

    // Compare every album against every other album
    for i in 0..album_keys.len() {
        for j in (i + 1)..album_keys.len() {
            let key_a = album_keys[i];
            let key_b = album_keys[j];

            // Avoid re-processing
            if processed_pairs.contains(&(key_a.clone(), key_b.clone())) {
                continue;
            }
            processed_pairs.insert((key_a.clone(), key_b.clone()));

            let album_a = &artist.albums[key_a];
            let album_b = &artist.albums[key_b];

            let relation = compare_albums(album_a, album_b);

            match relation.relation {
                AlbumRelation::Disjoint => continue, // No relation, ignore

                AlbumRelation::Identical => {
                    println!("\n---------------------------------------------------------");
                    println!("⚠️  DUPLICATE ALBUMS DETECTED: '{}'", artist.name);
                    println!("1. '{}' ({} songs)", album_a.name, album_a.songs.len());
                    println!("2. '{}' ({} songs)", album_b.name, album_b.songs.len());
                    println!("   (Both albums contain the exact same song set)");

                    if !dry_run {
                        print!("Select option [0=Keep Both, 1=Delete 1st, 2=Delete 2nd]: ");
                        match read_user_input() {
                            1 => delete_album(album_a, dry_run),
                            2 => delete_album(album_b, dry_run),
                            _ => {
                                println!("Keeping both.");
                                if use_hard_links {
                                    try_hard_link_albums(album_a, album_b, dry_run);
                                }
                            }
                        }
                    } else {
                        info!("[Dry Run] Would prompt user to resolve identical albums.");
                    }
                }

                AlbumRelation::Subset => {
                    // A is inside B
                    println!("\n---------------------------------------------------------");
                    println!("⚠️  REDUNDANT ALBUM DETECTED: '{}'", artist.name);
                    println!(
                        "   Album '{0}' is completely included in '{1}'",
                        album_a.name, album_b.name
                    );
                    println!("   '{0}' has {1} songs.", album_a.name, album_a.songs.len());
                    println!("   '{0}' has {1} songs.", album_b.name, album_b.songs.len());

                    if !dry_run {
                        print!("Delete subset album '{}'? [y/N]: ", album_a.name);
                        if read_yes_no() {
                            delete_album(album_a, dry_run);
                        }
                    } else {
                        info!("[Dry Run] Would prompt to delete subset '{}'", album_a.name);
                    }
                }

                AlbumRelation::Superset => {
                    // B is inside A
                    println!("\n---------------------------------------------------------");
                    println!("⚠️  REDUNDANT ALBUM DETECTED: '{}'", artist.name);
                    println!(
                        "   Album '{1}' is completely included in '{0}'",
                        album_a.name, album_b.name
                    );

                    if !dry_run {
                        print!("Delete subset album '{}'? [y/N]: ", album_b.name);
                        if read_yes_no() {
                            delete_album(album_b, dry_run);
                        }
                    } else {
                        info!("[Dry Run] Would prompt to delete subset '{}'", album_b.name);
                    }
                }

                AlbumRelation::PartialOverlap => {
                    // Case 4: Keep everything, optionally hard link
                    if use_hard_links {
                        info!(
                            "Partial overlap between '{}' and '{}'. Attempting hard links for shared songs...",
                            album_a.name, album_b.name
                        );
                        try_hard_link_albums(album_a, album_b, dry_run);
                    }
                }
            }
        }
    }
}

struct ComparisonData {
    relation: AlbumRelation,
}

fn compare_albums(a: &AlbumEntry, b: &AlbumEntry) -> ComparisonData {
    let mut matches = 0;

    for song_a in &a.songs {
        for song_b in &b.songs {
            if is_same_song(song_a, song_b) {
                matches += 1;
                break;
            }
        }
    }

    let a_len = a.songs.len();
    let b_len = b.songs.len();

    let relation = if matches == 0 {
        AlbumRelation::Disjoint
    } else if matches == a_len && matches == b_len {
        AlbumRelation::Identical
    } else if matches == a_len && matches < b_len {
        AlbumRelation::Subset // A is inside B
    } else if matches == b_len && matches < a_len {
        AlbumRelation::Superset // B is inside A
    } else {
        AlbumRelation::PartialOverlap
    };

    ComparisonData { relation }
}

// --- Hard Linking Logic ---

fn try_hard_link_albums(a: &AlbumEntry, b: &AlbumEntry, dry_run: bool) {
    for song_a in &a.songs {
        for song_b in &b.songs {
            if is_same_song(song_a, song_b) {
                // If they are physically different files, hard link them
                if let (Some(path_a), Some(path_b)) = (&song_a.file_path, &song_b.file_path) {
                    if path_a != path_b && !are_files_hard_linked(path_a, path_b) {
                        // Check if file sizes are identical (Prerequisite for safe hard linking logic)
                        if get_file_size(path_a) == get_file_size(path_b) {
                            hard_link_file(path_a, path_b, dry_run);
                        }
                    }
                }
            }
        }
    }
}

fn hard_link_file(src: &Path, target: &Path, dry_run: bool) {
    info!("Hard Linking: {:?} -> {:?}", target, src);
    if !dry_run {
        // To hard link A to B, we delete B and create a link from A to B's path
        // WARNING: This destroys the unique metadata in B (Album Name will become A's)
        if let Err(e) = std::fs::remove_file(target) {
            error!("Failed to remove target for linking: {}", e);
            return;
        }
        if let Err(e) = std::fs::hard_link(src, target) {
            error!("Failed to create hard link: {}", e);
        }
    }
}

#[cfg(unix)]
fn are_files_hard_linked(p1: &Path, p2: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    if let (Ok(m1), Ok(m2)) = (std::fs::metadata(p1), std::fs::metadata(p2)) {
        return m1.ino() == m2.ino() && m1.dev() == m2.dev();
    }
    false
}

#[cfg(not(unix))]
fn are_files_hard_linked(_: &Path, _: &Path) -> bool {
    false // Rough fallback for Windows
}

// --- Utilities ---

/// Strict check: ISRC OR (Size + Title). No Duration.
fn is_same_song(a: &SongMetadata, b: &SongMetadata) -> bool {
    // 1. ISRC Check
    if let (Some(isrc_a), Some(isrc_b)) = (&a.isrc, &b.isrc) {
        if isrc_a == isrc_b {
            return true;
        }
        // If ISRCs differ, they are distinct.
        if !isrc_a.is_empty() && !isrc_b.is_empty() {
            return false;
        }
    }

    // 2. Fallback: Title + File Size (within 1%)
    let title_a = SongMetadata::normalize_str(&a.title);
    let title_b = SongMetadata::normalize_str(&b.title);

    if title_a != title_b {
        return false;
    }

    if let (Some(path_a), Some(path_b)) = (&a.file_path, &b.file_path) {
        let s_a = get_file_size(path_a);
        let s_b = get_file_size(path_b);
        if s_a > 0 && s_b > 0 {
            let diff = (s_a as i64 - s_b as i64).abs() as u64;
            // Strict: Must be very close (1%) to be considered "Same Audio"
            return diff < (s_a / 100);
        }
    }

    false
}

fn get_file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn delete_album(album: &AlbumEntry, dry_run: bool) {
    info!("Deleting Album: {}", album.name);
    for song in &album.songs {
        if let Some(p) = &song.file_path {
            delete_file(p, dry_run);
        }
    }
}

fn delete_file(path: &Path, dry_run: bool) {
    info!("Deleting file: {:?}", path);
    if !dry_run {
        if let Err(e) = std::fs::remove_file(path) {
            error!("Error deleting {:?}: {}", path, e);
            return;
        }
        // Try cleaning up parent dir
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent); // Fails silently if not empty
        }
    }
}

fn read_user_input() -> usize {
    let _ = io::stdout().flush();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().parse().unwrap_or(0)
}

fn read_yes_no() -> bool {
    let _ = io::stdout().flush();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let s = input.trim().to_lowercase();
    s == "y" || s == "yes"
}
