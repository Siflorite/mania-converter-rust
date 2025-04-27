mod mcz2osz;

pub use self::mcz2osz::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Meta {
    creator: String,
    background: String,
    version: String,
    preview: Option<u32>,
    mode: u8,
    song: Song,
    mode_ext: ModeExt,
}
#[derive(Debug, Deserialize)]
struct Song {
    title: String,
    artist: String,
    titleorg: Option<String>,
    artistorg: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ModeExt {
    column: u8,
}
#[derive(Debug, Deserialize)]
struct Beat {
    beat: Vec<u32>,
    bpm: f64,
}
impl Beat {
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
#[derive(Debug, Deserialize)]
struct Effect {
    beat: Vec<u32>,
    scroll: f64,
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
#[derive(Debug, Deserialize)]
struct Note {
    beat: Vec<u32>,
    endbeat: Option<Vec<u32>>,
    column: Option<u8>,
    sound: Option<String>,
    // vol: Option<i16>,
    offset: Option<u32>,
    // r#type: Option<u8>,
}
impl Note {
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
    fn end_beat_to_float(&self) -> f64 {
        // 提取数组中的元素
        if let Some(end_beat) = &self.endbeat{
            let beat_0 = end_beat[0] as f64;
            let beat_1 = end_beat[1] as f64;
            let beat_2 = end_beat[2] as f64;
        
            // 计算结果
            let result = beat_0 + (beat_1 / beat_2);
        
            // 返回结果
            return result
        }
        self.beat_to_float()
    }
}
#[derive(Debug, Deserialize)]
struct McData {
    meta: Meta,
    time: Vec<Beat>,
    effect: Option<Vec<Effect>>,
    note: Vec<Note>,
}