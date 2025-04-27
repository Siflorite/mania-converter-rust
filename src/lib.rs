pub mod malody_func;
pub mod osu_func;

use std::fmt;

// Some miscellaneous stuff:

#[derive(Debug)]
pub struct BeatMapInfo {
    title: String,
    title_unicode: Option<String>,
    artist: String,
    artist_unicode: Option<String>,
    creator: String,
    version: String,
    column_count: u8,
    sr: Option<f64>
}

impl fmt::Display for BeatMapInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title_unicode_str = self.title_unicode.as_ref().map_or("".into(), |v| v.clone());
        let artist_unicode_str = self.artist_unicode.as_ref().map_or("".into(), |v| v.clone());
        let title_str = match title_unicode_str.is_empty() {
            true => self.title.clone(),
            false => format!("{} ({})", self.title, title_unicode_str)
        };
        let artist_str = match artist_unicode_str.is_empty() {
            true => self.title.clone(),
            false => format!("{} ({})", self.artist, artist_unicode_str)
        };
        
        let sr_str = self.sr.map_or("N/A".into(), |v| format!("{:.4}", v));
        
        write!(
            f,
            "Title: {}\nArtist: {}\nCreator: {}\nVersion: {}\nColumns: {}\nSR: {}",
            title_str, artist_str, self.creator, self.version, self.column_count, sr_str
        )
    }
}