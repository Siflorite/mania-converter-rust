pub mod calc_sr;
pub mod osz_func;
mod helper_functions;

use core::f64;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
pub use calc_sr::{calculate_from_data, calculate_from_file};
pub use osz_func::{parse_osz_file, parse_osz_postprocess, parse_whole_dir_osz};

use crate::BeatMapInfo;

#[derive(Debug)]
pub struct OsuMisc {
    pub audio_file_name: String,
    pub preview_time: i32,
    pub title: String,
    pub title_unicode: String,
    pub artist: String,
    pub artist_unicode: String,
    pub creator: String,
    pub version: String,
    pub circle_size: u32,
    pub od: f64,
    pub background: String,
}

#[derive(Debug)]
pub struct OsuTimingPoint {
    pub time: f64,
    pub val: f64, 
    pub is_timing: bool,
}

#[derive(Debug)]
pub struct OsuHitObject {
    pub x_pos: u32,
    pub time: u32,
    pub end_time: Option<u32>,
}

#[derive(Debug)]
pub struct OsuData {
    pub misc: OsuMisc,
    pub timings: Vec<OsuTimingPoint>,
    pub notes: Vec<OsuHitObject>,
}

#[derive(Debug)]
enum Section {
    General,
    Metadata,
    Difficulty,
    Events,
    TimingPoints,
    HitObjects,
    Unknown,
}

impl From<&str> for Section {
    fn from(s: &str) -> Self {
        match s {
            "General" => Section::General,
            "Metadata" => Section::Metadata,
            "Difficulty" => Section::Difficulty,
            "Events" => Section::Events,
            "TimingPoints" => Section::TimingPoints,
            "HitObjects" => Section::HitObjects,
            _ => Section::Unknown,
        }
    }
}

impl OsuData {
    fn parse_key_value(line: &str) -> Option<(&str, &str)> {
        line.split_once(':').map(|(k, v)| (k.trim(), v.trim()))
    }

    fn parse_timing_point(line: &str) -> Option<OsuTimingPoint> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            return None;
        }

        let time = parts[0].parse().ok()?;
        let val = parts[1].parse().ok()?;
        let is_timing = parts.get(6).map_or(true, |&x| x == "1");

        Some(OsuTimingPoint { time, val, is_timing })
    }

    fn parse_hit_object(line: &str) -> Option<OsuHitObject> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            return None;
        }

        let x_pos = parts[0].parse().ok()?;
        let time = parts[2].parse().ok()?;
        let end_time = match parts[3] {
            "128" => parts[5].split(':').next().and_then(|s| s.parse().ok()),
            _ => None
        };

        Some(OsuHitObject { x_pos, time, end_time })
    }

    pub fn from_file(file_path: &str) -> Result<Self, io::Error> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        let mut misc = OsuMisc {
            audio_file_name: String::new(),
            preview_time: 0,
            title: String::new(),
            title_unicode: String::new(),
            artist: String::new(),
            artist_unicode: String::new(),
            creator: String::new(),
            version: String::new(),
            circle_size: 0,
            od: 0.0,
            background: String::new(),
        };

        let mut timings = Vec::new();
        let mut notes = Vec::new();
        let mut current_section = Section::Unknown;

        for line in reader.lines() {
            let line = line?.trim().to_string();
            if line.is_empty() {
                continue;
            }

            // Check if this is a section header
            if line.starts_with('[') && line.ends_with(']') {
                current_section = Section::from(&line[1..line.len() - 1]);
                continue;
            }

            match current_section {
                Section::General | Section::Metadata | Section::Difficulty => {
                    if let Some((key, value)) = Self::parse_key_value(&line) {
                        match key {
                            "AudioFilename" => misc.audio_file_name = value.to_string(),
                            "PreviewTime" => misc.preview_time = value.parse().unwrap_or(0),
                            "Mode" =>  {
                                let v = value.parse().unwrap_or(0);
                                if v != 3 {
                                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "This program only supports mania mode!"));
                                }
                            },
                            "Title" => misc.title = value.to_string(),
                            "TitleUnicode" => misc.title_unicode = value.to_string(),
                            "Artist" => misc.artist = value.to_string(),
                            "ArtistUnicode" => misc.artist_unicode = value.to_string(),
                            "Creator" => misc.creator = value.to_string(),
                            "Version" => misc.version = value.to_string(),
                            "CircleSize" => misc.circle_size = value.parse().unwrap_or(0),
                            "OverallDifficulty" => misc.od = value.parse().unwrap_or(0.0),
                            _ => {}
                        }
                    }
                }
                Section::Events => {
                    if line.starts_with("//") {
                        continue;
                    }
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 3 && parts[0] == "0" && parts[1] == "0" {
                        misc.background = parts[2].trim_matches('"').to_string();
                    }
                }
                Section::TimingPoints => {
                    if let Some(timing) = Self::parse_timing_point(&line) {
                        timings.push(timing);
                    }
                }
                Section::HitObjects => {
                    if let Some(note) = Self::parse_hit_object(&line) {
                        notes.push(note);
                    }
                }
                Section::Unknown => {}
            }
        }

        Ok(OsuData { misc, timings, notes})
    }

    fn get_bpm_range(&self) -> (f64, Option<f64>) {
        // FilterMap will not include None values
        let bpm_list: Vec<f64> = self.timings
            .iter().filter_map(|t| {
                match t.is_timing {
                    true => Some(60000.0 / t.val),
                    false => None
                }
            }).collect();
        if bpm_list.is_empty() { return (0.0, None) }
        let min_bpm = *bpm_list.iter().min_by(|&x, &y| x.partial_cmp(y).unwrap()).unwrap();
        let max_bpm: Option<f64> = if bpm_list.len() == 1 {None} else {
            Some(*bpm_list.iter().max_by(|&x, &y| x.partial_cmp(y).unwrap()).unwrap())
        };
        (min_bpm, max_bpm)
    }

    fn get_length(&self) -> u32 {
        let min_time = self.notes.iter().map(|n| n.time).min().unwrap_or(0);
        let max_time = self.notes.iter().map(|n| n.time).max().unwrap_or(0);
        let max_time_tail = self.notes.iter().filter_map(|n| n.end_time).max().unwrap_or(0);
        let length = max_time.max(max_time_tail).saturating_sub(min_time);
        length
    }

    pub fn to_beatmap_info(&self, b_calc_sr: bool) -> BeatMapInfo {
        let (min_bpm, max_bpm) = self.get_bpm_range();

        let length = self.get_length();

        let note_count = self.notes.len() as u32;
        let ln_count = self.notes.iter().filter(
            |&n| n.end_time.is_some()
        ).count() as u32;

        BeatMapInfo {
            title: self.misc.title.clone(),
            title_unicode: Some(self.misc.title_unicode.clone()),
            artist: self.misc.artist.clone(),
            artist_unicode: Some(self.misc.artist_unicode.clone()),
            creator: self.misc.creator.clone(),
            version: self.misc.version.clone(),
            column_count: self.misc.circle_size as u8,
            min_bpm: min_bpm,
            max_bpm: max_bpm,
            length: length,
            sr: 
            if b_calc_sr { 
                match calculate_from_data(self, 1.0) {
                    Ok(sr) => Some(sr.max(0.0)),
                    Err(_) => None
                }
            } else { None },
            note_count: note_count - ln_count,
            ln_count: ln_count,
            bg_name: Some(self.misc.background.clone())
        }
    }
}