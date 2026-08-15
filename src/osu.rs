//! Type definitions and related functions for osu! file format.

pub mod calc_sr;
mod helper_functions;
pub mod osz2mcz;
pub mod osz_func;

pub use calc_sr::{calculate_from_data, calculate_from_file};
pub use osz_func::{parse_osz_file, parse_osz_postprocess, parse_whole_dir_osz};
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::str::FromStr;

use crate::BeatMapInfo;
use crate::malody::{self, Beat, McData, Meta, Note};

/// Miscellaneous information for osu! file format.
///
/// Includes information from \[General\], \[Editor\], \[Metadata\] and \[Difficulty\] sections.
/// Only stores useful information for conversions.
///
/// Reference: <https://osu.ppy.sh/wiki/en/Client/File_formats/osu_%28file_format%29>
#[derive(Debug, Clone, Default)]
pub struct OsuMisc {
    /// The filename of the audio file.
    ///
    /// General.AudioFilename
    pub audio_file_name: String,
    /// The audio preview time point in milliseconds.
    ///
    /// General.PreviewTime
    pub preview_time: i32,
    /// Ronamnized song title (a.k.a in ASCII).
    ///
    /// Metadata.Title
    pub title: String,
    /// Song title in original language.
    ///
    /// Metadata.TitleUnicode
    pub title_unicode: String,
    /// Ronamnized song artist name (a.k.a in ASCII).
    ///
    /// Metadata.Artist
    pub artist: String,
    /// song artist name in original language.
    ///
    /// Metadata.ArtistUnicode
    pub artist_unicode: String,
    /// Creator of the beatmap.
    ///
    /// Metadata.Creator
    pub creator: String,
    /// Difficulty name.
    ///
    /// Metadata.Version
    pub version: String,
    /// ID of the single beatmap difficulty.
    /// 0 for unuploaded or converted beatmaps.
    ///
    /// Metadata.BeatmapID
    pub beatmap_id: u64,
    /// ID of the beatmap set.
    /// -1 for unuploaded or converted beatmaps.
    ///
    /// Metadata.BeatmapSetID
    pub beatmap_set_id: i64,
    /// CS in osu!, refers to number of columns in mania mode.
    ///
    /// Difficulty.CircleSize
    pub circle_size: u32,
    /// OD (Overall Difficulty) in osu!, defines the range of judgement time windows.
    /// Refer to <https://osu.ppy.sh/wiki/en/Beatmap/Overall_difficulty#osu!mania> for more information.
    ///
    /// SR calculation relies on OD to evaluate accuracy-related parameters.
    ///
    /// Difficulty.OverallDifficulty
    pub od: f64,
    /// The filename of the background picture.
    ///
    /// Defined in osu! file's \[Events\] section, under `//Background and Video events`.
    pub background: String,
}

/// Storyboard sound samples in osu! file.
///
/// Defined in osu! file's \[Events\] section, under `//Storyboard Sound Samples`.
/// These sound samples are played when reaching the time point, irrelevant to hit objects.
///  
/// Reference: <https://osu.ppy.sh/wiki/en/Storyboard/Scripting/Audio>
///
/// Sound samples are defined as `Sample,<time>,<layer_num>,"<filepath>",<volume>` in osu! files.
#[derive(Debug, Clone)]
pub struct OsuStoryboardSoundSample {
    /// The time point in milliseconds when the sound sample is played.
    /// Though times seem to be integers in osu! files,
    /// we use [`f64`] for consistency with [`OsuTimingPoint`].
    pub time: f64,
    /// The filename of the sound sample.
    pub hitsound: String,
    /// The volume of the sound sample, ranging from 0 to 100.
    pub volume: u8,
}

/// Formats [`OsuStoryboardSoundSample`] back to osu! file format line.
/// We use `<layer_num> = 0` so that samples are always played.
impl fmt::Display for OsuStoryboardSoundSample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Sample,{},0,\"{}\",{}\n",
            self.time, self.hitsound, self.volume
        )
    }
}

