use rayon::prelude::*;
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::str;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

use crate::BeatMapInfo;
use crate::malody::McData;
use crate::misc::sanitize_filename;
use crate::osu::OsuDataLegacy;
use crate::zip_utils::{chart_formats, convert_mixed_archive, create_archive, extract_archive};

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
///
/// Archives containing .osu charts but no .mc charts are copied byte-for-byte to .osz.
/// In that case no charts are parsed or converted, and the callback receives empty
/// beatmap information plus the extracted original files in the temporary directory.
/// Mixed archives retain original .osu charts and all assets with their paths;
/// beatmap information describes converted .mc charts only. Conversion errors propagate.
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
///
/// Archives containing only target-format charts are copied unchanged; the returned
/// beatmap information is empty because no charts were parsed or converted.
/// Mixed archives retain existing target charts and assets unchanged. Returned
/// information describes converted charts only; a failed conversion returns an error.
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
    let (has_mc, has_osu) = chart_formats(mcz_path, "mc", "osu")?;
    if !has_mc && has_osu {
        // Keep postprocess access to extracted files, but never repack the output.
        extract_archive(mcz_path, temp_dir_path)?;
        let output = mcz_path.with_extension("osz");
        if output != mcz_path {
            std::fs::copy(mcz_path, &output)?;
        }
        return Ok((output, Vec::new()));
    }
    if has_mc && has_osu {
        return convert_mixed_archive(
            mcz_path,
            temp_dir_path,
            "mc",
            "osu",
            "osz",
            |source, target| {
                let (_, data) = process_mc_file_self(source, target, true, |_| {})?;
                Ok(data.get_beatmap_info(b_calc_sr))
            },
        );
    }

    let beatmap_data_vec: Arc<Mutex<Vec<BeatMapInfo>>> = Arc::new(Mutex::new(Vec::new()));

    // Collect assets required by all .mc files
    let required_files: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    let add_files_to_required = |path: &Path| {
        if path.is_file() {
            required_files.lock().unwrap().insert(path.to_path_buf());
        }
    };

    extract_archive(mcz_path, temp_dir_path)?;

    // 在临时文件夹中找到 .mc 文件并转换为 .osu 文件
    WalkDir::new(temp_dir_path)
        .into_iter()
        .par_bridge()
        .for_each(|entry| {
            let entry = entry.unwrap();
            let entry_path = entry.path();

            if entry_path.extension() == Some(std::ffi::OsStr::new("mc")) {
                let (osu_file_path, osu_data) = match process_mc_file_self(
                    entry_path,
                    &entry_path.with_extension("osu"),
                    false,
                    add_files_to_required,
                ) {
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
                    beatmap_data_vec.lock().unwrap().push(beatmap_data);
                    required_files.lock().unwrap().insert(osu_file_path);
                }
            }
        });

    // 创建新的 .osz ZIP 文件
    let osz_file_path = mcz_path.with_extension("osz");
    println!("Generating .osz at: {:?}", osz_file_path);
    create_archive(&osz_file_path, &required_files.lock().unwrap())?;

    Ok((
        osz_file_path,
        Arc::try_unwrap(beatmap_data_vec)
            .unwrap()
            .into_inner()
            .unwrap(),
    ))
}

/// Process a single .mc file, convert it into osu! format,
/// and return the .osu file path and data.  
///
/// The function uses callback to post all required assets in .mc file,
/// so that the caller can collect them and add them to the .osz file.
fn process_mc_file_self<F>(
    mc_file_path: &Path,
    osu_path: &Path,
    preserve_paths: bool,
    callback: F,
) -> io::Result<(PathBuf, OsuDataLegacy)>
where
    F: Fn(&Path),
{
    // 解析并转换 .mc 文件为 .osu 文件
    let mc_data = McData::from_file(&mc_file_path.to_string_lossy())?;
    let mut osu_data = mc_data.to_osu_data()?;

    // sanitize filenames
    let parent_path = mc_file_path.parent().unwrap_or(Path::new("."));
    let sanitize_reference = |name: &str| {
        if preserve_paths {
            return name.to_string();
        }
        let file_name = Path::new(name)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(name);
        sanitize_filename(file_name)
    };

    let background = sanitize_reference(&mc_data.meta.background);
    let audio = sanitize_reference(&osu_data.misc.audio_file_name);

    let background_path = parent_path.join(&background);
    let audio_path = parent_path.join(&audio);
    if !background_path.is_file() || !audio_path.is_file() {
        println!("{:?}, {:?}", background_path, audio_path);
        eprintln!("Warning: Some files specified in the mc file are missing.");
    }
    // Add them to required_files
    callback(&background_path);
    callback(&audio_path);

    osu_data.misc.background = background;
    osu_data.misc.audio_file_name = audio;

    // 把hitsounds打包进去
    let add_hitsound = |name: &mut String| {
        if name.is_empty() {
            return;
        }

        let sanitized = sanitize_reference(name);
        let path = parent_path.join(&sanitized);

        if !path.is_file() {
            eprintln!(
                "Warning: Hitsound file {} missing in {}.",
                path.display(),
                mc_file_path.display()
            );
        }

        callback(&path);
        *name = sanitized;
    };

    osu_data
        .storyboard_samples
        .iter_mut()
        .for_each(|s| add_hitsound(&mut s.hitsound));

    osu_data
        .notes
        .iter_mut()
        .filter_map(|n| n.hitsound.as_mut())
        .for_each(add_hitsound);

    // 转换 .mc 文件为 .osu 文件
    println!("Generating .osu file at: {:?}", osu_path);
    osu_data.to_file(&osu_path.to_string_lossy())?;

    Ok((osu_path.to_path_buf(), osu_data))
}
