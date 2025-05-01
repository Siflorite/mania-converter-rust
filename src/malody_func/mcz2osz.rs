use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::str;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};
use rayon::prelude::*;

use crate::osu_func::{OsuData, OsuMisc, OsuTimingPoint, OsuHitObject};
use crate::malody_func::McData;
use crate::BeatMapInfo;

/// Convert all .mcz files under given dir to .osz files.  
/// "." or "" will set dir to the Run Directory.
pub fn process_whole_dir_mcz(dir: &str, b_calc_sr: bool, b_print_results: bool) -> io::Result<()> {
    let current_dir = if dir == "" {"."} else {dir}; // 当前目录
    // let results_queue = Arc::new(SegQueue::<(PathBuf, Vec<BeatMapInfo>)>::new());

    // 遍历当前目录下的所有文件
    let processed: Vec<_> = WalkDir::new(current_dir).into_iter().par_bridge()
        .filter_map(|entry| {
        let entry = entry.ok()?;
        let path = entry.path();

        // 检查文件扩展名是否为 .mcz
        if path.extension() == Some(std::ffi::OsStr::new("mcz")) {
            // 将 .mcz 文件转换为 .osz 文件
            match process_mcz_file(path, b_calc_sr) {
                Ok(info_tuple) => Some(info_tuple),
                Err(e) => {
                    eprintln!("Error processing {}: {}", path.display(), e);
                    None
                }
            } 
        } else { None }
    }).collect();

    // 收集结果
    // If you really want to use SegQueue, you must manually pop out as Arc referces can't be moved
    // Even though SeqQueue provides a `into_iter()` function... But no Copy Trait...
    // let processed: Vec<_> = results_queue.into_iter().collect(); <- Illegal!
    // while let Some(item) = results_queue.pop() processed.push(item);

    if b_print_results {
        println!("\nConversion Summary:");
        println!("{:-<80}", "");
        for (path, info) in processed.iter() {
            println!("OSZ File: {}", path.display());
            println!("Contains {} beatmaps:", info.len());
            for beatmap in info.iter() {
                println!("\n{beatmap}");
            }
            println!("{:-<80}\n", "");
        }
        let total_beatmaps: usize = processed.iter().map(|(_, v)| v.len()).sum();
        println!("Total processed files: {}", processed.len());
        println!("Total converted beatmaps: {}", total_beatmaps);
    }
    
    Ok(())
}

/// 将mcz文件转换为osz文件，处理完成后执行后处理函数，可以实现难度图生成等功能<br>
/// 输入参数：mcz文件路径，后处理函数 （默认计算星级）<br>
/// 后处理函数参数：内部谱面信息，存放.osu, .mc文件和音乐与背景的临时目录<br>
/// 输出结果：osz文件路径
/// 由于函数执行完后临时目录会被清除，请不要将生成的内容存放于临时目录中
pub fn process_mcz_file_postprocess<F>(
    path: &Path,
    mut post_process: F,
) -> io::Result<PathBuf>
where
    F: FnMut(&[BeatMapInfo], &Path) -> io::Result<()>,
{
    let temp_dir = tempdir::TempDir::new("mcz_to_osz")?;
    let temp_dir_path = temp_dir.path();
    
    // 使用原有核心处理逻辑，默认计算难度
    let (osz_path, mut beatmap_infos) = process_mcz_core(path, temp_dir_path, true)?;
    beatmap_infos.sort_by(|x, y| {
        x.sr.partial_cmp(&y.sr).unwrap()
    });
    // 执行后处理闭包
    post_process(&beatmap_infos, temp_dir_path)?;
    
    Ok(osz_path)
}

/// 将mcz文件转换为osz文件<br>
/// 输入参数：mcz文件路径，是否计算星级<br>
/// 输出结果：osz文件路径，内部谱面信息
pub fn process_mcz_file(path: &Path, b_calc_sr: bool) -> io::Result<(PathBuf, Vec<BeatMapInfo>)> {
    // 创建解压缩后的文件夹
    let temp_dir = tempdir::TempDir::new("mcz_to_osz")?;
    let temp_dir_path = temp_dir.path();

    // 正经处理过程
    process_mcz_core(path, temp_dir_path, b_calc_sr)
}