/// Timing points in osu! file.
///
/// Defined in osu! file's \[TimingPoints\] section.
/// Reference: <https://osu.ppy.sh/wiki/en/Client/File_formats/osu_%28file_format%29#timing-points>
///
/// Timing points are defined as `time,beatLength,meter,sampleSet,sampleIndex,volume,uninherited,effects` in osu! files.
#[derive(Debug, Clone)]
pub struct OsuTimingPoint {
    /// Start time of the timing section in milliseconds.
    /// The end of current timing section is the next timing point's time,
    /// or end of the chart if this is the last timing section.
    pub time: f64,
    /// For uninhereted timing points, in other words, the point when BPM changes,
    /// this is the duration of a beat in milliseconds, which is `60000f64 / BPM`.
    /// For example, a section with 120 BPM has a beat duration of 60000 / 120 = 500ms.
    ///
    /// For inhereted timing points, in other words, only scroll speed changes and BPM remains the same,
    /// this value will be `-100f64 / scroll_speed`.
    /// For example, -50 makes scroll speed 2x, while -200 makes scroll speed 0.5x.
    pub val: f64,
    /// Whether or not the timing point changes BPM.
    /// `true` for uninherited timing points, `false` for inherited timing points.
    pub is_timing: bool,
}

impl OsuTimingPoint {
    /// Parses timing points from lines under section \[TimingPoints\].
    /// We only need `time`, `beatLength` and `uninherited`.
    pub fn parse(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 8 {
            return None;
        }

        let time = parts[0].parse().ok()?;
        let val = parts[1].parse().ok()?;
        let is_timing = parts.get(6).is_none_or(|&x| x == "1");

        Some(Self {
            time,
            val,
            is_timing,
        })
    }
}

/// Formats [`OsuTimingPoint`] back to osu! file format line.
///
/// We use meter = 4 for 4/4 time rhythm as a hard-coded value since most songs follow this beat,
/// side effects when songs come in other beats only causes main beats not aligned in osu! editors (which won't even affect notes aligned to grids),
/// gameplay is also not affected because osu! uses time in milliseconds for timing of notes.
///
/// We use sampleSet = 2 for normalSet, sampleIndex = 0 for osu! default hitsounds, and volume = 0.
/// Because mania players usually keep hitsounds silent.
///
/// effects section uses bit 0 and 3 for kiai time and whether to omit the first barline.
/// These will not survive through conversions, so we use 0 for no effects.
impl fmt::Display for OsuTimingPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},4,2,0,0,{},0",
            self.time, self.val, self.is_timing as u8
        )
    }
}

/// Hit objects in osu! file.
///
/// Defined in osu! file's \[HitObjects\] section.
/// Reference: <https://osu.ppy.sh/wiki/en/Client/File_formats/osu_%28file_format%29#hit-objects>
///
/// For osu!mania gamemode, hit objects fall into hit circles and hold notes.
///
/// Hit circle definition: `x,y,time,type,hitSound,hitSample`
///
/// Hold note definition: `x,y,time,type,hitSound,endTime:hitSample`
#[derive(Debug, Clone)]
pub struct OsuHitObject<H> {
    /// The horizontal position of the hit object in [osu! pixels](https://osu.ppy.sh/wiki/en/Client/Beatmap_editor/osu%21_pixel).
    ///
    /// For quick calculation, osu! uses 512 as playfield width.
    /// In osu!mania gamemode, suppose total column count (`CS` in osu! \[Difficulty\] section) is m,
    /// and a note is on column $n \in [0, m)$.
    /// osu! stores x_pos as `(n as f64 + 0.5) * 512f64 / m as f64`.
    /// And osu will arrange any hit object with x_pos
    /// in range [n * 512 / m, (n + 1) * 512 / m) to column n.
    /// The supposed calculation in osu! is `column = x_pos * CS / 512`, calculated in integers.
    pub x_pos: u32,
    /// Time when the object is to be hit, in milliseconds from the beginning of the beatmap's audio.
    ///
    /// In osu!stable (osu file format v14), time is stored as [`u32`];
    /// while in osu!lazer (osu file format v128), time is stored as [`f64`].
    /// Therefore the generic type H is used to represent both types.
    /// But beatmaps uploaded to osu! website are all converted to v14, regardless of where they are created.
    /// So we suppose osu! still uses integer time in gameplay judgements, float times are floored to integers.
    /// Therefore [`OsuHitObjectLegacy`] (a.k.a. [`OsuHitObject<u32>`]) is used for conversion with Malody formats.
    ///
    /// It is recommended to use [`OsuHitObjectV128`] (a.k.a. [`OsuHitObject<f64>`])
    /// to parse osu! file, and convert the data to [`OsuHitObjectLegacy`] for later process.
    pub time: H,
    /// End time of a hold note in milliseconds, [`None`] for hit circles.
    pub end_time: Option<H>,
    /// Volume of the hitsound sample, ranging from 1 to 100.
    /// 0 for using current timing section's volume.
    pub volume: u8,
    /// Filename of custom hitsound, [`None`] for using default hitsound.
    pub hitsound: Option<String>,
}

