use crate::{artist::Artist, track::Track};

pub struct Album {
    title: String,
    artist: Box<Artist>,
    tracks: Vec<Track>,

    year: u32,
}