/// Old mcz pure process with no extra stuff.  
/// Using temp dirs from pub functions, then after processing, the temp dir will not vanish.
fn process_mcz_core(mcz_path: &Path, temp_dir_path: &Path, b_calc_sr: bool) -> io::Result<(PathBuf, Vec<BeatMapInfo>)> {
    
    let beatmap_data_vec: Arc<Mutex<Vec<BeatMapInfo>>> = Arc::new(Mutex::new(Vec::new()));
    // 在process_mcz_file中添加资源收集
    let required_files: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

    let add_files_to_required = |bg: &Path, audio: &Path| {
        if bg.exists() {
            let mut required_files = required_files.lock().unwrap();
            required_files.insert(bg.to_path_buf());
        }
        if audio.exists() {
            let mut required_files = required_files.lock().unwrap();
            required_files.insert(audio.to_path_buf());
        }
    };

    // 打开 .mcz 文件作为 ZIP 压缩文件
    let file = File::open(mcz_path)?;
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
    
    // 在临时文件夹中找到 .mc 文件并转换为 .osu 文件
    WalkDir::new(temp_dir_path).into_iter().par_bridge()
        .for_each(|entry| {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        
        if entry_path.extension() == Some(std::ffi::OsStr::new("mc")) {
            let (osu_file_path, osu_data) = match process_mc_file_self(entry_path, add_files_to_required) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to convert .mc file {}: {}.", entry_path.to_string_lossy(), e);
                    return;
                }
            };

            let beatmap_data = osu_data.to_beatmap_info(b_calc_sr);
            {
                let mut beatmap_data_vec = beatmap_data_vec.lock().unwrap();
                beatmap_data_vec.push(beatmap_data);
                let mut required_files = required_files.lock().unwrap();
                required_files.insert(osu_file_path);
            }
        }
    });
    
    // 创建新的 .osz ZIP 文件
    let osz_file_path = mcz_path.with_extension("osz");
    println!("Generating .osz at: {:?}", osz_file_path);
    let osz_file = File::create(osz_file_path.clone())?;
    let mut zip_writer = ZipWriter::new(osz_file);
    // 将临时文件夹中的文件添加到 .osz 文件中
    add_files_to_zip(&mut zip_writer, &required_files.lock().unwrap())?;
    // 完成写入
    zip_writer.finish()?;

    Ok((osz_file_path, Arc::try_unwrap(beatmap_data_vec).unwrap().into_inner().unwrap()))
}

/// Completely ignore mcz structre, brutal convert.  
/// Only use it when you can handle the audio and BG related to this .mc file.<br>
/// As osu won't accept non-ascii filenames, you need to do the sanitizing stuff.
pub fn process_mc_file(path: &Path) -> io::Result<PathBuf> {
    let mc_data = analyze_mc_file(&path)?;
    let mut osu_path = PathBuf::from(&path);
    // 获取文件名部分（带后缀）
    if let Some(file_stem) = osu_path.file_stem() {
        // 重新组合路径
        osu_path.set_file_name(format!("{}.osu", file_stem.to_string_lossy()));
    }
    println!("Generating .osu file at: {:?}", osu_path);
    let osu_file = File::create(osu_path.clone())?;
    let mut writer = BufWriter::new(osu_file);

    let osu_data = convert_mc_to_osu(&mc_data)?;
    if let Some(data) = osu_data {
        serialize_osu_data(&mut writer, &data)?;
        Ok(osu_path)
    } else {
        eprint!("Cannot get .mc data.");
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid mc data"))
    }
}

/// The function used in this crate
fn process_mc_file_self<F>(mc_file_path: &Path, callback: F) 
    -> io::Result<(PathBuf, OsuData)>
    where F: Fn(&Path, &Path) -> () {
    // 解析并转换 .mc 文件为 .osu 文件
    let mut mc_data = match analyze_mc_file(&mc_file_path){
        Ok(data) => {
            data
        }
        Err(e) => {
            eprintln!("Error analyzing file {:?}: {}", mc_file_path, e);
            return Err(e);
        }
    };

    // 对 mc_data 中的图片和音频文件名进行替代，并验证文件存在
    let sanitized_background = sanitize_filename(&mc_data.meta.background);
    let sanitized_audio = sanitize_filename(&mc_data.note.last().and_then(|n| n.sound.as_ref()).unwrap_or(&String::new()));
    if let Some(parent_path) = mc_file_path.parent() {
        let background_path = parent_path.join(&sanitized_background);
        let audio_path = parent_path.join(&sanitized_audio);

        if !background_path.exists() || !audio_path.exists() {
            println!("{:?}, {:?}", background_path, audio_path);
            eprintln!("Warning: Some files specified in the mc file are missing.");
        }

        callback(&background_path, &audio_path); // Add them to required_files
    }
    
    mc_data.meta.background = sanitized_background;
    if let Some(note) = mc_data.note.last() {
        if let Some(_sound) = &note.sound {
            let len = mc_data.note.len();
            mc_data.note[len-1].sound = Some(sanitized_audio);
        }
    }
    // 转换 .mc 文件为 .osu 文件
    let mut osu_path = PathBuf::from(&mc_file_path);
    // 获取文件名部分（带后缀）
    if let Some(file_stem) = osu_path.file_stem() {
        // 重新组合路径
        osu_path.set_file_name(format!("{}.osu", file_stem.to_string_lossy()));
    }
    println!("Generating .osu file at: {:?}", osu_path);
    let osu_file = File::create(osu_path).unwrap();
    let mut writer = BufWriter::new(osu_file);

    let osu_data = match convert_mc_to_osu(&mc_data).unwrap() {
        Some(data) => data,
        None => {
            eprintln!("Cannot get .mc data.");
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid mc data"));
        }
    };
    serialize_osu_data(&mut writer, &osu_data).unwrap();
    let osu_file_path = mc_file_path.with_extension("osu");
    Ok((osu_file_path, osu_data))
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
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
        )?;
        io::copy(&mut file, zip_writer)?;
    }
    Ok(())
}