/// Hit objects in osu!stable generated osu! file (osu file format v14),
/// uses [`u32`] for time.
pub type OsuHitObjectLegacy = OsuHitObject<u32>;

/// Hit objects in osu!lazer generated osu! file (osu file format v128),
/// uses [`f64`] for time.
/// It is supposed that hit object times are floored to integers in gameplay judgements.
pub type OsuHitObjectV128 = OsuHitObject<f64>;

/// [`u32`] and [`f64`] are binded to constant version strings with trait [`HasVersion`].
pub trait HasVersion {
    const VERSION: &'static str;
}

/// osu file format v14 uses [`u32`] to store hit object times.
impl HasVersion for u32 {
    const VERSION: &'static str = "v14";
}

/// osu file format v128 uses [`f64`] to store hit object times.
impl HasVersion for f64 {
    const VERSION: &'static str = "v128";
}

/// Common trait for osu! hit objects.
/// [`HitObject`] needs to be [`Sized`] to return [`Self`].
pub trait HitObject: Sized + fmt::Display {
    /// The type of time used in the hit object.
    /// Currently only [`u32`] and [`f64`] are supported.
    type TimeType: PartialOrd + Copy + Into<f64> + FromStr + HasVersion;
    /// Parses a line from osu! \[HitObjects\] section to [`HitObject`] instance.
    fn parse(line: &str) -> Option<Self>;
    /// Converts [`HitObject`] instance to [`OsuHitObjectLegacy`], a.k.a [`OsuHitObject<u32>`].
    fn to_legacy(self) -> OsuHitObjectLegacy;
    /// Converts [`HitObject`] instance to [`OsuHitObjectV128`], a.k.a [`OsuHitObject<f64>`].
    fn to_v128(self) -> OsuHitObjectV128;
    /// Gets the version of the osu file format from [`HitObject::TimeType`].
    fn version() -> &'static str;
    /// Gets [`OsuHitObject<H>::x_pos`].
    fn get_x_pos(&self) -> u32;
    /// Gets [`OsuHitObject<H>::time`].
    fn get_time(&self) -> Self::TimeType;
    /// Gets [`OsuHitObject<H>::end_time`].
    fn get_end_time(&self) -> Option<Self::TimeType>;
    /// Gets [`OsuHitObject<H>::volume`].
    fn get_volume(&self) -> u8;
    /// Gets [`OsuHitObject<H>::hitsound`].
    fn get_hitsound(&self) -> &str;
}

