use std::path::PathBuf;

use crate::{
    ALLOWED_EXTENSIONS,
    fs::{Cache, recurse_directory},
    track::DirtyTrack,
};

pub struct DirtyLibrary {
    path: PathBuf,
    pub tracks: Vec<DirtyTrack>,
}

impl DirtyLibrary {
    pub fn new(path: PathBuf, cache: Cache) -> Self {
        let tracks = recurse_directory(
            &path,
            true,
            Some(&|p: &PathBuf| {
                p.extension()
                    .and_then(|ext| ext.to_str())
                    .map_or(false, |ext_str| {
                        ALLOWED_EXTENSIONS
                            .iter()
                            .any(|allowed_ext| allowed_ext.eq_ignore_ascii_case(ext_str))
                    })
            }),
            cache.scan_count,
        )
        .into_iter()
        .map(|file_path| file_path.into())
        .collect();

        DirtyLibrary { path, tracks }
    }
}
