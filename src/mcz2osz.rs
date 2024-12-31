use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::str;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};
use serde::Deserialize;

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
    bpm: f32,
}
impl Beat {
    fn beat_to_float(&self) -> f32 {
        // 提取数组中的元素
        let beat_0 = self.beat[0] as f32;
        let beat_1 = self.beat[1] as f32;
        let beat_2 = self.beat[2] as f32;
    
        // 计算结果
        let result = beat_0 + (beat_1 / beat_2);
    
        // 返回结果
        result
    }
}
#[derive(Debug, Deserialize)]
struct Effect {
    beat: Vec<u16>,
    scroll: f32,
}
impl Effect {
    fn beat_to_float(&self) -> f32 {
        // 提取数组中的元素
        let beat_0 = self.beat[0] as f32;
        let beat_1 = self.beat[1] as f32;
        let beat_2 = self.beat[2] as f32;
    
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
    fn beat_to_float(&self) -> f32 {
        // 提取数组中的元素
        let beat_0 = self.beat[0] as f32;
        let beat_1 = self.beat[1] as f32;
        let beat_2 = self.beat[2] as f32;
    
        // 计算结果
        let result = beat_0 + (beat_1 / beat_2);
    
        // 返回结果
        result
    }
    fn end_beat_to_float(&self) -> f32 {
        // 提取数组中的元素
        if let Some(end_beat) = &self.endbeat{
            let beat_0 = end_beat[0] as f32;
            let beat_1 = end_beat[1] as f32;
            let beat_2 = end_beat[2] as f32;
        
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

pub fn process_whole_dir_mcz() -> io::Result<()> {
    let current_dir = "."; // 当前目录
    
    // 遍历当前目录下的所有文件
    for entry in WalkDir::new(current_dir) {
        let entry = entry?;
        let path = entry.path();
        
        // 检查文件扩展名是否为 .mcz
        if path.extension() == Some(std::ffi::OsStr::new("mcz")) {
            // 将 .mcz 文件转换为 .osz 文件
            process_mcz_file(path)?;
        }
    }
    
    Ok(())
}

pub fn process_mcz_file(path: &Path) -> io::Result<()> {
    // 创建解压缩后的文件夹
    let temp_dir = tempdir::TempDir::new("mcz_to_osz")?;
    let temp_dir_path = temp_dir.path();
    
    // 打开 .mcz 文件作为 ZIP 压缩文件
    let file = File::open(path)?;
    let mut zip_archive = ZipArchive::new(file)?;
    
    // 遍历 ZIP 压缩文件中的所有文件
    for i in 0..zip_archive.len() {
        let mut file = zip_archive.by_index(i)?;
        let file_name_bytes = file.name_raw();
        let mut final_file = String::new();
        match str::from_utf8(file_name_bytes)
        {
            Ok(file_name) => {
                final_file = file_name.to_string();
            }
            Err(e) => {
                println!("Failed to decode file name as UTF-8: {}", e);
            }
        }
        // 将字符串 `s` 使用 CP437 编码器编码为字节数组
        // match file_name.to_cp437(&CP437_CONTROL){
        //     Ok(cow) =>{
        //         match cow {
        //             // 如果是借用的内容，直接返回引用
        //             Cow::Borrowed(borrowed) => {
        //                 let (decoded, _, _had_errors) = UTF_8.decode(borrowed);
        //                 final_file = decoded.to_string();
        //             },
        //             // 如果是拥有的内容，转换为借用并返回
        //             Cow::Owned(owned) => {
        //                 let (decoded, _, _had_errors) = UTF_8.decode(&owned);
        //                 final_file = decoded.to_string();
        //             },
        //         };
        //     }
        //     Err(_e) => {
                
        //     }
        // }
        // println!("{}", final_file);
        // 将文件名中的非ASCII字符替换为下划线
        let sanitized_file_name = sanitize_filename(&final_file);
        let target_path = temp_dir_path.join(&sanitized_file_name);
        // println!("{:?}", target_path);
        // 创建目标目录（包含子文件夹）
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 将文件解压到临时目录中
        if file.is_file() {
            let mut output_file = BufWriter::new(File::create(&target_path)?);
            io::copy(&mut file, &mut output_file)?;
        }
    }
    
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
        }
    }
    
    // 创建新的 .osz ZIP 文件
    let osz_file_path = path.with_extension("osz");
    println!("Generating .osz at: {:?}", osz_file_path);
    let osz_file = File::create(osz_file_path)?;
    let mut zip_writer = ZipWriter::new(osz_file);
    
    // 将临时文件夹中的文件添加到 .osz 文件中
    add_files_to_zip(&mut zip_writer, temp_dir_path, temp_dir_path)?;
    
    // 完成写入
    zip_writer.finish()?;
    
    Ok(())
}

fn add_files_to_zip(
    zip_writer: &mut ZipWriter<File>,
    dir: &Path,
    base_path: &Path,
) -> io::Result<()> {
    // 遍历目录中的所有文件和文件夹
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        // 获取文件相对路径
        let rel_path = path.strip_prefix(base_path).unwrap().to_str().unwrap();
        
        if path.is_file() {
            // 打开文件
            let file = File::open(&path)?;
            let mut file_reader = BufReader::new(file);
            
            // 写入 ZIP 文件
            zip_writer.start_file(rel_path, FileOptions::default().compression_method(CompressionMethod::Stored))?;
            io::copy(&mut file_reader, zip_writer)?;
        } else if path.is_dir() {
            // 递归处理子目录
            add_files_to_zip(zip_writer, &path, base_path)?;
        }
    }
    
    Ok(())
}

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

