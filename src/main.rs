#![allow(dead_code)]

mod dedup;
mod library;
mod lives;
mod metadata;
mod playlists;
mod utils;

use crate::library::Library;
use clap::{Parser, Subcommand};
use log::debug;
use rayon::prelude::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lyradd", version, about)]
struct Cli {
    /// Increase verbosity (-v = info, -vv = debug)
    #[arg(short = 'v', action = clap::ArgAction::Count)]
    verbosity: u8,

    /// Recursive search in directories
    #[arg(short = 'r', long = "recursive", default_value_t = false)]
    recursive: bool,

    /// Overwrite existing files
    #[arg(short = 'f', long = "force", default_value_t = false)]
    overwrite: bool,

    /// Concurrent download jobs
    #[arg(short = 'j', long = "jobs", default_value_t = 4)]
    jobs: usize,

    /// Dry run (no changes made)
    #[arg(short = 'n', long = "dry-run", default_value_t = false)]
    dry_run: bool,

    /// Use hard links for duplicate songs instead of keeping two copies (Saves space, but merges file tags)
    #[arg(long = "hard-link", default_value_t = false)]
    hard_link: bool,

    /// Base music directory
    #[arg(value_name = "MUSIC_DIR", required = true)]
    music_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download lyrics for audio files
    Lyrics {},

    /// Remove duplicate songs (prioritizes Albums over Singles)
    RemoveDupes {},

    /// Convert CSV playlist files to M3U format
    Playlist {
        /// Directory where M3U files will be saved
        #[arg(short = 'o', long = "output", required = true)]
        output_dir: PathBuf,

        /// Directory containing CSV playlist files
        #[arg(short = 'c', long = "csv-dir", required = true)]
        csv_files: PathBuf,
    },

    /// Identify and manage live albums in the library  
    ManageLives {},
}

fn main() {
    let cli = Cli::parse();

    // Initialize Logger
    let log_level = match cli.verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let start = std::time::Instant::now();
    let library = Library::new(cli.music_dir, cli.recursive);
    debug!("Library initialized in {:.2?}", start.elapsed());

    match cli.command {
        Commands::ManageLives {} => {
            lives::run(&library, cli.dry_run);
        }
        Commands::RemoveDupes {} => {
            dedup::run(&library, cli.dry_run, cli.hard_link);
        }
        Commands::Lyrics {} => {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(cli.jobs)
                .build()
                .unwrap();

            pool.install(|| {
                library.get_all_songs().par_iter().for_each(|metadata| {
                    if let Err(_) = metadata.get_lyrics(cli.overwrite) {
                        // Logged internally
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

            // Parse Playlists
            let mut playlists: Vec<playlists::Playlist> = playlists_paths
                .into_iter()
                .map(playlists::Playlist::new)
                .collect();

            // Match against library
            for pl in &mut playlists {
                pl.filter_and_complete_from_library(&library);
            }

            // Generate Reports
            playlists::generate_missing_report(&playlists, &output_dir);

            // Save M3Us
            for playlist in playlists {
                playlist.save_to_m3u(&output_dir);
            }
        }
    }
}
