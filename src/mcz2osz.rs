use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::str;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};
use serde::Deserialize;
use rayon::prelude::*;

#[derive(Debug, Deserialize)]
struct Meta {
    creator: String,
    background: String,
    version: String,
    preview: Option<u32>,
    mode: u32,
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
    beat: Vec<u16>,
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
    beat: Vec<u16>,
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
    beat: Vec<u16>,
    endbeat: Option<Vec<u16>>,
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

/// Convert all .mcz files under given dir to .osz files.  
/// "." or "" will set dir to the Run Directory.
pub fn process_whole_dir_mcz(dir: &str) -> io::Result<()> {
    let current_dir = if dir == "" {"."} else {dir}; // 当前目录
    
    // 遍历当前目录下的所有文件
    for entry in WalkDir::new(current_dir) {
        let entry = entry?;
        let path = entry.path();
        
        // 检查文件扩展名是否为 .mcz
        if path.extension() == Some(std::ffi::OsStr::new("mcz")) {
            // 将 .mcz 文件转换为 .osz 文件
            let _ = process_mcz_file(path)?;
        }
    }
    
    Ok(())
}

pub fn process_mcz_file(path: &Path) -> io::Result<PathBuf> {
    // 创建解压缩后的文件夹
    let temp_dir = tempdir::TempDir::new("mcz_to_osz")?;
    let temp_dir_path = temp_dir.path();
    
    // 打开 .mcz 文件作为 ZIP 压缩文件
    let file = File::open(path)?;
    let mut zip_archive = ZipArchive::new(file)?;
    
    // 遍历 ZIP 压缩文件中的所有文件
    for i in 0..zip_archive.len() {
        let mut file = zip_archive.by_index(i)?;

        // 纯文件名，不含路径
        let file_name_bytes = file.name_raw();
        let translated_file_name = match str::from_utf8(file_name_bytes)
        {
            Ok(file_name) => {
                file_name.to_string()
            }
            Err(e) => {
                eprintln!("Failed to decode file name as UTF-8: {}", e);
                "invalid_utf8_name".to_string()
            }
        };
        let pure_file_name = Path::new(&translated_file_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap();

        // 清理非法字符并生成目标路径
        let sanitized = sanitize_filename(pure_file_name);
        let _s = &sanitized;
        let target_path = temp_dir_path.join(sanitized);

        // 将文件解压到临时目录中
        if file.is_file() {
            let mut output = File::create(&target_path)?;
            io::copy(&mut file, &mut output)?;
        }
    }

    // 在process_mcz_file中添加资源收集
    let mut required_files = HashSet::new();
    
    // 在临时文件夹中找到 .mc 文件并转换为 .osu 文件
    for entry in WalkDir::new(temp_dir_path) {
        let entry = entry?;
        let entry_path = entry.path();
        
        if entry_path.extension() == Some(std::ffi::OsStr::new("mc")) {
            // 解析并转换 .mc 文件为 .osu 文件
            let mut mc_data = match analyze_mc_file(&entry_path){
                Ok(data) => {
                    data
                }
                Err(e) => {
                    println!("Error analyzing file {:?}: {}", entry_path, e);
                    continue;
                }
            };

            // 对 mc_data 中的图片和音频文件名进行替代，并验证文件存在
            let sanitized_background = sanitize_filename(&mc_data.meta.background);
            let sanitized_audio = sanitize_filename(&mc_data.note.last().and_then(|n| n.sound.as_ref()).unwrap_or(&String::new()));
            if let Some(parent_path) = entry_path.parent() {
                let background_path = parent_path.join(&sanitized_background);
                let audio_path = parent_path.join(&sanitized_audio);

                if !background_path.exists() || !audio_path.exists() {
                    println!("{:?}, {:?}", background_path, audio_path);
                    println!("Warning: Some files specified in the mc file are missing.");
                    continue;
                }
                if sanitized_background != "" {
                    required_files.insert(background_path.clone());
                }
                if sanitized_audio != "" {
                    required_files.insert(audio_path.clone());
                }
            }
            
            
            mc_data.meta.background = sanitized_background;
            if let Some(note) = mc_data.note.last() {
                if let Some(_sound) = &note.sound {
                    let len = mc_data.note.len();
                    mc_data.note[len-1].sound = Some(sanitized_audio);
                }
            }
            // 转换 .mc 文件为 .osu 文件
            convert_mc_to_osu(&entry_path, &mc_data)?;
            let osu_file_path = entry_path.with_extension("osu");
            required_files.insert(osu_file_path);
        }
    }
    
    // 创建新的 .osz ZIP 文件
    let osz_file_path = path.with_extension("osz");
    println!("Generating .osz at: {:?}", osz_file_path);
    let osz_file = File::create(osz_file_path)?;
    let mut zip_writer = ZipWriter::new(osz_file);
    
    // 将临时文件夹中的文件添加到 .osz 文件中
    add_files_to_zip(&mut zip_writer, &required_files)?;
    
    // 完成写入
    zip_writer.finish()?;

    let output_file_path = path.with_extension("osz");
    Ok(output_file_path)
}

fn add_files_to_zip(
    zip_writer: &mut ZipWriter<File>,
    files: &HashSet<PathBuf>,
) -> io::Result<()> {
    let sorted_files: Vec<_> = files.iter().collect();
    
    for path in sorted_files {
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| io::Error::new(
                io::ErrorKind::InvalidInput, 
                "Invalid file name"
            ))?;
        
        let mut file = File::open(path)?;
        zip_writer.start_file(
            file_name,
            FileOptions::default()
                .compression_method(CompressionMethod::Stored)
        )?;
        io::copy(&mut file, zip_writer)?;
    }
    Ok(())
}

// Old function, adds everything to the zip, deprecated
// fn add_files_to_zip(
//     zip_writer: &mut ZipWriter<File>,
//     dir: &Path,
//     base_path: &Path,
// ) -> io::Result<()> {
//     // 遍历目录中的所有文件和文件夹
//     for entry in fs::read_dir(dir)? {
//         let entry = entry?;
//         let path = entry.path();
        
//         // 获取文件相对路径
//         let rel_path = path.strip_prefix(base_path).unwrap().to_str().unwrap();
        
//         if path.is_file() {
//             // 打开文件
//             let file = File::open(&path)?;
//             let mut file_reader = BufReader::new(file);
            
//             // 写入 ZIP 文件
//             zip_writer.start_file(rel_path, FileOptions::default().compression_method(CompressionMethod::Stored))?;
//             io::copy(&mut file_reader, zip_writer)?;
//         } else if path.is_dir() {
//             // 递归处理子目录
//             add_files_to_zip(zip_writer, &path, base_path)?;
//         }
//     }
    
//     Ok(())
// }

fn convert_mc_to_osu(mc_path: &Path, mc_data: &McData) -> io::Result<()> {
    // 打印解析后的数据
    // println!("{:#?}", mc_data);

    // 检查模式是否为 0（Key 模式）
    if mc_data.meta.mode != 0 {
        println!("This program only supports Malody Chart in Key Mode!");
        return Ok(());
    }

    let mut audio = String::new();
    // 打印音频信息
    if let Some(note) = mc_data.note.last() {
        if let Some(sound) = &note.sound {
            audio = sound.to_string();
        }
    }

    let mut osu_path = PathBuf::from(mc_path);
    // 获取文件名部分（带后缀）
    if let Some(file_stem) = osu_path.file_stem() {
        // 重新组合路径
        osu_path.set_file_name(format!("{}.osu", file_stem.to_string_lossy()));
    }

    println!("Generating .osu file at: {:?}", osu_path);
    // 创建和写入 .osu 文件
    let osu_file = File::create(osu_path)?;
    let mut writer = BufWriter::new(osu_file);

    // 您可以继续根据解析后的数据进行其他操作
    // 构建 General 部分
    write!(writer, "osu file format v14\n\n[General]\n")?;
    write!(writer, "AudioFilename: {}\n", audio)?;
    write!(writer, "AudioLeadIn: 0\nPreviewTime: {}\nCountdown: 0\nSampleSet: Soft\n", &mc_data.meta.preview.unwrap_or(0))?;
    write!(writer, "StackLeniency: 0.7\nMode: 3\nLetterboxInBreaks: 0\nSpecialStyle: 0\nWidescreenStoryboard: 1\n\n")?;

    // 构建 Editor 部分
    write!(writer, "[Editor]\nDistanceSpacing: 1\nBeatDivisor: 8\nGridSize: 4\nTimelineZoom: 2\n\n")?;

    // 构建 Metadata 部分
    write!(writer, "[Metadata]\n")?;
    write!(writer, "Title:{}\n", mc_data.meta.song.titleorg.as_deref().unwrap_or(&mc_data.meta.song.title))?;
    write!(writer, "TitleUnicode:{}\n", mc_data.meta.song.title)?;
    write!(writer, "Artist:{}\n", mc_data.meta.song.artistorg.as_deref().unwrap_or(&mc_data.meta.song.artist))?;
    write!(writer, "ArtistUnicode:{}\n", mc_data.meta.song.artist)?;
    write!(writer, "Creator:{}\n", mc_data.meta.creator)?;
    write!(writer, "Version:{}\n", mc_data.meta.version)?;
    write!(writer, "Source:\nTags:\nBeatmapID:0\nBeatmapSetID:-1\n\n")?;

    // 构建 Difficulty 部分
    write!(writer, "[Difficulty]\n")?;
    write!(writer, "HPDrainRate:8\nCircleSize:{}\nOverallDifficulty:8\nApproachRate:5\nSliderMultiplier:1.4\nSliderTickRate:1\n\n", mc_data.meta.mode_ext.column)?;

    // 构建 Events 部分
    write!(writer, "[Events]\n//Background and Video events\n")?;
    if !mc_data.meta.background.is_empty() {
        write!(writer, "0,0,\"{}\",0,0\n", mc_data.meta.background)?;
    }
    write!(writer, "//Break Periods\n//Storyboard Layer 0 (Background)\n")?;
    write!(writer, "//Storyboard Layer 1 (Fail)\n//Storyboard Layer 2 (Pass)\n")?;
    write!(writer, "//Storyboard Layer 3 (Foreground)\n//Storyboard Layer 4 (Overlay)\n")?;
    write!(writer, "//Storyboard Sound Samples\n\n")?;

    // 构建 TimingPoints 部分
    
    // 这里需要实现 BPM 和效果列表的处理
    // 这里用来写新的实现方法：
    let offset_ms = if let Some(note) = mc_data.note.last() {
        note.offset.unwrap_or(0)
    } else {
        0
    } as f64;
    
    // 取第一个BPM为基准BPM
    let bpm_base = mc_data.time.get(0)
        .map(|t| t.bpm as f64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing BPM data"))?;
    let interval_base = 60000_f64 / bpm_base;
    // let used = (interval_base * 10e11).round() / 10e11;
    // write!(writer, "//Original = {interval_base}, Used = {used}\n\n")?; // 仅做测试！
    write!(writer, "[TimingPoints]\n")?;
    
    let mut bpm_list: Vec<(f64, u64, f64)> = Vec::new(); // 分别记录Malody的拍数,对应的osu内毫秒时刻和间隔时间
    
    for (index, item) in mc_data.time.iter().enumerate() {
        if index == 0 {
            let start_beat = (offset_ms as f64 / interval_base).ceil();
            let start_time = (start_beat * interval_base - offset_ms).floor() as u64;

            bpm_list.push((start_beat, start_time, interval_base));
            continue;
        }
        let current_beat = item.beat_to_float();
        let current_interval = 60000_f64 / item.bpm;
        
        let (old_beat, old_time, old_interval) = bpm_list[index-1];
        let current_time = old_time + ((current_beat - old_beat) * old_interval) as u64;
        bpm_list.push((current_beat, current_time, current_interval));
    }

    let beat_to_time = |beat: f64| {
        // 处理空列表情况
        if bpm_list.is_empty() {
            return 0;
        }
        // 添加前导检查
        if beat < bpm_list[0].0 {
            return (beat * interval_base - offset_ms) as u64;
        }
        // 使用更安全的二分查找
        let idx = bpm_list.partition_point(
            |probe| probe.0 <= beat).saturating_sub(1);
        
        let item = &bpm_list[idx];
        let interv = (item.2 * 10e11).round() / 10e11;
        item.1 + ((beat - item.0) * interv).floor() as u64
    };

    let mut effect_list: Vec<(f64, u64, f64)> = Vec::new(); // 分别记录Malody的拍数,对应的osu内毫秒时刻和osu格式变速

    if let Some(effects) = &mc_data.effect {
        for (_index, item) in effects.iter().enumerate() {
            let current_beat = item.beat_to_float();
            let current_time = beat_to_time(current_beat);
            let scroll = item.scroll;
            let osu_scroll = if scroll > 0_f64 {
                -100_f64 / scroll
            } else {
                -100000000_f64
            };
            effect_list.push((current_beat, current_time, osu_scroll));
        }
    }

    // 不会有谱面有一百万个timing吧，不用双指针了，怎么简单怎么来咯
    // let timings = [bpm_list, effect_list].concat().sort_by_key(|x| x.1);
    // 合并两个有序列表（假设bpm_list和effect_list已按current_time升序排列）
    let min_time = bpm_list.first().map(|x| x.1).unwrap_or(0);

    let mut i = 0;  // bpm_list指针
    // 跳过早于bpm起始时间的effect元素
    let mut j = effect_list.partition_point(|x| x.1 < min_time);

    while i < bpm_list.len() && j < effect_list.len() {
        let bpm = &bpm_list[i];
        let eff = &effect_list[j];

        // 比较时间戳并优先写入较小的
        if bpm.1 <= eff.1 {
            write!(writer, "{},{:.12},4,2,0,10,1,0\n", bpm.1, bpm.2)?;
            i += 1;
        } else {
            write!(writer, "{},{:.12},4,2,0,10,0,0\n", eff.1, eff.2)?;
            j += 1;
        }
    }

    // 处理剩余元素
    while i < bpm_list.len() {
        write!(writer, "{},{:.12},4,2,0,10,1,0\n", bpm_list[i].1, bpm_list[i].2)?;
        i += 1;
    }

    while j < effect_list.len() {
        write!(writer, "{},{:.12},4,2,0,10,0,0\n", effect_list[j].1, effect_list[j].2)?;
        j += 1;
    }

    // 构建 HitObjects 部分
    let total_column = mc_data.meta.mode_ext.column;
    let column_factor = 512.0 / total_column as f64;
    write!(writer, "\n[HitObjects]\n")?;

    let hit_objects: Vec<_> = mc_data.note[..mc_data.note.len()-1]
    .par_iter()
    .map(|item| {
        let item_time = beat_to_time(item.beat_to_float());
        let column = item.column.unwrap_or(0);
        let x_pos = ((column as f64 + 0.5) * column_factor as f64).floor() as u64;
        // 处理 item 的 endbeat
        if let Some(_end_beat) = &item.endbeat {
            let item_beat_end = item.end_beat_to_float();
            let item_end_time = beat_to_time(item_beat_end);
            format!("{},192,{},128,0,{}:0:0:0:0:",
                x_pos, item_time, item_end_time)
        } else {
            let start_flag = if i == 0 {5} else {1};
            // 没有 endbeat 的情况
            format!("{},192,{},{},0,0:0:0:0:",
                x_pos, item_time, start_flag)
        }
    })
    .collect();

    writer.write_all(hit_objects.join("\n").as_bytes())?;
    
    Ok(())
}

fn analyze_mc_file(file_path: &Path) -> io::Result<McData> {
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

fn sanitize_filename(file_name: &str) -> String {
    // 将文件名中的非ASCII字符替换为下划线
    file_name.chars()
        .map(|c| if c.is_ascii() { c } else { '_' })
        .collect()
}