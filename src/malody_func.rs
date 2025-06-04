mod mcz2osz;

pub use self::mcz2osz::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub creator: String,
    pub background: String,
    pub version: String,
    pub preview: Option<i32>,
    pub mode: u8,
    pub song: Song,
    pub mode_ext: ModeExt,
}
#[derive(Debug, Deserialize)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub titleorg: Option<String>,
    pub artistorg: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct ModeExt {
    pub column: u8,
}
#[derive(Debug, Deserialize)]
pub struct Beat {
    pub beat: Vec<u32>,
    pub bpm: f64,
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
#[derive(Debug, Deserialize)]
pub struct Note {
    pub beat: Vec<u32>,
    pub endbeat: Option<Vec<u32>>,
    pub column: Option<u8>,
    pub sound: Option<String>,
    pub vol: Option<i16>,
    pub offset: Option<i32>,
    pub r#type: Option<u8>,
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
pub struct McData {
    pub meta: Meta,
    pub time: Vec<Beat>,
    pub effect: Option<Vec<Effect>>,
    pub note: Vec<Note>,
}