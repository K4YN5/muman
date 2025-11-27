use std::path::{Path, PathBuf};

/// Recursively traverses a directory and collects all files into the provided vector.
pub fn recurse_dir(path: &Path, files: &mut Vec<PathBuf>, recursive: bool) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() && recursive {
                recurse_dir(&path, files, recursive);
            }
        }
    }
}

/// Encodes a string for use in a URL query parameter.
/// Replaces non-alphanumeric characters with %XX hex sequences.
pub fn encode_url(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len());
    const HEX: &[u8] = b"0123456789ABCDEF";

    for &byte in s.as_bytes() {
        // Unreserved characters (RFC 3986) allowed in URLs
        if matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0F) as usize] as char);
        }
    }
    encoded
}
