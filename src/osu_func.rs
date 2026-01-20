pub mod calc_sr;
mod helper_functions;
pub mod osz_func;
pub mod osz2mcz;

pub use calc_sr::{calculate_from_data, calculate_from_file};
use core::f64;
pub use osz_func::{parse_osz_file, parse_osz_postprocess, parse_whole_dir_osz};
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use crate::BeatMapInfo;
use crate::malody_func::{self, McData, Meta, Beat, Note};

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

    fn version() -> &'static str;
    fn get_x_pos(&self) -> u32;
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

    fn version() -> &'static str {
        "v14"
    }

    fn parse(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            return None;
        }

        let x_pos = parts[0].parse().ok()?;
        let time = parts[2].parse().ok()?;
        let end_time = match parts[3] {
            "128" => parts[5].split(':').next().and_then(|s| s.parse().ok()),
            _ => None,
        };

        Some(Self {
            x_pos,
            time,
            end_time,
        })
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

    fn get_x_pos(&self) -> u32 {
        self.x_pos
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

    fn version() -> &'static str {
        "v128"
    }

    fn parse(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            return None;
        }

        let x_pos = parts[0].parse().ok()?;
        let time = parts[2].parse().ok()?;
        let end_time = match parts[3] {
            "128" => parts[5].split(':').next().and_then(|s| s.parse().ok()),
            _ => None,
        };

        Some(Self {
            x_pos,
            time,
            end_time,
        })
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

    fn get_x_pos(&self) -> u32 {
        self.x_pos
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
    H: HitObject + Clone,
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

        Some(OsuTimingPoint {
            time,
            val,
            is_timing,
        })
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

    pub fn to_legacy(self) -> OsuDataLegacy
    where
        H: HitObject,
    {
        OsuDataLegacy {
            misc: self.misc,
            timings: self.timings,
            notes: self.notes.into_iter().map(H::to_legacy).collect(),
        }
    }

    pub fn to_v128(self) -> OsuDataV128
    where
        H: HitObject,
    {
        OsuDataV128 {
            misc: self.misc,
            timings: self.timings,
            notes: self.notes.into_iter().map(H::to_v128).collect(),
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
                            "Mode" => {
                                let v = value.parse().unwrap_or(0);
                                if v != 3 {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidInput,
                                        "This program only supports mania mode!",
                                    ));
                                }
                            }
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
                            }
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

        Ok(Self {
            misc,
            timings,
            notes,
        })
    }

    pub fn to_file(&self, file_path: &str) -> io::Result<()>
    where
        H: Send + Sync,
    {
        let osu_file = File::create(file_path)?;
        let mut writer = BufWriter::new(osu_file);

        // 构建 General 部分
        write!(writer, "osu file format {}\n\n[General]\n", H::version())?;
        write!(writer, "AudioFilename: {}\n", self.misc.audio_file_name)?;
        write!(
            writer,
            "AudioLeadIn: 0\nPreviewTime: {}\nCountdown: 0\nSampleSet: Soft\n",
            self.misc.preview_time
        )?;
        write!(writer, "StackLeniency: 0.7\nMode: 3\nLetterboxInBreaks: 0\nSpecialStyle: 0\nWidescreenStoryboard: 1\n\n")?;

        // 构建 Editor 部分
        write!(
            writer,
            "[Editor]\nDistanceSpacing: 1\nBeatDivisor: 8\nGridSize: 4\nTimelineZoom: 2\n\n"
        )?;

        // 构建 Metadata 部分
        write!(writer, "[Metadata]\n")?;
        write!(writer, "Title:{}\n", self.misc.title)?;
        write!(writer, "TitleUnicode:{}\n", self.misc.title_unicode)?;
        write!(writer, "Artist:{}\n", self.misc.artist)?;
        write!(writer, "ArtistUnicode:{}\n", self.misc.artist_unicode)?;
        write!(writer, "Creator:{}\n", self.misc.creator)?;
        write!(writer, "Version:{}\n", self.misc.version)?;
        write!(
            writer,
            "Source:\nTags:\nBeatmapID:{}\nBeatmapSetID:{}\n\n",
            self.misc.beatmap_id, self.misc.beatmap_set_id
        )?;

        // 构建 Difficulty 部分
        write!(writer, "[Difficulty]\n")?;
        let od_str = if self.misc.od.trunc() == self.misc.od {
            format!("{:.0}", self.misc.od)
        } else {
            format!("{:.1}", self.misc.od)
        };
        write!(writer, "HPDrainRate:8\nCircleSize:{}\nOverallDifficulty:{}\nApproachRate:5\nSliderMultiplier:1.4\nSliderTickRate:1\n\n", self.misc.circle_size, od_str)?;

        // 构建 Events 部分
        write!(writer, "[Events]\n//Background and Video events\n")?;
        if !self.misc.background.is_empty() {
            write!(writer, "0,0,\"{}\",0,0\n", self.misc.background)?;
        }
        write!(
            writer,
            "//Break Periods\n//Storyboard Layer 0 (Background)\n"
        )?;
        write!(
            writer,
            "//Storyboard Layer 1 (Fail)\n//Storyboard Layer 2 (Pass)\n"
        )?;
        write!(
            writer,
            "//Storyboard Layer 3 (Foreground)\n//Storyboard Layer 4 (Overlay)\n"
        )?;
        write!(writer, "//Storyboard Sound Samples\n\n")?;

        // 构建 TimingPoints 部分
        let timing_points: Vec<_> = self
            .timings
            .par_iter()
            .map(|tp| format!("{},{},4,2,0,10,{},0", tp.time, tp.val, tp.is_timing as u8))
            .collect();

        // 构建 HitObjects 部分
        let hit_objects: Vec<_> = self
            .notes
            .par_iter()
            .map(|ho| {
                let h = ho.get_time();
                let h_f: f64 = h.into();
                let h_str = if h_f.fract() < 1e-12 {
                    format!("{:.0}", h_f)
                } else {
                    format!("{:.12}", h_f)
                };
                if let Some(t) = ho.get_end_time() {
                    let t_f: f64 = t.into();
                    let t_str = if t_f.fract() < 1e-12 {
                        format!("{:.0}", t_f)
                    } else {
                        format!("{:.12}", t_f)
                    };
                    format!("{},192,{},128,0,{}:0:0:0:0:", ho.get_x_pos(), h_str, t_str)
                } else {
                    format!("{},192,{},1,0,0:0:0:0:", ho.get_x_pos(), h_str)
                }
            })
            .collect();

        write!(writer, "[TimingPoints]\n")?;
        writer.write_all(timing_points.join("\n").as_bytes())?;
        write!(writer, "\n\n[HitObjects]\n")?;
        writer.write_all(hit_objects.join("\n").as_bytes())?;

        Ok(())
    }

    fn get_bpm_range(&self) -> (f64, Option<f64>) {
        // FilterMap will not include None values
        let bpm_list: Vec<f64> = self
            .timings
            .iter()
            .filter_map(|t| match t.is_timing {
                true => Some(60000.0 / t.val),
                false => None,
            })
            .collect();
        if bpm_list.is_empty() {
            return (0.0, None);
        }
        let min_bpm = *bpm_list
            .iter()
            .min_by(|&x, &y| x.partial_cmp(y).unwrap())
            .unwrap();
        let max_bpm: Option<f64> = if bpm_list.len() == 1 {
            None
        } else {
            Some(
                *bpm_list
                    .iter()
                    .max_by(|&x, &y| x.partial_cmp(y).unwrap())
                    .unwrap(),
            )
        };
        (min_bpm, max_bpm)
    }

    fn get_length(&self) -> u32 {
        let (min_time, max_time) = self
            .notes
            .iter()
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
        let ln_count = self
            .notes
            .iter()
            .filter(|&n| n.get_end_time().is_some())
            .count() as u32;

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
            sr: if b_calc_sr {
                match calculate_from_data(&self.clone().to_legacy(), 1.0) {
                    Ok(sr) => Some(sr.max(0.0)),
                    Err(_) => None,
                }
            } else {
                None
            },
            note_count: note_count - ln_count,
            ln_count: ln_count,
            bg_name: Some(self.misc.background.clone()),
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

impl OsuDataLegacy {
    pub fn to_mc_data(&self) -> McData {
        // 轨道数
        let column_num = self.misc.circle_size;

        // malody的初始时间点可以认为是osu往回退第一个负数时间
        let original_timings = self.timings.iter().filter(|t| t.is_timing).collect::<Vec<_>>();
        let original_offset = original_timings.first().unwrap().time;
        let original_interval = original_timings.first().unwrap().val;
        let offset_beats = if original_offset > 0.0 {
            (original_offset / original_interval).ceil()
        } else {
            0.0
        };
        let offset = offset_beats * original_interval;

        let preview = self.misc.preview_time - offset as i32;

        let song = malody_func::Song {
            title: self.misc.title.clone(),
            artist: self.misc.artist.clone(),
            titleorg: Some(self.misc.title_unicode.clone()),
            artistorg: Some(self.misc.artist_unicode.clone()),
        };

        let mode_ext = malody_func::ModeExt {
            column: column_num as u8,
        };

        let mc_meta = Meta {
            creator: self.misc.creator.clone(),
            background: self.misc.background.clone(),
            version: self.misc.version.clone(),
            preview: Some(preview),
            mode: 0, 
            song,
            mode_ext,
        };

        let timings = original_timings
            .iter()
            .enumerate()
            .map(|(i, t)| {
                OsuTimingPoint {
                    time: if i == 0 {t.time - offset.floor()} else {t.time}, // 只需要调整第一根时间线的位置
                    val: t.val,
                    is_timing: t.is_timing,
                }
            })
            .collect::<Vec<_>>();

        let beat_counts = timings
            .windows(2)
            .map(|t| {
                let start_time = t[0].time;
                let interval = t[0].val;
                let end_time = t[1].time;
                let val = (end_time - start_time) / interval;
                Beat::from_float(val)
            }).collect::<Vec<_>>();

        let mut beat_counts_fold = beat_counts
            .iter()
            .scan(Beat::default(), |sum, &x| {
                *sum += x;
                Some(*sum)
            })
            .collect::<Vec<_>>();
        beat_counts_fold.insert(0, Beat::default());

        let beats_grid = beat_counts_fold
            .iter()
            .enumerate()
            .map(|(i, b)| {
                malody_func::Timing {
                    beat: b.to_vec(),
                    bpm: 60000f64 / timings[i].val,
                }
            })
            .collect::<Vec<_>>();

        // 顺便把效果也转换到mc
        let time_to_beat = |time: f64| -> Beat {
            let timing_index = timings.partition_point(|t| t.time < time).saturating_sub(1);
            let timing = &timings[timing_index];
            let beats = beat_counts_fold[timing_index];
            beats + Beat::from_float((time - timing.time) / timing.val)
        };

        let effects_grid = self.timings
            .iter()
            .filter(|t| !t.is_timing)
            .map(|t| {
                let effect_time = t.time;
                let beat = time_to_beat(effect_time);
                malody_func::Effect {
                    beat: beat.to_vec(),
                    scroll: -100f64 / t.val,
                }
            })
            .collect::<Vec<_>>();
        let mc_effects = if effects_grid.is_empty() {
            None
        } else {
            Some(effects_grid)
        };

        let to_grid = |note: &OsuHitObjectLegacy| -> Note {
            let start_time = note.time;
            let start_grid = time_to_beat(start_time as f64);

            let end_grid = if let Some(end_time) = note.end_time {
                let end_grid = time_to_beat(end_time as f64);
                Some(end_grid)
            } else {
                None
            };

            let column = note.x_pos * column_num / 512;

            Note { 
                beat: start_grid.to_vec(), 
                endbeat: end_grid.map(|e| e.to_vec()),
                column: Some(column as u8), 
                sound: None, 
                vol: None, 
                offset: None, 
                r#type: None 
            }
        };

        let mut new_notes = self.notes
            .iter()
            .enumerate()
            .map(|(_i, n)| to_grid(n))
            .collect::<Vec<_>>();

        // Malody最后一个音符是开始时间信息
        new_notes.push(
            Note { 
                beat: Vec::from([0,0,1]), 
                endbeat: None, 
                column: None, 
                sound: Some(self.misc.audio_file_name.clone()), 
                vol: Some(100), 
                offset: Some(timings[0].time.abs() as i32), 
                r#type: Some(1)
            });

        McData {
            meta: mc_meta,
            time: beats_grid,
            effect: mc_effects,
            note: new_notes,
        }
    }
}