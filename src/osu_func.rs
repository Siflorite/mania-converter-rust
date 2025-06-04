pub mod calc_sr;
pub mod osz_func;
mod helper_functions;
// pub mod osz2mcz;

use core::f64;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
pub use calc_sr::{calculate_from_data, calculate_from_file};
pub use osz_func::{parse_osz_file, parse_osz_postprocess, parse_whole_dir_osz};

use crate::BeatMapInfo;

#[derive(Debug, Clone)]
pub struct OsuMisc {
    pub audio_file_name: String,
    pub preview_time: i32,
    pub title: String,
    pub title_unicode: String,
    pub artist: String,
    pub artist_unicode: String,
    pub creator: String,
    pub version: String,
    pub beatmap_id: u64,
    pub beatmap_set_id: i64, // -1 for unuploaded
    pub circle_size: u32,
    pub od: f64,
    pub background: String,
}

#[derive(Debug, Clone)]
pub struct OsuTimingPoint {
    pub time: f64,
    pub val: f64, 
    pub is_timing: bool,
}

pub trait HitObject: Sized {
    type TimeType: PartialOrd + Copy + Into<f64>;
    fn parse(line: &str) -> Option<Self>;
    fn to_legacy(self) -> OsuHitObjectLegacy;
    fn to_v128(self) -> OsuHitObjectV128;
    fn get_time(&self) -> Self::TimeType;
    fn get_end_time(&self) -> Option<Self::TimeType>;
}

#[derive(Debug, Clone)]
pub struct OsuHitObjectLegacy {
    pub x_pos: u32,
    pub time: u32,
    pub end_time: Option<u32>,
}

impl HitObject for OsuHitObjectLegacy {
    type TimeType = u32;

    fn parse(line: &str) -> Option<Self> {
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

        Some(Self { x_pos, time, end_time })
    }

    fn to_legacy(self) -> OsuHitObjectLegacy {
        self
    }

    fn to_v128(self) -> OsuHitObjectV128 {
        OsuHitObjectV128 {
            x_pos: self.x_pos,
            time: self.time as f64,
            end_time: self.end_time.map(|t| t as f64),
        }
    }

    fn get_time(&self) -> Self::TimeType {
        self.time
    }

    fn get_end_time(&self) -> Option<Self::TimeType> {
        self.end_time
    }
}   

#[derive(Debug, Clone)]
pub struct OsuHitObjectV128 {
    pub x_pos: u32,
    pub time: f64,
    pub end_time: Option<f64>,
}

impl HitObject for OsuHitObjectV128 {
    type TimeType = f64;
    
    fn parse(line: &str) -> Option<Self> {
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

        Some(Self { x_pos, time, end_time })
    }

    fn to_legacy(self) -> OsuHitObjectLegacy {
        OsuHitObjectLegacy {
            x_pos: self.x_pos,
            time: self.time as u32,
            end_time: self.end_time.map(|t| t as u32),
        }
    }

    fn to_v128(self) -> OsuHitObjectV128 {
        self
    }

    fn get_time(&self) -> Self::TimeType {
        self.time
    }

    fn get_end_time(&self) -> Option<Self::TimeType> {
        self.end_time
    }
}