impl<H> HitObject for OsuHitObject<H>
where
    H: PartialOrd + Copy + Into<f64> + FromStr + HasVersion,
{
    type TimeType = H;

    fn version() -> &'static str {
        H::VERSION
    }

    fn parse(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        let effect_parts = parts.last().map_or(Vec::new(), |s| s.split(':').collect());

        if parts.len() < 6 {
            return None;
        }

        let x_pos = parts[0].parse().ok()?;
        let time = parts[2].parse::<Self::TimeType>().ok()?;
        let end_time = match parts[3] {
            "128" => effect_parts.first().and_then(|s| s.parse().ok()),
            _ => None,
        };

        let (volume, hitsound) = if effect_parts.len() >= 5
            && let [.., volume_str, hitsound_str] = effect_parts.as_slice()
        {
            (
                volume_str.parse().unwrap_or(0),
                if hitsound_str.is_empty() {
                    None
                } else {
                    Some(hitsound_str.to_string())
                },
            )
        } else {
            (0u8, None)
        };

        Some(Self {
            x_pos,
            time,
            end_time,
            volume,
            hitsound,
        })
    }

    fn to_legacy(self) -> OsuHitObjectLegacy {
        OsuHitObjectLegacy {
            x_pos: self.x_pos,
            time: self.time.into() as u32,
            end_time: self.end_time.map(|t| t.into() as u32),
            volume: self.volume,
            hitsound: self.hitsound,
        }
    }

    fn to_v128(self) -> OsuHitObjectV128 {
        OsuHitObjectV128 {
            x_pos: self.x_pos,
            time: self.time.into(),
            end_time: self.end_time.map(|t| t.into()),
            volume: self.volume,
            hitsound: self.hitsound,
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

    fn get_volume(&self) -> u8 {
        self.volume
    }

    fn get_hitsound(&self) -> &str {
        if let Some(hitsound) = &self.hitsound {
            hitsound
        } else {
            ""
        }
    }
}

/// Formats [`OsuHitObject`] back to osu! file format line.
///
/// Playfield height in [osu! pixels](https://osu.ppy.sh/wiki/en/Client/Beatmap_editor/osu%21_pixel)
/// is 384, thus y = 192 marks the center height, which is also used in osu!mania charts by default.
///
/// For types, osu! uses bit 0 to mark hit circle, 2 to mark new combo start, and 7 to mark osu!mania hold note.
/// We use 1 and 128 to represent hit circles and hold notes.
///
/// We use hitSound = 0 for normal hitsound.
/// For hitSample, we set normalSet and additionalSet = 0 to use normal sound set.
/// As referred in <https://osu.ppy.sh/wiki/en/Client/File_formats/osu_%28file_format%29#hitsounds>,
/// `<sampleSet>-hit<hitSound><index>.wav` will be played.
///
/// When filename is given, no addition sounds will be played.
/// Unlike [`OsuStoryboardSoundSample`], hitsound filename don't need to be quoted.
impl<H> fmt::Display for OsuHitObject<H>
where
    H: PartialOrd + Copy + Into<f64> + FromStr + HasVersion,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let time_float = self.get_time().into();
        // osu displays at most 12 digits of precision
        let time_str = if time_float.fract() < 1e-12 {
            format!("{:.0}", time_float)
        } else {
            format!("{:.12}", time_float)
        };

        let endtime_str = if let Some(t) = self.get_end_time() {
            let endtime_float: f64 = t.into();
            if endtime_float.fract() < 1e-12 {
                format!("{:.0}:", endtime_float)
            } else {
                format!("{:.12}:", endtime_float)
            }
        } else {
            String::new()
        };

        write!(
            f,
            "{},192,{},{},0,{}0:0:0:{}:{}",
            self.get_x_pos(),
            time_str,
            if endtime_str.is_empty() { 1 } else { 128 },
            endtime_str,
            self.get_volume(),
            self.get_hitsound()
        )
    }
}

/// The data structure of osu! file.
///
/// Reference: <https://osu.ppy.sh/wiki/en/Client/File_formats/osu_%28file_format%29>
#[derive(Debug, Clone)]
pub struct OsuData<H> {
    /// Miscellaneous data in section \[General\], \[Metadata\], \[Difficulty\] and background.
    pub misc: OsuMisc,
    /// Storyboard samples in section \[Events\], under //Storyboard Sound Samples.
    pub storyboard_samples: Vec<OsuStoryboardSoundSample>,
    /// Timing points and effects (inherited ones) in section \[TimingPoints\].
    pub timings: Vec<OsuTimingPoint>,
    /// Hit objects in section \[HitObjects\].
    pub notes: Vec<H>,
}