fn convert_mc_to_osu(mc_data: &McData) -> io::Result<Option<OsuData>> {
    // 打印解析后的数据
    // println!("{:#?}", mc_data);

    // 检查模式是否为 0（Key 模式）
    if mc_data.meta.mode != 0 {
        eprintln!("This program only supports Malody Chart in Key Mode!");
        return Ok(None);
    }

    let audio = mc_data.note.last()
        .and_then(|n| n.sound.as_ref())
        .unwrap_or(&String::new())
        .clone();

    let mut osu_data = OsuData{
        misc: OsuMisc { 
            audio_file_name: audio.clone(), 
            preview_time: mc_data.meta.preview.unwrap_or(0), 
            title: mc_data.meta.song.titleorg.clone().unwrap_or(mc_data.meta.song.title.clone()), 
            title_unicode: mc_data.meta.song.title.clone(), 
            artist: mc_data.meta.song.artistorg.clone().unwrap_or(mc_data.meta.song.artist.clone()), 
            artist_unicode: mc_data.meta.song.artist.clone(), 
            creator: mc_data.meta.creator.clone(), 
            version: mc_data.meta.version.clone(), 
            circle_size: mc_data.meta.mode_ext.column as u32,
            od: 8.0, 
            background: mc_data.meta.background.clone(),
        },
        timings: Vec::new(),
        notes: Vec::new(),
    };

    // 构建 TimingPoints 部分
    
    // 这里需要实现 BPM 和效果列表的处理
    // 这里用来写新的实现方法：
    let offset_ms = if let Some(note) = mc_data.note.last() {
        note.offset.unwrap_or(0)
    } else {
        0
    } as f64;
    if osu_data.misc.preview_time > offset_ms as u32{
        osu_data.misc.preview_time -= offset_ms as u32;
    }
    
    // 取第一个BPM为基准BPM
    let bpm_base = mc_data.time.get(0)
        .map(|t| t.bpm as f64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing BPM data"))?;
    let interval_base = 60000_f64 / bpm_base;
    
    let mut bpm_list: Vec<(f64, u32, f64)> = Vec::new(); // 分别记录Malody的拍数,对应的osu内毫秒时刻和间隔时间
    
    for (index, item) in mc_data.time.iter().enumerate() {
        if index == 0 {
            let start_beat = (offset_ms as f64 / interval_base).ceil();
            let start_time = (start_beat * interval_base - offset_ms).floor() as u32;

            bpm_list.push((start_beat, start_time, interval_base));
            continue;
        }
        let current_beat = item.beat_to_float();
        let current_interval = 60000_f64 / item.bpm;
        
        let (old_beat, old_time, old_interval) = bpm_list[index-1];
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
        let idx = bpm_list.partition_point(
            |probe| probe.0 <= beat).saturating_sub(1);
        
        let item = &bpm_list[idx];
        let interv = (item.2 * 10e11).round() / 10e11;
        item.1 + ((beat - item.0) * interv).floor() as u32
    };

    let mut effect_list: Vec<(f64, u32, f64)> = Vec::new(); // 分别记录Malody的拍数,对应的osu内毫秒时刻和osu格式变速

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
            osu_data.timings.push(
                OsuTimingPoint { time: bpm.1 as f64, val: bpm.2, is_timing: true }
            );
            i += 1;
        } else {
            osu_data.timings.push(
                OsuTimingPoint { time: eff.1 as f64, val: eff.2, is_timing: false }
            );
            j += 1;
        }
    }

    // 处理剩余元素
    while i < bpm_list.len() {
        osu_data.timings.push(
            OsuTimingPoint { time: bpm_list[i].1 as f64, val: bpm_list[i].2, is_timing: true }
        );
        i += 1;
    }

    while j < effect_list.len() {
        osu_data.timings.push(
            OsuTimingPoint { time: effect_list[j].1 as f64, val: effect_list[j].2, is_timing: false }
        );
        j += 1;
    }

    // 构建 HitObjects 部分
    let total_column = mc_data.meta.mode_ext.column;
    let column_factor = 512.0 / total_column as f64;

    osu_data.notes = mc_data.note[..mc_data.note.len()-1]
    .par_iter()
    .map(|item| {
        let item_time = beat_to_time(item.beat_to_float());
        let column = item.column.unwrap_or(0);
        let x_pos = ((column as f64 + 0.5) * column_factor as f64).floor() as u32;
        // 处理 item 的 endbeat
        if let Some(_end_beat) = &item.endbeat {
            let item_beat_end = item.end_beat_to_float();
            let item_end_time = beat_to_time(item_beat_end);
            OsuHitObject{x_pos: x_pos, time: item_time, end_time: Some(item_end_time)}
        } else {
            OsuHitObject{x_pos: x_pos, time: item_time, end_time: None}
        }
    })
    .collect();
    
    Ok(Some(osu_data))
}

