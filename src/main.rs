#![allow(dead_code)]
#![allow(clippy::uninlined_format_args)]

mod library;
mod metadata;
mod playlists;
mod utils;

use crate::library::Library;
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use std::path::PathBuf;

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

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download lyrics for audio files
    Lyrics {
        /// Music directory/directories (operands, POSIX style)
        #[arg(value_name = "MUSIC_DIR", required = true)]
        music_dir: Vec<PathBuf>,
    },

    /// Convert CSV playlist files to M3U format
    Playlist {
        /// Music directory/directories (operands, POSIX style)
        #[arg(value_name = "MUSIC_DIR", required = true)]
        music_dir: Vec<PathBuf>,

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

    match cli.command {
        Commands::Lyrics { music_dir } => {
            let library = Library::new(music_dir, cli.recursive);

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
            music_dir,
            csv_files,
            output_dir,
        } => {
            let library = Library::new(music_dir, cli.recursive);
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

            for playlist in playlists {
                playlist.save_to_m3u(&output_dir);
            }
        }
    }
}