    // 您可以继续根据解析后的数据进行其他操作
    // 构建 General 部分
    let mut osu_file_content = String::new();
    osu_file_content.push_str("osu file format v14\n\n[General]\n");
    osu_file_content.push_str(&format!("AudioFilename: {}\n", audio));
    osu_file_content.push_str(&format!("AudioLeadIn: 0\nPreviewTime: {}\nCountdown: 0\nSampleSet: Soft\n", &mc_data.meta.preview.unwrap_or(0)));
    osu_file_content.push_str("StackLeniency: 0.7\nMode: 3\nLetterboxInBreaks: 0\nSpecialStyle: 0\nWidescreenStoryboard: 1\n\n");

    // 构建 Editor 部分
    osu_file_content.push_str("[Editor]\nDistanceSpacing: 1\nBeatDivisor: 8\nGridSize: 4\nTimelineZoom: 2\n\n");

    // 构建 Metadata 部分
    osu_file_content.push_str("[Metadata]\n");
    osu_file_content.push_str(&format!("Title:{}\n", mc_data.meta.song.titleorg.as_deref().unwrap_or(&mc_data.meta.song.title)));
    osu_file_content.push_str(&format!("TitleUnicode:{}\n", mc_data.meta.song.title));
    osu_file_content.push_str(&format!("Artist:{}\n", mc_data.meta.song.artistorg.as_deref().unwrap_or(&mc_data.meta.song.artist)));
    osu_file_content.push_str(&format!("ArtistUnicode:{}\n", mc_data.meta.song.artist));
    osu_file_content.push_str(&format!("Creator:{}\n", mc_data.meta.creator));
    osu_file_content.push_str(&format!("Version:{}\n", mc_data.meta.version));
    osu_file_content.push_str("Source:\nTags:\nBeatmapID:0\nBeatmapSetID:-1\n\n");

    // 构建 Difficulty 部分
    osu_file_content.push_str("[Difficulty]\n");
    osu_file_content.push_str(&format!("HPDrainRate:8\nCircleSize:{}\nOverallDifficulty:8\nApproachRate:5\nSliderMultiplier:1.4\nSliderTickRate:1\n\n", mc_data.meta.mode_ext.column));

