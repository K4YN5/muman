use std::{fs, path::PathBuf};

use log::debug;

/// Recursively traverse a directory and collect file paths. Optionally filter files and changes
/// the initial capacity of the returned vector.
pub fn recurse_directory(
    path: &PathBuf,
    recursive: bool,
    filter: Option<&dyn Fn(&PathBuf) -> bool>,
    file_count: Option<usize>,
) -> Vec<PathBuf> {
    let mut files = Vec::with_capacity(
        file_count.unwrap_or(fs::read_dir(&path).map(|rd| rd.count()).unwrap_or(0)),
    );

    let mut dirs_to_visit = Vec::with_capacity(16);
    dirs_to_visit.push(path.clone());

    while let Some(current_dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_dir() && recursive {
                    dirs_to_visit.push(path);
                } else if path.is_file() {
                    if filter.map_or(true, |f| f(&path)) {
                        files.push(path);
                    }
                }
            }
        }
    }

    files
}

const CACHE_PATH: &str = "cache.txt";

pub struct Cache {
    pub last_scan: Option<u32>,
    pub scan_count: Option<usize>,
}

impl Cache {
    pub fn new() -> Self {
        Self::read_from_file().unwrap_or(Cache {
            last_scan: None,
            scan_count: None,
        })
    }

    pub fn write_to_file(&self) -> std::io::Result<()> {
        let mut content = String::new();
        if let Some(last_scan) = self.last_scan {
            content.push_str(&format!(
                "last_scan: {}:{} {}/{}/{}\n",
                (last_scan / 100) % 100,
                last_scan % 100,
                (last_scan / 10000) % 100,
                (last_scan / 1000000) % 100,
                last_scan / 100000000
            ));
        }
        if let Some(scan_count) = self.scan_count {
            content.push_str(&format!("scan_count: {}\n", scan_count));
        }
        fs::write(CACHE_PATH, content)
    }

    pub fn read_from_file() -> std::io::Result<Self> {
        let content = fs::read_to_string(CACHE_PATH)?;
        let mut cache = Cache::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                debug!("Invalid cache line: {}", line);
                continue;
            }
            let key = parts[0].trim();
            let value = parts[1].trim();

            match key {
                "last_scan" => {
                    debug!("Parsing last_scan: {}", value);
                    if let Some(timestamp) = parse_datetime_to_u32(value) {
                        cache.last_scan = Some(timestamp);
                    }
                }
                "scan_count" => {
                    debug!("Parsing scan_count: {}", value);
                    if let Ok(count) = value.parse::<usize>() {
                        cache.scan_count = Some(count);
                    }
                }
                _ => {}
            }
        }

        Ok(cache)
    }
}

fn parse_datetime_to_u32(datetime: &str) -> Option<u32> {
    let datetime_parts: Vec<&str> = datetime.split_whitespace().collect();
    if datetime_parts.len() != 2 {
        debug!("Invalid datetime format: {}", datetime);
        return None;
    }

    let time_parts: Vec<&str> = datetime_parts[0].split(':').collect();
    let date_parts: Vec<&str> = datetime_parts[1].split('/').collect();
    if time_parts.len() != 2 {
        debug!("Invalid time format: {}", datetime_parts[0]);
        return None;
    }
    if date_parts.len() != 3 {
        debug!("Invalid date format: {}", datetime_parts[1]);
        return None;
    }

    let hour = time_parts[0].parse::<u32>().ok()?;
    let minute = time_parts[1].parse::<u32>().ok()?;
    let day = date_parts[0].parse::<u32>().ok()?;
    let month = date_parts[1].parse::<u32>().ok()?;
    let year = date_parts[2].parse::<u32>().ok()?;

    // In YYMMDDHHmm
    Some(year * 100000000 + month * 1000000 + day * 10000 + hour * 100 + minute)
}
