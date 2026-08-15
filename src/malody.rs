mod mcz2osz;

use std::ops::Add;
use std::{
    fs::File,
    io::{self, BufReader, Read},
    ops::AddAssign,
};

use crate::osu::{
    OsuDataLegacy, OsuHitObjectLegacy, OsuMisc, OsuStoryboardSoundSample, OsuTimingPoint,
};

pub use self::mcz2osz::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Meta {
    pub creator: String,
    pub background: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<i32>,
    pub mode: u8,
    pub song: Song,
    pub mode_ext: ModeExt,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Song {
    pub title: String,
    pub artist: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titleorg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artistorg: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct ModeExt {
    pub column: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Beat {
    pub main_beat: u32,
    pub sub_beat: u32,
    pub div_beat: u32,
}

impl Beat {
    pub fn to_float(&self) -> f64 {
        let main_beat = self.main_beat as f64;
        let sub_beat = self.sub_beat as f64;
        let div_beat = self.div_beat as f64;

        main_beat + (sub_beat / div_beat)
    }

    pub fn from_float(val: f64) -> Self {
        const MAXIMUM_DIVISION: u32 = 16;
        const MAXIMUM_RESIDUAL: f64 = 0.5 / MAXIMUM_DIVISION as f64;

        let beat: u32 = val.floor() as u32;
        let fraction = val.fract();
        if (1.0 - fraction) < MAXIMUM_RESIDUAL {
            return Self {
                main_beat: beat + 1,
                sub_beat: 0,
                div_beat: 1,
            };
        }

        let (numerator, denominator) = (1..=16)
            .flat_map(|d| (0..d).map(move |n| (n, d)))
            .min_by(|&a, &b| {
                let residual_a = (fraction - a.0 as f64 / a.1 as f64).abs();
                let residual_b = (fraction - b.0 as f64 / b.1 as f64).abs();
                residual_a
                    .partial_cmp(&residual_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or((0, 1));

        Self {
            main_beat: beat,
            sub_beat: numerator,
            div_beat: denominator,
        }
    }

    pub fn to_vec(&self) -> Vec<u32> {
        vec![self.main_beat, self.sub_beat, self.div_beat]
    }
}

impl Add for Beat {
    type Output = Self;
    fn add(self, other: Beat) -> Beat {
        let val = self.to_float() + other.to_float();
        Beat::from_float(val)
    }
}

impl AddAssign for Beat {
    fn add_assign(&mut self, other: Beat) {
        let val = self.to_float() + other.to_float();
        *self = Beat::from_float(val);
    }
}

impl Default for Beat {
    fn default() -> Self {
        Self {
            main_beat: 0,
            sub_beat: 0,
            div_beat: 1,
        }
    }
}

impl From<&[u32]> for Beat {
    fn from(value: &[u32]) -> Self {
        assert!(value.len() == 3, "beat must be a 3-element array");
        Beat {
            main_beat: value[0],
            sub_beat: value[1],
            div_beat: value[2],
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Timing {
    pub beat: Vec<u32>,
    pub bpm: f64,
}
impl Timing {
    fn beat_to_float(&self) -> f64 {
        // 提取数组中的元素
        let beat_0 = self.beat[0] as f64;
        let beat_1 = self.beat[1] as f64;
        let beat_2 = self.beat[2] as f64;

        // 计算结果
        let result = beat_0 + (beat_1 / beat_2);

        // 返回结果
        result
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Effect {
    pub beat: Vec<u32>,
    pub scroll: f64,
}
impl Effect {
    fn beat_to_float(&self) -> f64 {
        // 提取数组中的元素
        let beat_0 = self.beat[0] as f64;
        let beat_1 = self.beat[1] as f64;
        let beat_2 = self.beat[2] as f64;

        // 计算结果
        let result = beat_0 + (beat_1 / beat_2);

        // 返回结果
        result
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Note {
    pub beat: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endbeat: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vol: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<u8>,
}
impl Note {
    pub fn beat_to_float(&self) -> f64 {
        // 提取数组中的元素
        let beat_0 = self.beat[0] as f64;
        let beat_1 = self.beat[1] as f64;
        let beat_2 = self.beat[2] as f64;

        // 计算结果
        let result = beat_0 + (beat_1 / beat_2);

        // 返回结果
        result
    }
    pub fn end_beat_to_float(&self) -> f64 {
        // 提取数组中的元素
        if let Some(end_beat) = &self.endbeat {
            let beat_0 = end_beat[0] as f64;
            let beat_1 = end_beat[1] as f64;
            let beat_2 = end_beat[2] as f64;

            // 计算结果
            let result = beat_0 + (beat_1 / beat_2);

            // 返回结果
            return result;
        }
        self.beat_to_float()
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct McData {
    pub meta: Meta,
    pub time: Vec<Timing>,
    pub effect: Option<Vec<Effect>>,
    pub note: Vec<Note>,
}

impl McData {
    pub fn from_file(file_path: &str) -> io::Result<Self> {
        // 打开文件并使用 BufReader 读取文件内容
        let file = File::open(file_path)?;
        let mut reader = BufReader::new(file);
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        if let Some(index) = content.find('{') {
            // 删除第一个 `{` 前的所有字符
            content.drain(..index);
        }

        // 解析 JSON 数据并转换为 McData 结构体
        let mc_data: McData = serde_json::from_str(&content)?;

        Ok(mc_data)
    }

    pub fn to_osu_data(&self) -> io::Result<OsuDataLegacy> {
        // 打印解析后的数据
        // println!("{:#?}", mc_data);

        // 检查模式是否为 0（Key 模式）
        if self.meta.mode != 0 {
            eprintln!("This program only supports Malody Chart in Key Mode!");
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "This program only supports Malody Chart in Key Mode!",
            ));
        }

        // 20260621修改
        // 自Malody4.3.7以来，负责音频Offset的虚拟note在note集合的最后一个元素
        // 但是老版本在第一个
        // 为了适应大批量旧谱转换，使用filter代替
        // 鉴别方式为type字段存在且为1
        // 音频音符包含起始音乐和所有的HitSounds，音频音符默认在0拍
        let mut audio_notes = self
            .note
            .iter()
            .filter(|n| n.r#type == Some(1))
            .collect::<Vec<_>>();
        audio_notes.sort_unstable_by(|a, b| {
            a.beat_to_float()
                .partial_cmp(&b.beat_to_float())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let audio_note = audio_notes.iter().find(|note| note.beat_to_float() == 0.0);
        let sound_effects = audio_notes
            .iter()
            .filter(|note| note.beat_to_float() != 0.0);
        let audio = audio_note
            .and_then(|n| n.sound.clone())
            .unwrap_or(String::new());

        let mut osu_data = OsuDataLegacy {
            misc: OsuMisc {
                audio_file_name: audio.clone(),
                preview_time: self.meta.preview.unwrap_or(-1),
                title: self.meta.song.title.clone(),
                title_unicode: self
                    .meta
                    .song
                    .titleorg
                    .clone()
                    .unwrap_or(self.meta.song.title.clone()),
                artist: self.meta.song.artist.clone(),
                artist_unicode: self
                    .meta
                    .song
                    .artistorg
                    .clone()
                    .unwrap_or(self.meta.song.artist.clone()),
                creator: self.meta.creator.clone(),
                version: self.meta.version.clone(),
                beatmap_id: 0,
                beatmap_set_id: -1,
                circle_size: self.meta.mode_ext.column as u32,
                od: 8.0,
                background: self.meta.background.clone(),
            },
            storyboard_samples: Vec::new(),
            timings: Vec::new(),
            notes: Vec::new(),
        };

        // 构建 TimingPoints 部分
        let offset_ms = audio_note
            .map(|n| n.offset.unwrap_or(0) as f64)
            .unwrap_or(0.0);
        if osu_data.misc.preview_time > offset_ms as i32 {
            osu_data.misc.preview_time += offset_ms as i32;
        }

        // 取第一个BPM为基准BPM
        let bpm_base = self
            .time
            .first()
            .map(|t| t.bpm)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing BPM data"))?;
        let interval_base = 60000_f64 / bpm_base;

        let mut bpm_list: Vec<(f64, u32, f64)> = Vec::new(); // 分别记录Malody的拍数,对应的osu内毫秒时刻和间隔时间

        for (index, item) in self.time.iter().enumerate() {
            if index == 0 {
                // 原先的计算是：考虑Malody的offset为x，那么Malody第0拍的音乐时间为-x
                // 因此计算要经过至少几个interval可以达到正时间，记为第一个timing point
                // 但是这样会导致小节开始线偏移。
                // 因此解决方案是，默认按4/4拍处理，第一个timing point就是离第一个物件最近的4的整倍数的拍子
                // 如果找不到就fallback到原处理
                let start_beat = (offset_ms / interval_base).ceil();

                // 记得过滤掉HS虚拟note
                let first_note_beat = self
                    .note
                    .iter()
                    .filter(|n| n.r#type != Some(1))
                    .map(|n| n.beat_to_float())
                    .min_by(|a, b| a.total_cmp(b))
                    .unwrap_or(0.0);
                let first_timepoint_beat = ((first_note_beat / 4.0).floor() * 4.0).max(start_beat);
                let first_timepoint_time =
                    (first_timepoint_beat * interval_base - offset_ms).floor() as u32;

                bpm_list.push((first_timepoint_beat, first_timepoint_time, interval_base));
                continue;
            }
            let current_beat = item.beat_to_float();
            let current_interval = 60000_f64 / item.bpm;

            let (old_beat, old_time, old_interval) = bpm_list[index - 1];
            let current_time = old_time + ((current_beat - old_beat) * old_interval) as u32;
            bpm_list.push((current_beat, current_time, current_interval));
        }

        let beat_to_time = |beat: f64| {
            // 处理空列表情况
            if bpm_list.is_empty() {
                return 0;
            }
            // 添加前导检查
            if beat < bpm_list[0].0 {
                return (beat * interval_base - offset_ms) as u32;
            }
            // 使用更安全的二分查找
            let idx = bpm_list
                .partition_point(|probe| probe.0 <= beat)
                .saturating_sub(1);

            let item = &bpm_list[idx];
            let interv = (item.2 * 1e12).round() / 1e12;
            item.1 + ((beat - item.0) * interv).floor() as u32
        };

        // 分别记录Malody的拍数,对应的osu内毫秒时刻和osu格式变速
        let effect_list: Vec<(f64, u32, f64)> = if let Some(effects) = &self.effect {
            effects
                .iter()
                .map(|effect| {
                    let current_beat = effect.beat_to_float();
                    let current_time = beat_to_time(current_beat);
                    let scroll = effect.scroll;
                    let osu_scroll = if scroll > 0_f64 {
                        -100_f64 / scroll
                    } else {
                        -100000000_f64
                    };
                    (current_beat, current_time, osu_scroll)
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut timings = [bpm_list.clone(), effect_list].concat();
        timings.sort_by_key(|x| x.1);
        osu_data.timings = timings
            .iter()
            .map(|&(_, time, scroll)| OsuTimingPoint {
                time: time as f64,
                val: scroll,
                is_timing: scroll > 0.0,
            })
            .collect();

        // 构建 HitObjects 部分
        let total_column = self.meta.mode_ext.column;
        let column_factor = 512.0 / total_column as f64;

        osu_data.notes = self
            .note
            .par_iter()
            .filter(|n| n.r#type != Some(1))
            .map(|item| {
                let item_time = beat_to_time(item.beat_to_float());
                let column = item.column.unwrap_or(0);
                let x_pos = ((column as f64 + 0.5) * column_factor).floor() as u32;
                // 处理 item 的 endbeat
                if let Some(_end_beat) = &item.endbeat {
                    let item_beat_end = item.end_beat_to_float();
                    let item_end_time = beat_to_time(item_beat_end);
                    OsuHitObjectLegacy {
                        x_pos: x_pos,
                        time: item_time,
                        end_time: Some(item_end_time),
                        volume: item.vol.map_or(0u8, |_| 100u8), // Malody <= 4.3.7 有bug，所有vol如果存在就是100，实际值无效
                        hitsound: item.sound.clone(),
                    }
                } else {
                    OsuHitObjectLegacy {
                        x_pos: x_pos,
                        time: item_time,
                        end_time: None,
                        volume: item.vol.map_or(0u8, |_| 100u8),
                        hitsound: item.sound.clone(),
                    }
                }
            })
            .collect();

        // 构建Sound Samples部分
        osu_data.storyboard_samples = sound_effects
            .map(|hs| {
                OsuStoryboardSoundSample {
                    time: beat_to_time(hs.beat_to_float()) as f64,
                    hitsound: hs.sound.clone().unwrap_or("".into()),
                    // Strangely, 这里的vol就没问题
                    volume: hs.vol.map_or(0u8, |v| v as u8),
                }
            })
            .collect();

        Ok(osu_data)
    }
}
