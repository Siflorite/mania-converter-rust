use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::str;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::BeatMapInfo;
use crate::malody::McData;
use crate::misc::sanitize_filename;
use crate::osu::OsuDataLegacy;

/// Convert all .mcz files under given dir to .osz files.  
/// "." or "" will set dir to the Run Directory.
pub fn process_whole_dir_mcz(dir: &str, b_calc_sr: bool, b_print_results: bool) -> io::Result<()> {
    let current_dir = if dir.is_empty() { "." } else { dir }; // 当前目录
    // let results_queue = Arc::new(SegQueue::<(PathBuf, Vec<BeatMapInfo>)>::new());

    // 遍历当前目录下的所有文件
    let processed: Vec<_> = WalkDir::new(current_dir)
        .into_iter()
        .par_bridge()
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
            } else {
                None
            }
        })
        .collect();

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
    b_calc_sr: bool,
    mut post_process: F,
) -> io::Result<PathBuf>
where
    F: FnMut(&[BeatMapInfo], &Path) -> io::Result<()>,
{
    let temp_dir = tempfile::tempdir()?;

    // 使用原有核心处理逻辑，默认计算难度
    let (osz_path, mut beatmap_infos) = process_mcz_core(path, temp_dir.path(), b_calc_sr)?;
    if b_calc_sr {
        beatmap_infos.sort_by(|x, y| x.sr.partial_cmp(&y.sr).unwrap_or(std::cmp::Ordering::Equal));
    }
    // 执行后处理闭包
    post_process(&beatmap_infos, temp_dir.path())?;
    Ok(osz_path)
}

/// 将mcz文件转换为osz文件<br>
/// 输入参数：mcz文件路径，是否计算星级<br>
/// 输出结果：osz文件路径，内部谱面信息
pub fn process_mcz_file(path: &Path, b_calc_sr: bool) -> io::Result<(PathBuf, Vec<BeatMapInfo>)> {
    let mut beatmap_infos = Vec::new();
    let osz_path = process_mcz_file_postprocess(path, b_calc_sr, |infos, _| {
        beatmap_infos = infos.to_vec();
        Ok(())
    })?;
    Ok((osz_path, beatmap_infos))
}

/// Old mcz pure process with no extra stuff.  
/// Using temp dirs from pub functions, then after processing, the temp dir will not vanish.
fn process_mcz_core(
    mcz_path: &Path,
    temp_dir_path: &Path,
    b_calc_sr: bool,
) -> io::Result<(PathBuf, Vec<BeatMapInfo>)> {
    let beatmap_data_vec: Arc<Mutex<Vec<BeatMapInfo>>> = Arc::new(Mutex::new(Vec::new()));
    // 在process_mcz_file中添加资源收集
    let required_files: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

    let add_files_to_required = |bg: &Path, audio: &Path| {
        // println!("{:?}, {:?}", bg, audio);
        if bg.is_file() {
            let mut required_files = required_files.lock().unwrap();
            required_files.insert(bg.to_path_buf());
        }
        if audio.is_file() {
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
        let translated_file_name = match str::from_utf8(file_name_bytes) {
            Ok(file_name) => file_name.to_string(),
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
        let target_path = temp_dir_path.join(sanitized);

        // 将文件解压到临时目录中
        if file.is_file() {
            let mut output = File::create(&target_path)?;
            io::copy(&mut file, &mut output)?;
        }
    }

    // 在临时文件夹中找到 .mc 文件并转换为 .osu 文件
    WalkDir::new(temp_dir_path)
        .into_iter()
        .par_bridge()
        .for_each(|entry| {
            let entry = entry.unwrap();
            let entry_path = entry.path();

            if entry_path.extension() == Some(std::ffi::OsStr::new("mc")) {
                let (osu_file_path, osu_data) =
                    match process_mc_file_self(entry_path, add_files_to_required) {
                        Ok(data) => data,
                        Err(e) => {
                            eprintln!(
                                "Failed to convert .mc file {}: {}.",
                                entry_path.to_string_lossy(),
                                e
                            );
                            return;
                        }
                    };

                let beatmap_data = osu_data.get_beatmap_info(b_calc_sr);
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

    Ok((
        osz_file_path,
        Arc::try_unwrap(beatmap_data_vec)
            .unwrap()
            .into_inner()
            .unwrap(),
    ))
}

/// The function used in this crate
fn process_mc_file_self<F>(mc_file_path: &Path, callback: F) -> io::Result<(PathBuf, OsuDataLegacy)>
where
    F: Fn(&Path, &Path),
{
    // 解析并转换 .mc 文件为 .osu 文件
    let mc_data = McData::from_file(&mc_file_path.to_string_lossy())?;
    let mut osu_data = mc_data.to_osu_data()?;

    // 对 mc_data 中的图片和音频文件名进行替代，并验证文件存在
    // 音频不再默认取最后一个，从转换的osu_data里面提取
    let parent_path = mc_file_path.parent().unwrap_or(Path::new("."));
    let sanitized_background = sanitize_filename(&mc_data.meta.background);
    let sanitized_audio = sanitize_filename(&osu_data.misc.audio_file_name);

    let background_path = parent_path.join(&sanitized_background);
    let audio_path = parent_path.join(&sanitized_audio);
    if !background_path.exists() || !audio_path.exists() {
        println!("{:?}, {:?}", background_path, audio_path);
        eprintln!("Warning: Some files specified in the mc file are missing.");
    }
    callback(&background_path, &audio_path); // Add them to required_files

    osu_data.misc.background = sanitized_background;
    osu_data.misc.audio_file_name = sanitized_audio;

    // TODO: 把hitsounds打包进去

    // 转换 .mc 文件为 .osu 文件
    let osu_path = mc_file_path.with_extension("osu");
    println!("Generating .osu file at: {:?}", osu_path);
    osu_data.to_file(&osu_path.to_string_lossy())?;

    Ok((osu_path, osu_data))
}

fn add_files_to_zip(zip_writer: &mut ZipWriter<File>, files: &HashSet<PathBuf>) -> io::Result<()> {
    let sorted_files: Vec<_> = files.iter().collect();

    for path in sorted_files {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name"))?;

        let mut file = File::open(path)?;
        zip_writer.start_file(
            file_name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )?;
        io::copy(&mut file, zip_writer)?;
    }
    Ok(())
}