fn serialize_osu_data(writer: &mut BufWriter<File>, osu_data: &OsuData) -> io::Result<()> {
    // 构建 General 部分
    write!(writer, "osu file format v14\n\n[General]\n")?;
    write!(writer, "AudioFilename: {}\n", osu_data.misc.audio_file_name)?;
    write!(writer, "AudioLeadIn: 0\nPreviewTime: {}\nCountdown: 0\nSampleSet: Soft\n", osu_data.misc.preview_time)?;
    write!(writer, "StackLeniency: 0.7\nMode: 3\nLetterboxInBreaks: 0\nSpecialStyle: 0\nWidescreenStoryboard: 1\n\n")?;

    // 构建 Editor 部分
    write!(writer, "[Editor]\nDistanceSpacing: 1\nBeatDivisor: 8\nGridSize: 4\nTimelineZoom: 2\n\n")?;

    // 构建 Metadata 部分
    write!(writer, "[Metadata]\n")?;
    write!(writer, "Title:{}\n", osu_data.misc.title)?;
    write!(writer, "TitleUnicode:{}\n", osu_data.misc.title_unicode)?;
    write!(writer, "Artist:{}\n", osu_data.misc.artist)?;
    write!(writer, "ArtistUnicode:{}\n", osu_data.misc.artist_unicode)?;
    write!(writer, "Creator:{}\n", osu_data.misc.creator)?;
    write!(writer, "Version:{}\n", osu_data.misc.version)?;
    write!(writer, "Source:\nTags:\nBeatmapID:0\nBeatmapSetID:-1\n\n")?;

    // 构建 Difficulty 部分
    write!(writer, "[Difficulty]\n")?;
    let od_str = if osu_data.misc.od.trunc() == osu_data.misc.od {
        format!("{:.0}", osu_data.misc.od) 
    } else {
        format!("{:.1}", osu_data.misc.od)
    };
    write!(writer, "HPDrainRate:8\nCircleSize:{}\nOverallDifficulty:{}\nApproachRate:5\nSliderMultiplier:1.4\nSliderTickRate:1\n\n", osu_data.misc.circle_size, od_str)?;

    // 构建 Events 部分
    write!(writer, "[Events]\n//Background and Video events\n")?;
    if !osu_data.misc.background.is_empty() {
        write!(writer, "0,0,\"{}\",0,0\n", osu_data.misc.background)?;
    }
    write!(writer, "//Break Periods\n//Storyboard Layer 0 (Background)\n")?;
    write!(writer, "//Storyboard Layer 1 (Fail)\n//Storyboard Layer 2 (Pass)\n")?;
    write!(writer, "//Storyboard Layer 3 (Foreground)\n//Storyboard Layer 4 (Overlay)\n")?;
    write!(writer, "//Storyboard Sound Samples\n\n")?;

    // 构建 TimingPoints 部分
    let timing_points: Vec<_> = osu_data.timings
        .par_iter()
        .map(|tp| format!("{},{},4,2,0,10,{},0",
            tp.time, tp.val, tp.is_timing as u8))
        .collect();

    // 构建 HitObjects 部分
    let hit_objects: Vec<_> = osu_data.notes
        .par_iter()
        .map(|ho| {
            if let Some(t) = ho.end_time {
                format!("{},192,{},128,0,{}:0:0:0:0:",
                    ho.x_pos, ho.time, t)
            } else {
                format!("{},192,{},1,0,0:0:0:0:",
                    ho.x_pos, ho.time)
            }
        })
        .collect();
    
    write!(writer, "[TimingPoints]\n")?;
    writer.write_all(timing_points.join("\n").as_bytes())?;
    write!(writer, "\n\n[HitObjects]\n")?;
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