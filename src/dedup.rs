use crate::library::Library;
use crate::metadata::SongMetadata;
use log::{error, info, warn};
use std::collections::HashMap;
use std::path::Path;

/// Orchestrates the deduplication process.
pub fn run(library: &Library, dry_run: bool) {
    let all_songs = library.get_all_songs();

    // 1. Build an Album Frequency Map
    // We count how many times each "Album Name" appears in the entire library.
    // If count > 1, it's a real album. If count == 1, it might be a single.
    let mut album_counts: HashMap<String, usize> = HashMap::new();
    for song in &all_songs {
        if let Some(ref album) = song.album {
            let norm_album = SongMetadata::normalize_str(&Some(album.clone()));
            *album_counts.entry(norm_album).or_default() += 1;
        }
    }

    // 2. Group songs by (Title, Artist) for duplicate detection
    let mut grouped: HashMap<(String, String), Vec<&SongMetadata>> = HashMap::new();
    for song in &all_songs {
        let key = (
            SongMetadata::normalize_str(&song.title),
            SongMetadata::normalize_str(&song.artist),
        );
        if !key.0.is_empty() {
            grouped.entry(key).or_default().push(song);
        }
    }

    for ((title, artist), dupes) in grouped.iter().filter(|(_, v)| v.len() > 1) {
        info!(
            "Found {} copies of: '{}' by '{}'",
            dupes.len(),
            title,
            artist
        );
        process_duplicate_group(dupes, &album_counts, dry_run);
    }
}

fn process_duplicate_group(
    dupes: &[&SongMetadata],
    album_counts: &HashMap<String, usize>,
    dry_run: bool,
) {
    let mut candidates = dupes.to_vec();

    // SORTING STRATEGY:
    // 1. Quality Score (Album Track > Single > Broken Metadata)
    // 2. ISRC Presence
    // 3. File Path (Tie-breaker)
    candidates.sort_by(|a, b| {
        let score_a = get_quality_score(a, album_counts);
        let score_b = get_quality_score(b, album_counts);

        match score_b.cmp(&score_a) {
            std::cmp::Ordering::Equal => {
                let a_has_isrc = a.isrc.is_some();
                let b_has_isrc = b.isrc.is_some();
                match b_has_isrc.cmp(&a_has_isrc) {
                    std::cmp::Ordering::Equal => a.file_path.cmp(&b.file_path),
                    other => other,
                }
            }
            other => other,
        }
    });

    if let Some(winner) = candidates.first() {
        info!(
            "  KEEPING: {:?} (Album: {:?})",
            winner
                .file_path
                .as_ref()
                .map(|p| p.file_name().unwrap_or_default()),
            winner.album.as_deref().unwrap_or("Unknown")
        );

        for loser in &candidates[1..] {
            remove_song_file(loser, dry_run);
        }
    } else {
        warn!("Duplicate group processed but found no candidates.");
    }
}

/// Calculus to determine the "value" of a track version.
/// 3 = Album Track (Belongs to an album with multiple songs OR Album Name != Title)
/// 2 = Single (Album Name == Title AND no other songs in that album)
/// 1 = Poor Metadata (Missing Album)
fn get_quality_score(song: &SongMetadata, album_counts: &HashMap<String, usize>) -> u8 {
    match (&song.album, &song.title) {
        (Some(album), Some(title)) => {
            let norm_album = SongMetadata::normalize_str(&Some(album.clone()));
            let norm_title = SongMetadata::normalize_str(&Some(title.clone()));

            // Check if this album exists elsewhere in the library
            let is_multi_track_album = *album_counts.get(&norm_album).unwrap_or(&0) > 1;

            if norm_album == norm_title && !is_multi_track_album {
                2 // It is a Single (Title matches Album, and it's the only one)
            } else {
                3 // It is an Album Track (Title != Album OR it's a Title Track of a full album)
            }
        }
        _ => 1, // Metadata is missing
    }
}

fn remove_song_file(song: &SongMetadata, dry_run: bool) {
    let path = match &song.file_path {
        Some(p) => p,
        None => return,
    };

    info!("  REMOVING: {:?}", path.file_name().unwrap_or_default());

    if !dry_run {
        if let Err(e) = std::fs::remove_file(path) {
            error!("Failed to remove file {:?}: {}", path, e);
            return;
        }
        cleanup_empty_dir(path.parent());
    }
}

fn cleanup_empty_dir(dir_opt: Option<&Path>) {
    if let Some(parent_dir) = dir_opt {
        if let Ok(entries) = std::fs::read_dir(parent_dir) {
            let has_other_files = entries.filter_map(|e| e.ok()).any(|e| {
                let p = e.path();
                if let Some(ext) = p.extension() {
                    let s = ext.to_string_lossy().to_lowercase();
                    matches!(s.as_str(), "mp3" | "flac" | "wav" | "m4a" | "aac" | "ogg")
                } else {
                    true
                }
            });

            if !has_other_files {
                if let Err(_) = std::fs::remove_dir(parent_dir) {
                    // Ignored
                } else {
                    info!("  CLEANED DIR: {:?}", parent_dir);
                }
            }
        }
    }
}