/// An enum to tell the section name while parsing osu! file.
#[derive(Debug)]
enum Section {
    /// Section \[General\]
    General,
    /// Section \[Editor\], ignored when parsing
    Editor,
    /// Section \[Metadata\]
    Metadata,
    /// Section \[Difficulty\]
    Difficulty,
    /// Section \[Events\]
    Events,
    /// Section \[TimingPoints\]
    TimingPoints,
    /// Section \[HitObjects\]
    HitObjects,
    /// Fallback section
    Unknown,
}

/// Parses section name from string.
impl From<&str> for Section {
    fn from(s: &str) -> Self {
        match s {
            "General" => Section::General,
            "Editor" => Section::Editor,
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
    /// Parses key-value pairs from lines under section \[General\], \[Metadata\] and \[Difficulty\].
    fn parse_key_value(line: &str) -> Option<(&str, &str)> {
        line.split_once(':').map(|(k, v)| (k.trim(), v.trim()))
    }

    /// Converts data to osu! file format v14.
    /// Only hit objects change [`HitObject::TimeType`] to [`u32`].
    pub fn to_legacy(self) -> OsuDataLegacy
    where
        H: HitObject,
    {
        OsuDataLegacy {
            misc: self.misc,
            storyboard_samples: self.storyboard_samples,
            timings: self.timings,
            notes: self.notes.into_iter().map(H::to_legacy).collect(),
        }
    }

    /// Converts data to osu! file format v128.
    /// Only hit objects change [`HitObject::TimeType`] to [`f64`].
    pub fn to_v128(self) -> OsuDataV128
    where
        H: HitObject,
    {
        OsuDataV128 {
            misc: self.misc,
            storyboard_samples: self.storyboard_samples,
            timings: self.timings,
            notes: self.notes.into_iter().map(H::to_v128).collect(),
        }
    }

    /// Rearrange [`Self::storyboard_samples`], [`Self::timings`] and [`Self::notes`] in time order.
    pub fn normalize(self) -> Self {
        let mut storyboard_samples = self.storyboard_samples;
        storyboard_samples.sort_unstable_by(|a, b| a.time.total_cmp(&b.time));

        let mut timings = self.timings;
        timings.sort_unstable_by(|a, b| a.time.total_cmp(&b.time));

        let mut notes = self.notes;
        notes.sort_unstable_by(|a, b| a.get_time().into().total_cmp(&b.get_time().into()));

        OsuData {
            misc: self.misc,
            storyboard_samples,
            timings,
            notes,
        }
    }

    /// Parses osu! file from file path.
    pub fn from_file(file_path: &str) -> Result<Self, io::Error> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        let mut misc = OsuMisc::default();
        let mut storyboard_samples = Vec::new();
        let mut timings = Vec::new();
        let mut notes = Vec::new();

        let mut current_section = Section::Unknown;

        for l in reader.lines() {
            let line = l?.trim().to_string();
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
                    // Parse key-value pairs
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
                        // Comments
                        continue;
                    }

                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 3 && parts[0] == "0" && parts[1] == "0" {
                        // Background
                        misc.background = parts[2].trim_matches('"').to_string();
                    } else if parts.len() == 5 && parts[0] == "Sample" {
                        // Sound sample
                        let time = parts[1].parse::<f64>();
                        let hitsound = parts[3].trim_matches('"');
                        let vol = parts[4].trim().parse::<u8>();
                        if let Ok(time) = time && !hitsound.is_empty() && let Ok(volume) = vol {
                            storyboard_samples.push(OsuStoryboardSoundSample {
                                time,
                                hitsound: hitsound.to_string(),
                                volume,
                            });
                        }
                    }
                }
                Section::TimingPoints => {
                    if let Some(timing) = OsuTimingPoint::parse(&line) {
                        timings.push(timing);
                    }
                }
                Section::HitObjects => {
                    if let Some(note) = H::parse(&line) {
                        notes.push(note);
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            misc,
            storyboard_samples,
            timings,
            notes,
        })
    }