#[derive(Debug, Clone)]
pub struct OsuData<H> {
    pub misc: OsuMisc,
    pub timings: Vec<OsuTimingPoint>,
    pub notes: Vec<H>,
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

impl<H> OsuData<H> 
where 
    H: HitObject + Clone
{
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

    // 转换到其他版本
    pub fn convert<T: HitObject>(self) -> OsuData<T>
    where
        T: From<H>,
    {
        OsuData {
            misc: self.misc,
            timings: self.timings,
            notes: self.notes.into_iter().map(T::from).collect(),
        }
    }

    pub fn to_legacy(self) -> OsuDataLegacy where H: HitObject {
        OsuDataLegacy {
            misc: self.misc,
            timings: self.timings,
            notes: self.notes.into_iter().map(H::to_legacy).collect(),
        }
    } 

    pub fn to_v128(self) -> OsuDataV128 where H: HitObject {
        OsuDataV128 {
            misc: self.misc,
            timings: self.timings,
            notes: self.notes.into_iter().map(H::to_v128).collect()
        }
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
            beatmap_id: 0,
            beatmap_set_id: 0,
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
                            "BeatmapID" => misc.beatmap_id = value.parse().unwrap_or(0),
                            "BeatmapSetID" => misc.beatmap_set_id = value.parse().unwrap_or(-1),
                            "CircleSize" => {
                                let cs_float: f64 = value.parse().unwrap_or(0.0);
                                misc.circle_size = cs_float as u32;
                            },
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
                    if let Some(note) = H::parse(&line) {
                        notes.push(note);
                    }
                }
                Section::Unknown => {}
            }
        }

        Ok(Self { misc, timings, notes})
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
        let (min_time, max_time) = self.notes.iter()
            .filter_map(|n| {
                let start = n.get_time().into();
                let end = n.get_end_time().map(|t| t.into()).unwrap_or(start);
                Some((start, end.max(start)))
            })
            .fold((f64::INFINITY, 0f64), |(min, max), (s, e)| {
                (min.min(s), max.max(e))
            });
        let duration = (max_time - min_time).max(0.0) as u32;
        duration
    }

    pub fn to_beatmap_info(&self, b_calc_sr: bool) -> BeatMapInfo {
        let (min_bpm, max_bpm) = self.get_bpm_range();

        let length = self.get_length();

        let note_count = self.notes.len() as u32;
        let ln_count = self.notes.iter().filter(
            |&n| n.get_end_time().is_some()
        ).count() as u32;

        BeatMapInfo {
            title: self.misc.title.clone(),
            title_unicode: Some(self.misc.title_unicode.clone()),
            artist: self.misc.artist.clone(),
            artist_unicode: Some(self.misc.artist_unicode.clone()),
            creator: self.misc.creator.clone(),
            version: self.misc.version.clone(),
            beatmap_id: self.misc.beatmap_id,
            beatmap_set_id: self.misc.beatmap_set_id,
            column_count: self.misc.circle_size as u8,
            min_bpm: min_bpm,
            max_bpm: max_bpm,
            length: length,
            sr: 
            if b_calc_sr {
                match calculate_from_data(&self.clone().to_legacy(), 1.0) {
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

// 实现类型别名
pub type OsuDataLegacy = OsuData<OsuHitObjectLegacy>;
pub type OsuDataV128 = OsuData<OsuHitObjectV128>;

// 转换实现
impl From<OsuDataV128> for OsuDataLegacy {
    fn from(v: OsuDataV128) -> Self {
        OsuDataLegacy {
            misc: v.misc,
            timings: v.timings,
            notes: v.notes.into_iter().map(|n| n.to_legacy()).collect(),
        }
    }
}

// 为OsuHitObjectV128添加到Legacy的转换
impl From<OsuHitObjectV128> for OsuHitObjectLegacy {
    fn from(v: OsuHitObjectV128) -> Self {
        Self {
            x_pos: v.x_pos,
            time: v.time as u32,
            end_time: v.end_time.map(|t| t as u32),
        }
    }
}

impl From<OsuDataLegacy> for OsuDataV128 {
    fn from(v: OsuDataLegacy) -> Self {
        OsuDataV128 {
            misc: v.misc,
            timings: v.timings,
            notes: v.notes.into_iter().map(|n| n.to_v128()).collect(),
        }
    }
}

// 为OsuHitObjectV128添加到Legacy的转换
impl From<OsuHitObjectLegacy> for OsuHitObjectV128 {
    fn from(v: OsuHitObjectLegacy) -> Self {
        Self {
            x_pos: v.x_pos,
            time: v.time as f64,
            end_time: v.end_time.map(|t| t as f64),
        }
    }
}