#![allow(dead_code)]
#![allow(clippy::uninlined_format_args)]

mod library;
mod metadata;
mod playlists;
mod utils;

use crate::{library::Library, metadata::SongMetadata};
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use std::{collections::HashMap, path::PathBuf};

#[derive(Parser)]
#[command(name = "lyradd", version, about)]
struct Cli {
    /// Increase verbosity (-v = info, -vv = debug)
    #[arg(short = 'v', action = clap::ArgAction::Count)]
    verbosity: u8,

    /// Recursive search
    #[arg(short = 'r', long = "recursive", default_value_t = false)]
    recursive: bool,

    /// Overwrite existing
    #[arg(short = 'f', long = "force", default_value_t = false)]
    overwrite: bool,

    /// Number of concurrent downloads
    #[arg(short = 'j', long = "jobs", default_value_t = 4)]
    jobs: usize,

    /// Music directory
    #[arg(value_name = "MUSIC_DIR", required = true)]
    music_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download lyrics for audio files
    Lyrics {},

    /// Test
    Test {},

    /// Convert CSV playlist files to M3U format
    Playlist {
        /// Output directory
        #[arg(short = 'o', long = "output", required = true)]
        output_dir: PathBuf,

        /// CSV directory
        #[arg(short = 'c', long = "csv-dir", required = true)]
        csv_files: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let start = std::time::Instant::now();
    let library = Library::new(cli.music_dir, cli.recursive);
    println!(
        "Library initialized in {:.2?}",
        std::time::Instant::now() - start
    );

    match cli.command {
        Commands::Test {} => {}
        Commands::Lyrics {} => {
            // Create a custom thread pool with limited concurrency
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(cli.jobs)
                .build()
                .unwrap();

            pool.install(|| {
                library.songs().par_iter().for_each(|metadata| {
                    if let Err(_) = metadata.get_lyrics(cli.overwrite) {
                        // Failures handled internally
                    }
                });
            });
        }
        Commands::Playlist {
            csv_files,
            output_dir,
        } => {
            let mut playlists_paths = Vec::new();

            utils::recurse_dir(&csv_files, &mut playlists_paths, cli.recursive);

            let playlists: Vec<playlists::Playlist> = playlists_paths
                .into_iter()
                .map(playlists::Playlist::new)
                .collect();

            let playlists: Vec<playlists::Playlist> = playlists
                .into_iter()
                .map(|mut pl| {
                    pl.filter_and_complete_from_library(&library);
                    pl
                })
                .collect();

            // Aggregate missing songs across all playlists with a counter for each song
            let mut missing_songs = HashMap::new();

            for playlist in &playlists {
                for song in &playlist.missing_songs {
                    *missing_songs.entry(song.clone()).or_insert(0) += 1;
                }
            }

            let mut missing_artists = HashMap::new();

            for song in missing_songs.keys() {
                if let Some(artist) = &song.artist {
                    *missing_artists.entry(artist.clone()).or_insert(0) += 1;
                }
            }

            // Print summary of missing songs sorted by frequency in a log file
            if !missing_songs.is_empty() {
                let mut missing_songs_vec: Vec<(&SongMetadata, &usize)> =
                    missing_songs.iter().collect();

                missing_songs_vec.sort_by(|a, b| b.1.cmp(a.1));

                let log_path = output_dir.join("missing_songs.log");
                let mut log_file = std::fs::File::create(&log_path).unwrap();
                use std::io::Write;
                writeln!(log_file, "Missing Songs Summary:").unwrap();
                for (song, count) in missing_songs_vec {
                    writeln!(
                        log_file,
                        "{} - Missing in {} playlists",
                        String::from(song),
                        count
                    )
                    .unwrap();
                }
                writeln!(
                    log_file,
                    "\nTotal unique missing songs: {}",
                    missing_songs.len()
                )
                .unwrap();
            }

            if !missing_artists.is_empty() {
                let mut missing_artists_vec: Vec<(&String, &usize)> =
                    missing_artists.iter().collect();

                missing_artists_vec.sort_by(|a, b| b.1.cmp(a.1));

                let log_path = output_dir.join("missing_artists.log");
                let mut log_file = std::fs::File::create(&log_path).unwrap();
                use std::io::Write;
                writeln!(log_file, "Missing Artists Summary:").unwrap();
                for (artist, count) in missing_artists_vec {
                    writeln!(
                        log_file,
                        "{} - Missing songs in {} playlists",
                        artist, count
                    )
                    .unwrap();
                }
                writeln!(
                    log_file,
                    "\nTotal unique missing artists: {}",
                    missing_artists.len()
                )
                .unwrap();
            }

            for playlist in playlists {
                playlist.save_to_m3u(&output_dir);
            }
        }
    }
}