    /// Writes osu! file to file path.
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
        write!(
            writer,
            "StackLeniency: 0.7\nMode: 3\nLetterboxInBreaks: 0\nSpecialStyle: 0\nWidescreenStoryboard: 1\n\n"
        )?;

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
        write!(
            writer,
            "HPDrainRate:8\nCircleSize:{}\nOverallDifficulty:{}\nApproachRate:5\nSliderMultiplier:1.4\nSliderTickRate:1\n\n",
            self.misc.circle_size, od_str
        )?;

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
        write!(writer, "//Storyboard Sound Samples\n")?;
        // OsuStoryboardSoundSample now implements fmt::Display
        for sample in self.storyboard_samples.iter() {
            write!(writer, "{}", sample)?;
        }

        // 构建 TimingPoints 部分
        // OsuTimingPoint now implements fmt::Display
        let timing_points: Vec<_> = self.timings.iter().map(|tp| tp.to_string()).collect();

        // 构建 HitObjects 部分
        // OsuHitObject now implements fmt::Display
        let hit_objects: Vec<_> = self.notes.iter().map(H::to_string).collect();

        write!(writer, "\n[TimingPoints]\n")?;
        writer.write_all(timing_points.join("\n").as_bytes())?;
        write!(writer, "\n\n[HitObjects]\n")?;
        writer.write_all(hit_objects.join("\n").as_bytes())?;

        Ok(())
    }

    /// Gets the range of BPM.
    /// # Return
    /// (min_bpm, max_bpm), constant BPM is max_bpm is [`None`]
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

    /// Gets beatmap time length in milliseconds
    fn get_length(&self) -> u32 {
        let (min_time, max_time) = self
            .notes
            .iter()
            .map(|n| {
                let start = n.get_time().into();
                let end = n.get_end_time().map(|t| t.into()).unwrap_or(start);
                (start, end.max(start))
            })
            .fold((f64::INFINITY, 0f64), |(min, max), (s, e)| {
                (min.min(s), max.max(e))
            });

        (max_time - min_time).max(0.0) as u32
    }

    /// Gets beatmap info.
    pub fn get_beatmap_info(&self, b_calc_sr: bool) -> BeatMapInfo {
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
            min_bpm,
            max_bpm,
            length,
            sr: if b_calc_sr {
                match calculate_from_data(&self.clone().to_legacy(), 1.0) {
                    Ok(sr) => Some(sr.max(0.0)),
                    Err(_) => None,
                }
            } else {
                None
            },
            note_count: note_count - ln_count,
            ln_count,
            bg_name: self.misc.background.clone(),
        }
    }
}

/// osu file format v14 generated by osu!stable, where hit objects uses [`u32`] for time type.
pub type OsuDataLegacy = OsuData<OsuHitObjectLegacy>;

/// osu file format v128 generated by osu!lazer, where hit objects uses [`f64`] for time type.
/// It is supposed that hit object times are floored to integers in gameplay judgements.
pub type OsuDataV128 = OsuData<OsuHitObjectV128>;

// 转换实现
impl From<OsuDataV128> for OsuDataLegacy {
    fn from(v: OsuDataV128) -> Self {
        v.to_legacy()
    }
}

// 为OsuHitObjectV128添加到Legacy的转换
impl From<OsuHitObjectV128> for OsuHitObjectLegacy {
    fn from(v: OsuHitObjectV128) -> Self {
        v.to_legacy()
    }
}

impl From<OsuDataLegacy> for OsuDataV128 {
    fn from(v: OsuDataLegacy) -> Self {
        v.to_v128()
    }
}

// 为OsuHitObjectV128添加到Legacy的转换
impl From<OsuHitObjectLegacy> for OsuHitObjectV128 {
    fn from(v: OsuHitObjectLegacy) -> Self {
        v.to_v128()
    }
}

