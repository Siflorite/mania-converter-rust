#![doc = include_str!("../README.md")]
pub mod graphx;
pub mod malody;
pub mod misc;
pub mod osu;

use std::fmt;
// Some miscellaneous stuff:

/// The information of a beatmap.
/// Often obtained after converting a beatmap, used when generating info cards.
///
/// Implements [`fmt::Display`] so that it can be directly printed.
///
/// # Examples
///
/// ```
/// use mania_converter::osu::{HitObject, OsuDataLegacy};
///
/// let osu_data = OsuDataLegacy::from_file("./tests/beatmaps/MIssionary - Shuen Kara Inochi o Sukuu.osu").unwrap();
/// let beatmap_info = osu_data.to_beatmap_info(false);
/// println!("{}", beatmap_info);
///
/// let regular_notes_num = osu_data
///     .notes
///     .iter()
///     .filter(|&n| n.get_end_time().is_none())
///     .count() as u32;
/// assert_eq!(beatmap_info.note_count, regular_notes_num);
/// assert_eq!(beatmap_info.sr, None);
/// ```
#[derive(Debug, Clone)]
pub struct BeatMapInfo {
    /// The ASCII title of the beatmap.
    pub title: String,
    /// The title in original language of the beatmap.
    /// Known as `TitleUnicode` in osu! and `titleorg` in Malody.
    pub title_unicode: Option<String>,
    /// The ASCII artist name.
    pub artist: String,
    /// The artist name in original language.
    /// Known as `ArtistUnicode` in osu! and `artistorg` in Malody.
    pub artist_unicode: Option<String>,
    /// The creator of the beatmap.
    pub creator: String,
    /// The version name of the beatmap.
    pub version: String,
    /// osu! Beatmap ID, 0 for unuploaded or converted beatmaps.
    pub beatmap_id: u64,
    /// osu! Beatmap Set ID, -1 for unuploaded or converted beatmaps.
    pub beatmap_set_id: i64,
    /// The number of columns for a mania-mode beatmap.
    /// Stored in `Difficulty.CircleSize` as integer in osu!, `meta.mode_ext.column` in Malody.
    pub column_count: u8,
    /// Minimum BPM of the beatmap.
    pub min_bpm: f64,
    /// Maximum BPM of the beatmap.
    /// [`Some`] if maximum BPM varies from the minimum value, [`None`] otherwise.
    pub max_bpm: Option<f64>,
    /// The time length of the beatmap (from the first note's start time to the last one's end time) in milliseconds.
    pub length: u32,
    /// The star rating of the beatmap based on [sunnyxxy's `Star-Rating-Rebirth`](https://github.com/sunnyxxy/Star-Rating-Rebirth)
    /// [2025/04/15 release](https://github.com/sunnyxxy/Star-Rating-Rebirth/releases/tag/2025%2F04%2F15).
    /// Since SR calcucation can be turned off in conversions, this segment may be [`None`].
    pub sr: Option<f64>,
    /// Total regular note (Hit circle in osu! term) count of the beatmap.
    pub note_count: u32,
    /// Total long note (Hold in osu! term) count of the beatmap.
    pub ln_count: u32,
    /// The name of the background image. Not used in formatted display.
    pub bg_name: String,
}

/// Displays the information of a beatmap.
///
/// Format:
///
/// Title: `"{title}"` if `title_unicode` is empty, otherwise `"{title} ({title_unicode})"`
///
/// Artist: `"{artist}"` if `artist_unicode` is empty, otherwise `"{artist} ({artist_unicode})"`
///
/// Creator: `"{creator}"`
///
/// Version: `"{version}"`
///
/// BeatmapID: `"{beatmap_id}"`
///
/// BeatmapSetID: `"{beatmap_set_id}"`
///
/// Columns: `"{column_count}"`
///
/// BPM: `"{min_bpm}"` if BPM is constant, otherwise `"{min_bpm}-{max_bpm}"`
///
/// Length: Length of the song in `"{minutes}:{seconds:02}.{milliseconds:03}"` format.
///
/// SR: `{sr:.4}` if SR is not [`None`], otherwise `"N/A"`
///
/// LN_Ratio: The ratio of long notes to total notes (`ln_count as f64 / (ln_count + note_count) as f64`) in `{:.3}` format.
impl fmt::Display for BeatMapInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title_unicode_str = self.title_unicode.as_ref().map_or("".into(), |v| v.clone());
        let artist_unicode_str = self
            .artist_unicode
            .as_ref()
            .map_or("".into(), |v| v.clone());
        let title_str = match title_unicode_str.is_empty() {
            true => self.title.clone(),
            false => format!("{} ({})", self.title, title_unicode_str),
        };
        let artist_str = match artist_unicode_str.is_empty() {
            true => self.artist.clone(),
            false => format!("{} ({})", self.artist, artist_unicode_str),
        };
        let bpm_str = match self.max_bpm {
            Some(val) => format!("{}-{}", self.min_bpm, val),
            None => format!("{}", self.min_bpm),
        };
        let length_str = format!(
            "{}:{:02}.{:03}",
            self.length / 60000,
            (self.length % 60000) / 1000,
            self.length % 1000
        );

        let sr_str = self.sr.map_or("N/A".into(), |v| format!("{:.4}", v));
        let ln_ratio = self.ln_count as f64 / (self.ln_count + self.note_count) as f64;

        write!(
            f,
            "Title: {}\nArtist: {}\nCreator: {}\nVersion: {}\nBeatmapID: {}\nBeatmapSetID: {}\nColumns: {}\nBPM: {}\nLength: {}\nSR: {}\nLN_Ratio: {:.3}",
            title_str,
            artist_str,
            self.creator,
            self.version,
            self.beatmap_id,
            self.beatmap_set_id,
            self.column_count,
            bpm_str,
            length_str,
            sr_str,
            ln_ratio
        )
    }
}
