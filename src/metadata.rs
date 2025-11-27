use crate::playlists::BasicTrackInfo;
use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SongMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub file_path: Option<PathBuf>,
}

impl From<&PathBuf> for SongMetadata {
    fn from(path: &PathBuf) -> Self {
        let mut metadata = SongMetadata {
            title: None,
            artist: None,
            album: None,
            isrc: None,
            file_path: Some(path.clone()),
        };
        metadata.fill();
        metadata
    }
}

// Impl conversion to and from string with title and artist separated by " - "
impl From<&SongMetadata> for String {
    fn from(value: &SongMetadata) -> Self {
        let title = value.title.as_deref().unwrap_or("Unknown Title");
        let artist = value.artist.as_deref().unwrap_or("Unknown Artist");
        format!("{} - {}", artist, title)
    }
}

impl From<BasicTrackInfo> for SongMetadata {
    fn from(value: BasicTrackInfo) -> Self {
        SongMetadata {
            title: Some(value.track_name),
            artist: Some(value.artist_names),
            album: Some(value.album_name),
            isrc: None,
            file_path: None,
        }
    }
}

impl SongMetadata {
    pub fn fill(&mut self) {
        if let Some(ref path) = self.file_path {
            if let Ok(tagged_file) = lofty::read_from_path(path) {
                if let Some(tag) = lofty::file::TaggedFileExt::primary_tag(&tagged_file) {
                    self.title = lofty::tag::Accessor::title(tag).map(|s| s.to_string());
                    self.artist = lofty::tag::Accessor::artist(tag).map(|s| s.to_string());
                    self.album = lofty::tag::Accessor::album(tag).map(|s| s.to_string());
                    self.isrc = tag
                        .get_string(&lofty::tag::ItemKey::Isrc)
                        .map(|s| s.to_string());
                }
            }
        }
    }

    pub fn get_lyrics(&self, overwrite: bool) -> Result<(), ()> {
        let url = match self.request_lyrics_url() {
            Some(u) => u,
            None => return Err(()),
        };

        let response = ureq::get(&url).call();

        match response {
            Ok(resp) => {
                if resp.status() != 200 {
                    return Err(());
                }

                let body = resp.into_body().read_to_string().unwrap();
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();

                let lyrics = match self.lyrics_from_response(&json) {
                    Some(lyr) => lyr,
                    None => return Err(()),
                };

                match self.save_lyrics(&lyrics, overwrite) {
                    Ok(_) => {
                        println!(
                            "Lyrics added: {}",
                            self.title.as_deref().unwrap_or("Unknown")
                        );
                        Ok(())
                    }
                    Err(_) => Err(()),
                }
            }
            Err(_) => Err(()),
        }
    }

    fn request_lyrics_url(&self) -> Option<String> {
        if self.title.is_none() || self.artist.is_none() {
            return None;
        }

        let title = urlencoding::encode(self.title.as_deref().unwrap());
        let artist = urlencoding::encode(self.artist.as_deref().unwrap());
        let album = self
            .album
            .as_deref()
            .map(urlencoding::encode)
            .unwrap_or_default();
        let isrc = self
            .isrc
            .as_deref()
            .map(urlencoding::encode)
            .unwrap_or_default();

        Some(format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}&album_name={}&isrc={}",
            title, artist, album, isrc
        ))
    }

    fn save_lyrics(&self, lyrics: &str, overwrite: bool) -> std::io::Result<()> {
        if let Some(ref path) = self.file_path {
            let mut lyrics_path = path.clone();
            lyrics_path.set_extension("lrc");

            if lyrics_path.exists() && !overwrite {
                return Ok(());
            }

            std::fs::write(lyrics_path, lyrics)?;
        }
        Ok(())
    }

    fn lyrics_from_response(&self, response: &serde_json::Value) -> Option<String> {
        if let Some(synced_lyrics) = response.get("syncedLyrics").and_then(|v| v.as_str()) {
            return Some(self.improve_lyrics_format(synced_lyrics));
        }

        if let Some(unsynced_lyrics) = response.get("plainLyrics").and_then(|v| v.as_str()) {
            return Some(self.improve_lyrics_format(unsynced_lyrics));
        }

        None
    }

    fn improve_lyrics_format(&self, lyrics: &str) -> String {
        let mut improved = String::new();
        if let Some(ref title) = self.title {
            improved.push_str(&format!("[ti:{}]\n", title));
        }
        if let Some(ref artist) = self.artist {
            improved.push_str(&format!("[ar:{}]\n", artist));
        }
        improved.push_str(lyrics);
        improved
    }

    pub fn matches_metadata(&self, other: &SongMetadata) -> bool {
        let self_artist = SongMetadata::normalize_str(&self.artist);
        let other_artists: Vec<String> = other
            .artist
            .as_ref()
            .map(|s| {
                s.split(';')
                    .map(|sub| SongMetadata::normalize_str(&Some(sub.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        if !self_artist.is_empty() && !other_artists.is_empty() {
            for other_artist in other_artists {
                if !other_artist.is_empty() && self_artist == other_artist {
                    return true;
                }
            }
            return false;
        }

        true
    }

    pub fn normalize_str(input: &Option<String>) -> String {
        match input {
            Some(s) => s
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
                .trim()
                .to_string(),
            None => String::new(),
        }
    }
}