impl OsuDataLegacy {
    /// Converts to Malody data structure.
    ///
    /// Mainly converts timing points and hit objects from milliseconds to BPM-based beats.
    pub fn to_mc_data(&self) -> McData {
        let osu_data_normalized = self.clone().normalize();
        // 轨道数
        let column_num = self.misc.circle_size;

        // malody的初始时间点可以认为是osu往回退第一个负数时间
        // 如果要严谨的话最好测onset，不过没必要
        let original_timings = osu_data_normalized
            .timings
            .iter()
            .filter(|t| t.is_timing)
            .collect::<Vec<_>>();
        let original_offset = original_timings.first().map_or(0.0, |t| t.time);
        // Uses 500ms as default interval (BPM 120) if there is no timing points, avoiding division by zero.
        let original_interval = original_timings
            .first()
            .map_or(500.0, |t| if t.val == 0.0 { 500.0 } else { t.val });
        let offset_beats = if original_offset > 0.0 {
            (original_offset / original_interval).ceil()
        } else {
            0.0
        };
        let offset = offset_beats * original_interval;

        let preview = self.misc.preview_time - offset as i32;

        let song = malody::Song {
            title: self.misc.title.clone(),
            artist: self.misc.artist.clone(),
            titleorg: Some(self.misc.title_unicode.clone()),
            artistorg: Some(self.misc.artist_unicode.clone()),
        };

        let mode_ext = malody::ModeExt {
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
            .map(|t| {
                // 把第一个时间点的timing和effect拉到新的offset上
                // epsilon参考Malody转osu，用1e-12
                OsuTimingPoint {
                    time: if (t.time - original_offset).abs() < 1e-12 {
                        t.time - offset.floor()
                    } else {
                        t.time
                    },
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
            })
            .collect::<Vec<_>>();

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
            .map(|(i, b)| malody::Timing {
                beat: b.to_vec(),
                bpm: 60000f64 / timings[i].val,
            })
            .collect::<Vec<_>>();

        // 顺便把效果也转换到mc
        let time_to_beat = |time: f64| -> Beat {
            let timing_index = timings.partition_point(|t| t.time < time).saturating_sub(1);
            let timing = &timings[timing_index];
            let beats = beat_counts_fold[timing_index];
            beats + Beat::from_float((time - timing.time) / timing.val)
        };

        let effects_grid = osu_data_normalized
            .timings
            .iter()
            .filter(|t| !t.is_timing)
            .map(|t| {
                let effect_time = t.time;
                let beat = time_to_beat(effect_time);
                malody::Effect {
                    beat: beat.to_vec(),
                    scroll: -100f64 / t.val,
                }
            })
            .collect::<Vec<_>>();

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
            let vol = if note.volume == 0 {
                None
            } else {
                Some(note.volume as i16)
            };

            Note {
                beat: start_grid.to_vec(),
                endbeat: end_grid.map(|e| e.to_vec()),
                column: Some(column as u8),
                sound: note.hitsound.clone(),
                vol,
                offset: None,
                r#type: None,
            }
        };

        let mut new_notes = self
            .notes
            .iter()
            .map(|n| to_grid(n))
            .collect::<Vec<_>>();

        // Malody最后一个音符是开始时间信息
        new_notes.push(Note {
            beat: vec![0, 0, 1],
            endbeat: None,
            column: None,
            sound: Some(self.misc.audio_file_name.clone()),
            vol: Some(100),
            offset: Some(timings[0].time.abs() as i32),
            r#type: Some(1),
        });

        // 以及自动播放的HitSound，对应osu的Storyboard Sound Samples
        let sound_samples = osu_data_normalized.storyboard_samples.iter().map(|sample| {
            let temp_osu_note = OsuHitObjectLegacy {
                x_pos: 0,
                time: sample.time as u32,
                end_time: None,
                volume: sample.volume,
                hitsound: Some(sample.hitsound.clone()),
            };
            let mut malody_hs_note = to_grid(&temp_osu_note);
            malody_hs_note.column = None;
            malody_hs_note.r#type = Some(1);
            malody_hs_note
        });
        new_notes.extend(sound_samples);

        McData {
            meta: mc_meta,
            time: beats_grid,
            effect: Some(effects_grid),
            note: new_notes,
        }
    }
}

/// Converts osu data to malody data.
impl<H> From<OsuData<H>> for McData
where
    H: HitObject + Clone,
{
    fn from(value: OsuData<H>) -> Self {
        value.to_legacy().to_mc_data()
    }
}
