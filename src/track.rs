use std::path::PathBuf;

use lofty::{
    file::{AudioFile, TaggedFileExt},
    tag::ItemValue,
};

use crate::{album::Album, artist::Artist};

#[derive(Debug)]
pub struct DirtyTrack {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,

    duration: Option<u32>,
    isrc: Option<String>,
    bitrate: Option<u32>,

    track_number: Option<u32>,
    disc_number: Option<u32>,
    year: Option<u32>,

    pub file_path: Option<PathBuf>,
}

impl DirtyTrack {
    fn fill_metadata(&mut self) {
        if let Some(path) = &self.file_path {
            if let Ok(tagged_file) = lofty::read_from_path(path) {
                if let Some(tag) = tagged_file.primary_tag() {
                    self.title = tag
                        .get_string(&lofty::tag::ItemKey::TrackTitle)
                        .map(|s| s.to_string());
                    self.artist = tag
                        .get_string(&lofty::tag::ItemKey::TrackArtist)
                        .map(|s| s.to_string());
                    self.album = tag
                        .get_string(&lofty::tag::ItemKey::AlbumTitle)
                        .map(|s| s.to_string());
                    self.genre = tag
                        .get_string(&lofty::tag::ItemKey::Genre)
                        .map(|s| s.to_string());
                    self.track_number = tag
                        .get_string(&lofty::tag::ItemKey::TrackNumber)
                        .map_or(None, |n| n.parse::<u32>().ok());
                    self.disc_number = tag
                        .get_string(&lofty::tag::ItemKey::DiscNumber)
                        .map_or(None, |n| n.parse::<u32>().ok());
                    self.year = tag
                        .get_string(&lofty::tag::ItemKey::Year)
                        .map_or(None, |n| n.parse::<u32>().ok());
                    self.isrc = tag
                        .get_string(&lofty::tag::ItemKey::Isrc)
                        .map(|s| s.to_string());
                }

                let properties = tagged_file.properties();
                self.duration = Some(properties.duration().as_secs() as u32);
                self.bitrate = properties.audio_bitrate();
            }
        }
    }
}

impl Default for DirtyTrack {
    fn default() -> Self {
        DirtyTrack {
            title: None,
            artist: None,
            album: None,
            genre: None,
            duration: None,
            isrc: None,
            bitrate: None,
            track_number: None,
            disc_number: None,
            year: None,
            file_path: None,
        }
    }
}

impl From<PathBuf> for DirtyTrack {
    fn from(path: PathBuf) -> Self {
        let mut track = DirtyTrack {
            file_path: Some(path),
            ..Default::default()
        };
        track.fill_metadata();
        track
    }
}

pub struct Track {
    title: String,

    artist: Box<Artist>,
    album: Box<Album>,

    genre: String,

    duration: u32,
    isrc: String,
    bitrate: u32,

    track_number: u32,
    disc_number: u32,
    year: u32,

    pub file_path: PathBuf,
}
