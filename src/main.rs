#![allow(dead_code)]
#![allow(clippy::uninlined_format_args)]

mod libman;
mod metadata;
mod playlists;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use muman::recurse_dir;

use crate::libman::Library;

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
            let library = Library::get_library(music_dir, cli.recursive);

            for metadata in library.songs() {
                match metadata.get_lyrics(cli.overwrite) {
                    Ok(_) => println!(
                        "Lyrics added for: {} - {}",
                        metadata.artist.as_deref().unwrap_or("Unknown Artist"),
                        metadata.title.as_deref().unwrap_or("Unknown Title")
                    ),
                    Err(_) => println!(
                        "Failed to get lyrics for: {} - {}",
                        metadata.artist.as_deref().unwrap_or("Unknown Artist"),
                        metadata.title.as_deref().unwrap_or("Unknown Title")
                    ),
                }
            }
        }
        Commands::Playlist {
            music_dir,
            csv_files,
            output_dir,
        } => {
            // Get all the songs from the lib
            let library = Library::get_library(music_dir, cli.recursive);

            // Iter the path and get all the playlist from the directory
            let mut playlists_paths = Vec::new();

            recurse_dir(&csv_files, &mut playlists_paths, cli.recursive);

            // Turn each playlist path into a Playlist struct
            let playlists: Vec<playlists::Playlist> = playlists_paths
                .into_iter()
                .map(playlists::Playlist::new)
                .collect();

            // For each playlist, filter and complete the songs from the library
            let playlists: Vec<playlists::Playlist> = playlists
                .into_iter()
                .map(|mut pl| {
                    pl.filter_and_complete_from_library(&library);
                    pl
                })
                .collect();

            // For each playlist, create an M3U file in the output directory
            for playlist in playlists {
                playlist.save_to_m3u(&output_dir);
            }
        }
    }
}
