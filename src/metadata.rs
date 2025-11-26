use crate::playlists::BasicTrackInfo;

#[derive(Debug, Clone)]
pub struct SongMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    album: Option<String>,
    isrc: Option<String>,

    pub file_path: Option<String>,
}
// Implement from and to for files path
impl From<&std::path::PathBuf> for SongMetadata {
    fn from(path: &std::path::PathBuf) -> Self {
        let mut metadata = SongMetadata {
            title: None,
            artist: None,
            album: None,
            isrc: None,
            file_path: Some(path.to_string_lossy().to_string()),
        };
        metadata.fill();

        metadata
    }
}

impl From<BasicTrackInfo> for SongMetadata {
    fn from(value: BasicTrackInfo) -> Self {
        let metadata = SongMetadata {
            title: Some(value.track_name),
            artist: Some(value.artist_names),
            album: Some(value.album_name),
            isrc: None,
            file_path: None,
        };

        metadata
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
            None => {
                println!("Insufficient metadata to request lyrics");
                return Err(());
            }
        };

        let response = ureq::get(&url).call();

        match response {
            Ok(resp) => {
                if resp.status() != 200 {
                    println!("No lyrics found for the song");
                    return Err(());
                }

                let body = resp.into_body().read_to_string().unwrap();
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();

                let lyrics = match self.lyrics_from_response(&json) {
                    Some(lyr) => lyr,
                    None => {
                        println!("No lyrics found in the response");
                        return Err(());
                    }
                };

                match self.save_lyrics(&lyrics, overwrite) {
                    Ok(_) => {
                        println!(
                            "Saved lyrics for {:?}",
                            self.title.as_deref().unwrap_or("Unknown Title")
                        );
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error saving lyrics: {}", e);
                        Err(())
                    }
                }
            }
            Err(e) => {
                eprintln!("Error fetching lyrics: {}", e);
                Err(())
            }
        }
    }

    fn is_complete(&self) -> bool {
        self.title.is_some() && self.artist.is_some() && self.album.is_some() && self.isrc.is_some()
    }

    fn request_lyrics_url(&self) -> Option<String> {
        if !self.is_complete() {
            return None;
        }

        let title = urlencoding::encode(self.title.as_deref().unwrap());
        let artist = urlencoding::encode(self.artist.as_deref().unwrap());
        let album = urlencoding::encode(self.album.as_deref().unwrap());
        let isrc = urlencoding::encode(self.isrc.as_deref().unwrap());

        Some(format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}&album_name={}&isrc={}",
            title, artist, album, isrc
        ))
    }

    fn save_lyrics(&self, lyrics: &str, overwrite: bool) -> std::io::Result<()> {
        if let Some(ref path) = self.file_path {
            let mut lyrics_path = std::path::PathBuf::from(path);
            lyrics_path.set_extension("lrc");

            if lyrics_path.exists() && !overwrite {
                return Ok(());
            }

            std::fs::write(lyrics_path, lyrics)?;
        }
        Ok(())
    }

    fn lyrics_from_response(&self, response: &serde_json::Value) -> Option<String> {
        if let Some(synced_lyrics) = response.get("syncedLyrics")
            && !synced_lyrics.is_null()
        {
            return Some(
                self.improve_lyrics_format(synced_lyrics.as_str().unwrap().to_string().as_str()),
            );
        }

        if let Some(unsynced_lyrics) = response.get("plainLyrics") {
            return Some(
                self.improve_lyrics_format(unsynced_lyrics.as_str().unwrap().to_string().as_str()),
            );
        }

        None
    }

    fn improve_lyrics_format(&self, lyrics: &str) -> String {
        let mut improved_lyrics = String::new();
        if let Some(ref title) = self.title {
            improved_lyrics.push_str(&format!("[ti:{}]\n", title));
        }
        if let Some(ref artist) = self.artist {
            improved_lyrics.push_str(&format!("[ar:{}]\n", artist));
        }
        improved_lyrics.push_str(lyrics);
        improved_lyrics
    }
    /// Checks if two songs are the same using fuzzy matching
    pub fn matches_metadata(&self, other: &SongMetadata) -> bool {
        let self_title = self.normalize_str(&self.title);
        let other_title = self.normalize_str(&other.title);
        println!("Comparing titles: '{}' vs '{}'", self_title, other_title);

        let self_artist = self.normalize_str(&self.artist);
        let other_artists: Vec<String> = other
            .artist
            .as_ref()
            .map(|s| {
                s.split(';')
                    .map(|s| s.trim().to_lowercase())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        if self_title != other_title {
            return false;
        }

        // Check if any of the other artists match self artist
        if !self_artist.is_empty() && !other_artists.is_empty() {
            for other_artist in other_artists {
                if !self_artist.is_empty() && !other_artist.is_empty() {
                    if self_artist != other_artist {
                        println!("Comparing artists: '{}' vs '{}'", self_artist, other_artist);
                        println!("Titles match though: '{}' vs '{}'", self_title, other_title);
                    }
                    return self_artist == other_artist;
                }
            }
            return false;
        }

        true
    }

    /// Helper to lowercase and remove punctuation for better matching
    /// e.g. "Beggin'" -> "beggin"
    fn normalize_str(&self, input: &Option<String>) -> String {
        match input {
            Some(s) => s
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace()) // Remove punctuation like ' or -
                .collect::<String>()
                .trim()
                .to_string(),
            None => String::new(),
        }
    }
}