    // 构建 Events 部分
    osu_file_content.push_str("[Events]\n//Background and Video events\n");
    if !mc_data.meta.background.is_empty() {
        osu_file_content.push_str(&format!("0,0,\"{}\",0,0\n", mc_data.meta.background));
    }
    osu_file_content.push_str("//Break Periods\n//Storyboard Layer 0 (Background)\n");
    osu_file_content.push_str("//Storyboard Layer 1 (Fail)\n//Storyboard Layer 2 (Pass)\n");
    osu_file_content.push_str("//Storyboard Layer 3 (Foreground)\n//Storyboard Layer 4 (Overlay)\n");
    osu_file_content.push_str("//Storyboard Sound Samples\n\n");

    // 构建 TimingPoints 部分
    osu_file_content.push_str("[TimingPoints]\n");
    // 这里需要实现 BPM 和效果列表的处理
    // 根据您的 Python 代码逻辑，处理 mc_data.time 和 mc_data.effect 列表
    let offset_ms = if let Some(note) = mc_data.note.last() {
        note.offset.unwrap_or(0)
    } else {
        0
    };
    let mut current_time = 0.0 - offset_ms as f32;
    let mut current_beat = mc_data.time[0].beat_to_float();
    let mut next_beat = current_beat;
    let bpm_base = mc_data.time[0].bpm;
    let mut ms_per_beat = 60000.0 / bpm_base;
    let mut index_effect = 0;
    let mut bpm_check_list: Vec<(f32, f32)> = Vec::new();
    let mut start_time = current_time;
    let mut start_beat = 0.0; 
    // 处理 BPM 列表
    for (index, item) in mc_data.time.iter().enumerate() {
        current_beat = item.beat_to_float();
        ms_per_beat = 60000.0 / item.bpm;
        bpm_check_list.push((current_beat, ms_per_beat));
        if index == 0 {
            while start_time < 0.0 {
                start_time += ms_per_beat;
                start_beat += 1.0;
            }
            start_time = start_time.floor();
            osu_file_content.push_str(&format!("{},{:.12},4,2,0,10,1,0\n", start_time.floor() as i32, ms_per_beat));
        } else {
            osu_file_content.push_str(&format!("{},{:.12},4,2,0,10,1,0\n", current_time.floor() as i32, ms_per_beat));
        }
        if index < mc_data.time.len() - 1 {
            next_beat = mc_data.time[index+1].beat_to_float();
        } else {
            break;
        }
        // 处理 effect_list 中的内容
        if let Some(effect_list) = &mc_data.effect {                    
            while index_effect < effect_list.len() {
                let effect_beat = &effect_list[index_effect].beat_to_float();
                // 检查 effectBeat 和 current_beat 的关系
                if effect_beat >= &current_beat && effect_beat < &next_beat {
                    let effect_time = current_time + (effect_beat - current_beat) * ms_per_beat;
                    // 检查滚动速度
                    if effect_list[index_effect].scroll < 0.0 {
                        index_effect += 1;
                        continue;
                    }
                    let speed_variation = if effect_list[index_effect].scroll != 0.0 {
                        -100.0 / effect_list[index_effect].scroll
                    } else {
                        -100000000.0
                    };
        
                    osu_file_content.push_str(&format!("{},{:.12},4,2,0,10,0,0\n", effect_time.floor() as i32, speed_variation));
                    index_effect += 1;
                } else {
                    break;
                }
            }
        }
        current_time += (next_beat - current_beat) * ms_per_beat;
    }
    // 处理剩余的 effect 列表
    if let Some(effect_list) = &mc_data.effect {
        while index_effect < effect_list.len() {
            let effect_beat = &effect_list[index_effect].beat_to_float();
            let effect_time = current_time + (effect_beat - current_beat) * ms_per_beat;
            // 检查滚动速度
            if effect_list[index_effect].scroll < 0.0 {
                index_effect += 1;
                continue;
            }
            let speed_variation = if effect_list[index_effect].scroll != 0.0 {
                -100.0 / effect_list[index_effect].scroll
            } else {
                -100000000.0
            };
            osu_file_content.push_str(&format!("{},{:.12},4,2,0,10,0,0\n", effect_time.floor() as i32, speed_variation));
            index_effect += 1;
        }
    }
    osu_file_content.push_str("\n");
    // 构建 HitObjects 部分
    osu_file_content.push_str("[HitObjects]\n");
    // 这里需要实现 noteList 的处理
    // 根据您的 Python 代码逻辑，处理 mc_data.note 列表
    fn get_ms_from_beat(beat: f32, bpm_check_list: &[(f32, f32)]
            ,start_time: f32, start_beat: f32) -> f32
    {
        let mut time = start_time;
        let mut beat_index = 1;
        let mut cur_bpm_beat = start_beat;
        let mut next_bpm_beat = if bpm_check_list.len() > 1 {
            bpm_check_list[beat_index].0
        } else {
            cur_bpm_beat
        };
        let mut cur_ms_per_beat = bpm_check_list[0].1;

        while beat > next_bpm_beat && beat_index - 1 < bpm_check_list.len() {
            time += cur_ms_per_beat * (next_bpm_beat - cur_bpm_beat);
            cur_bpm_beat = next_bpm_beat;
            cur_ms_per_beat = if bpm_check_list.len() > 1 {
                bpm_check_list[beat_index].1
            } else {
                cur_ms_per_beat
            };
            beat_index += 1;
            if beat_index >= bpm_check_list.len() {
                break;
            }
            next_bpm_beat = bpm_check_list[beat_index].0;
            time = time.floor();
        }
        time += (beat - cur_bpm_beat) * cur_ms_per_beat;
        time
    }

