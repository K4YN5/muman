use crate::library::Library;
use crate::metadata::SongMetadata;
use log::{error, info};
use std::collections::HashMap;
use std::path::Path;

pub fn run(library: &Library, dry_run: bool) {
    let all_songs = library.get_all_songs();

    // 1. Build Album Counts (for Single vs Album heuristic)
    let mut album_counts: HashMap<String, usize> = HashMap::new();
    for song in &all_songs {
        if let Some(ref album) = song.album {
            let norm_album = SongMetadata::normalize_str(&Some(album.clone()));
            *album_counts.entry(norm_album).or_default() += 1;
        }
    }

    // 2. Group primarily by Title + Artist
    let mut title_groups: HashMap<(String, String), Vec<&SongMetadata>> = HashMap::new();
    for song in &all_songs {
        let key = (
            SongMetadata::normalize_str(&song.title),
            SongMetadata::normalize_str(&song.artist),
        );
        if !key.0.is_empty() {
            title_groups.entry(key).or_default().push(song);
        }
    }

    for ((title, artist), potential_dupes) in title_groups.iter().filter(|(_, v)| v.len() > 1) {
        // 3. Sub-group by ISRC / Duration to avoid False Positives
        let valid_groups = separate_distinct_recordings(potential_dupes);

        for group in valid_groups {
            if group.len() > 1 {
                info!(
                    "Found {} true duplicates for: '{}' by '{}'",
                    group.len(),
                    title,
                    artist
                );
                process_duplicate_group(&group, &album_counts, dry_run);
            }
        }
    }
}

/// The core safety logic. splits a list of "Same Title" songs into
/// lists of "Actually the Same Audio" songs.
fn separate_distinct_recordings<'a>(songs: &[&'a SongMetadata]) -> Vec<Vec<&'a SongMetadata>> {
    let mut distinct_groups: Vec<Vec<&SongMetadata>> = Vec::new();

    for song in songs {
        let mut found_group = false;

        for group in &mut distinct_groups {
            let representative = group[0];

            if are_songs_identical(song, representative) {
                group.push(song);
                found_group = true;
                break;
            }
        }

        if !found_group {
            distinct_groups.push(vec![song]);
        }
    }

    distinct_groups
}

fn are_songs_identical(a: &SongMetadata, b: &SongMetadata) -> bool {
    // 1. CHECK ISRC (The Gold Standard)
    if let (Some(isrc_a), Some(isrc_b)) = (&a.isrc, &b.isrc) {
        // If ISRCs exist and differ, they are DIFFERENT recordings.
        if isrc_a != isrc_b {
            return false;
        }
        // If ISRCs match, they are the same.
        return true;
    }

    // 2. CHECK DURATION (The Fallback)
    // Only calculated if ISRC is missing on one or both.
    let dur_a = a.get_duration();
    let dur_b = b.get_duration();

    // If we fail to read duration (0), assume they are different to be safe.
    if dur_a == 0 || dur_b == 0 {
        return false;
    }

    // Tolerance of 3 seconds for silence padding differences
    if dur_a.abs_diff(dur_b) > 3 {
        return false;
    }

    // If duration matches (and no conflicting ISRC), assume Duplicate.
    true
}

fn process_duplicate_group(
    dupes: &[&SongMetadata],
    album_counts: &HashMap<String, usize>,
    dry_run: bool,
) {
    let mut candidates = dupes.to_vec();

    candidates.sort_by(|a, b| {
        let score_a = get_quality_score(a, album_counts);
        let score_b = get_quality_score(b, album_counts);

        match score_b.cmp(&score_a) {
            std::cmp::Ordering::Equal => {
                let a_has_isrc = a.isrc.is_some();
                let b_has_isrc = b.isrc.is_some();
                match b_has_isrc.cmp(&a_has_isrc) {
                    // Tie-breaker: Shortest Path length usually implies "Original Album"
                    // vs "Super Long Compilation Name"
                    std::cmp::Ordering::Equal => {
                        let path_a_len = a
                            .file_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().len())
                            .unwrap_or(usize::MAX);
                        let path_b_len = b
                            .file_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().len())
                            .unwrap_or(usize::MAX);
                        path_a_len.cmp(&path_b_len)
                    }
                    other => other,
                }
            }
            other => other,
        }
    });

    if let Some(winner) = candidates.first() {
        // Logging for user confidence
        let winner_album = winner.album.as_deref().unwrap_or("Unknown");
        let winner_filename = winner
            .file_path
            .as_ref()
            .map(|p| p.file_name().unwrap_or_default())
            .unwrap_or_default();

        info!(
            "  KEEPING: {:?} (Album: {:?})",
            winner_filename, winner_album
        );

        for loser in &candidates[1..] {
            remove_song_file(loser, dry_run);
        }
    }
}

fn get_quality_score(song: &SongMetadata, album_counts: &HashMap<String, usize>) -> i32 {
    let mut score = 0;

    if let (Some(album), Some(title)) = (&song.album, &song.title) {
        let norm_album = SongMetadata::normalize_str(&Some(album.clone()));
        let norm_title = SongMetadata::normalize_str(&Some(title.clone()));

        let is_multi_track_album = *album_counts.get(&norm_album).unwrap_or(&0) > 1;

        // Base Score: Album Track > Single
        if norm_album == norm_title && !is_multi_track_album {
            score += 2; // Single
        } else {
            score += 5; // Album Track
        }

        // HEURISTIC: Penalize Compilations / Radio Specials
        // We want the original studio album to win.
        let lower_album = album.to_lowercase();
        if lower_album.contains("radio")
            || lower_album.contains("special")
            || lower_album.contains("greatest")
            || lower_album.contains("best of")
            || lower_album.contains("remix")
        {
            score -= 3;
        }

        // Bonus: Prefer "Deluxe" over standard if everything else matches,
        // but not if it triggers the penalty above.
        if lower_album.contains("deluxe") {
            score += 1;
        }
    } else {
        score = 0; // Poor metadata
    }

    score
}

fn remove_song_file(song: &SongMetadata, dry_run: bool) {
    let path = match &song.file_path {
        Some(p) => p,
        None => return,
    };

    // Extra safety logging
    let album_name = song.album.as_deref().unwrap_or("Unknown");
    info!(
        "  REMOVING: {:?} (Album: {:?})",
        path.file_name().unwrap_or_default(),
        album_name
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
