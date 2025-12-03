use crate::album::Album;

pub struct Artist {
    pub name: String,
    albums: Vec<Album>,

    genre: String,
}