    // 遍历 noteList 来生成 HitObjectsList
    for (i,item) in mc_data.note.iter().enumerate() {
        // 获取节拍和时间
        if i == &mc_data.note.len() - 1 {break;}
        let item_beat = item.beat_to_float();
        let beat_time = get_ms_from_beat(item_beat, &bpm_check_list, start_time, start_beat);
        let mut x_pos = 0;
        // 计算 X 位置
        if let Some(column) = item.column
        {
            x_pos = ((column as f64 + 0.5) * 512.0 / mc_data.meta.mode_ext.column as f64) as u32;
        }
        // 处理 item 的 endbeat
        if let Some(_end_beat) = &item.endbeat {
            let item_beat_end = item.end_beat_to_float();
            let beat_end_time = get_ms_from_beat(item_beat_end, &bpm_check_list, start_time, start_beat);
            osu_file_content.push_str(&format!(
                "{},192,{},128,0,{}:0:0:0:0:\n",
                x_pos,
                beat_time.floor() as i32,
                beat_end_time.floor() as i32
            ));
        } else {
            let start_flag = if i == 0 {5} else {1};
            // 没有 endbeat 的情况
            osu_file_content.push_str(&format!(
                "{},192,{},{},0,0:0:0:0:\n",
                x_pos,
                beat_time.floor() as i32,
                start_flag,
            ));
        }
    }

    // 将内容写入 .osu 文件
    // 使用 PathBuf 来处理文件路径
    let mut osu_path = PathBuf::from(mc_path);

    // 获取文件名部分（带后缀）
    if let Some(file_stem) = osu_path.file_stem() {
        // 重新组合路径
        osu_path.set_file_name(format!("{}.osu", file_stem.to_string_lossy()));
    }

    // 打印生成的 .osu 文件路径
    println!("Generating .osu file at: {:?}", osu_path);

    // 创建和写入 .osu 文件
    let mut osu_file = File::create(osu_path)?;
    osu_file.write_all(osu_file_content.as_bytes())?;
    
    Ok(())
}

fn analyze_mc_file(file_path: &Path) -> io::Result<McData> {
    // 打开文件并使用 BufReader 读取文件内容
    let file = File::open(file_path).unwrap(); // 将 io::Error 转换为 serde_json::Error
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