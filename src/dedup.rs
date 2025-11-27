use crate::library::Library;
use crate::metadata::SongMetadata;
use log::{error, info, warn};
use std::collections::HashMap;
use std::path::Path;

/// Orchestrates the deduplication process.
pub fn run(library: &Library, dry_run: bool) {
    let all_songs = library.get_all_songs();

    // 1. Build an Album Frequency Map
    // This is purely structural: "Does this album name appear 15 times or 1 time?"
    // Used ONLY to prefer keeping files that are part of a larger set.
    let mut album_counts: HashMap<String, usize> = HashMap::new();
    for song in &all_songs {
        if let Some(ref album) = song.album {
            let norm_album = SongMetadata::normalize_str(&Some(album.clone()));
            *album_counts.entry(norm_album).or_default() += 1;
        }
    }

    // 2. Group songs by (Title, Artist)
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
        process_duplicate_group(dupes, &album_counts, title, artist, dry_run);
    }
}

fn process_duplicate_group(
    dupes: &[&SongMetadata],
    album_counts: &HashMap<String, usize>,
    title: &str,
    artist: &str,
    dry_run: bool,
) {
    // 3. Cluster by "Content Identity"
    // We might have 4 files with the same name, but they might be 2 pairs of different recordings.
    let mut content_buckets: Vec<Vec<&SongMetadata>> = Vec::new();

    for song in dupes {
        let mut placed = false;
        for bucket in &mut content_buckets {
            // Compare this song against the first song in the bucket
            if is_same_content(bucket[0], song) {
                bucket.push(song);
                placed = true;
                break;
            }
        }
        if !placed {
            content_buckets.push(vec![song]);
        }
    }

    // 4. Resolve Conflicts
    for bucket in content_buckets {
        // If a bucket has > 1 item, those are PHYSICALLY identical (or extremely close).
        // We only need one of them.
        if bucket.len() > 1 {
            resolve_conflict(&bucket, album_counts, title, artist, dry_run);
        }
    }
}

/// Strict objective check to see if audio is likely identical.
fn is_same_content(a: &SongMetadata, b: &SongMetadata) -> bool {
    // A. Strict ISRC Match
    if let (Some(isrc_a), Some(isrc_b)) = (&a.isrc, &b.isrc) {
        // If both have ISRC and they differ, they are 100% different.
        if isrc_a != isrc_b {
            return false;
        }
        // If they match, they are duplicates.
        return true;
    }

    // B. File Size Heuristic (if ISRC is missing on one or both)
    if let (Some(path_a), Some(path_b)) = (&a.file_path, &b.file_path) {
        if let (Ok(meta_a), Ok(meta_b)) = (std::fs::metadata(path_a), std::fs::metadata(path_b)) {
            let size_a = meta_a.len() as f64;
            let size_b = meta_b.len() as f64;

            if size_a == 0.0 || size_b == 0.0 {
                return false;
            }

            // Calculate percentage difference
            let diff = (size_a - size_b).abs();
            let avg = (size_a + size_b) / 2.0;
            let percent_diff = diff / avg;

            // If sizes are within 5% of each other, assume same audio.
            // (e.g. 30MB vs 31MB = Same. 30MB vs 45MB = Different).
            return percent_diff < 0.05;
        }
    }

    // Default to FALSE (Keep safe) if we can't determine.
    false
}

fn resolve_conflict(
    candidates: &[&SongMetadata],
    album_counts: &HashMap<String, usize>,
    title: &str,
    artist: &str,
    dry_run: bool,
) {
    let mut sorted = candidates.to_vec();

    // Tie-Breaker Logic:
    // We have determined these files contain the SAME audio. Which file do we keep?
    sorted.sort_by(|a, b| {
        // 1. Prefer the file that belongs to a "Full Album" over a "Single" folder.
        // We don't want to leave a hole in a 15-track album just to keep a loose single.
        let score_a = get_album_structure_score(a, album_counts);
        let score_b = get_album_structure_score(b, album_counts);

        match score_b.cmp(&score_a) {
            std::cmp::Ordering::Equal => {
                // 2. Prefer Metadata (Has ISRC?)
                let a_has = a.isrc.is_some();
                let b_has = b.isrc.is_some();
                match b_has.cmp(&a_has) {
                    std::cmp::Ordering::Equal => {
                        // 3. Final Deterministic Fallback (Alphabetical Path)
                        a.file_path.cmp(&b.file_path)
                    }
                    other => other,
                }
            }
            other => other,
        }
    });

    // Determine Winner
    if let Some(winner) = sorted.first() {
        info!(
            "Found {} true duplicates for: '{}' by '{}'",
            candidates.len(),
            title,
            artist
        );
        info!(
            "  KEEPING: {:?} (Album: {:?})",
            winner
                .file_path
                .as_ref()
                .map(|p| p.file_name().unwrap_or_default()),
            winner.album.as_deref().unwrap_or("Unknown")
        );

        // Delete the rest
        for loser in &sorted[1..] {
            remove_song_file(loser, dry_run);
        }
    }
}

/// Returns a structural score based on library context.
/// 2 = Likely a Multi-track Album (Count > 2)
/// 1 = Likely a Single (Count <= 2)
/// 0 = No Album info
fn get_album_structure_score(song: &SongMetadata, album_counts: &HashMap<String, usize>) -> u8 {
    if let Some(ref album) = song.album {
        let norm = SongMetadata::normalize_str(&Some(album.clone()));
        let count = *album_counts.get(&norm).unwrap_or(&0);

        if count > 2 {
            return 2;
        }
        return 1;
    }
    0
}

fn remove_song_file(song: &SongMetadata, dry_run: bool) {
    let path = match &song.file_path {
        Some(p) => p,
        None => return,
    };

    info!(
        "  REMOVING: {:?} (Album: {:?})",
        path.file_name().unwrap_or_default(),
        song.album.as_deref().unwrap_or("Unknown")
    );

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
                let _ = std::fs::remove_dir(parent_dir);
            }
        }
    }
}
