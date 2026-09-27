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
use crate::osu::{OsuDataLegacy, OsuDataV128};
use crate::zip_utils::{create_archive, extract_archive};

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

    // Collect assets required by all .mc files
    let required_files: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    let add_files_to_required = |path: &Path| {
        if path.is_file() {
            required_files.lock().unwrap().insert(path.to_path_buf());
        }
    };

    extract_archive(mcz_path, temp_dir_path)?;

    let entries = WalkDir::new(temp_dir_path)
        .into_iter()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    entries.par_iter().for_each(|entry| {
        let entry_path = entry.path();
        if !entry_path.is_file() {
            return; // Return this closure
        }

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
                beatmap_data_vec.lock().unwrap().push(beatmap_data);
                required_files.lock().unwrap().insert(osu_file_path);
            }
        } else if entry_path.extension() == Some(std::ffi::OsStr::new("osu")) {
            // If a player imports a mcz pack into Malody, and exports it from Malody,
            // Malody will pack up with extension name ".mcz"
            // So we need to handle osu files in mczs as well.
            if let Ok(osu_data) = OsuDataV128::from_file(&entry_path.to_string_lossy()) {
                let mut osu_data = osu_data.to_legacy();
                // Pack up assets
                let parent = entry_path.parent().unwrap_or(Path::new("."));
                let add_asset = |name: &mut String| {
                    if name.is_empty() {
                        return;
                    }

                    let file_name = Path::new(name.as_str())
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(name.as_str());
                    let sanitized_name = sanitize_filename(file_name);
                    add_files_to_required(&parent.join(&sanitized_name));
                    *name = sanitized_name;
                };

                add_asset(&mut osu_data.misc.audio_file_name);
                add_asset(&mut osu_data.misc.background);

                for sample in &mut osu_data.storyboard_samples {
                    add_asset(&mut sample.hitsound);
                }

                for hitsound in osu_data
                    .notes
                    .iter_mut()
                    .filter_map(|n| n.hitsound.as_mut())
                {
                    add_asset(hitsound);
                }

                // 写回临时目录中的 osu，保存修改后的资源引用。
                if let Err(e) = osu_data.to_file(&entry_path.to_string_lossy()) {
                    eprintln!("Failed to write {}: {e}", entry_path.display());
                    return;
                }

                add_files_to_required(entry_path);

                let beatmap_data = osu_data.get_beatmap_info(b_calc_sr);
                beatmap_data_vec.lock().unwrap().push(beatmap_data);
            } else {
                eprintln!("Failed to read .osu file {}", entry_path.to_string_lossy());
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
fn process_mc_file_self<F>(mc_file_path: &Path, callback: F) -> io::Result<(PathBuf, OsuDataLegacy)>
where
    F: Fn(&Path),
{
    // 解析并转换 .mc 文件为 .osu 文件
    let mc_data = McData::from_file(&mc_file_path.to_string_lossy())?;
    let mut osu_data = mc_data.to_osu_data()?;

    // sanitize filenames
    let parent_path = mc_file_path.parent().unwrap_or(Path::new("."));
    let sanitize_reference = |name: &str| {
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
    let osu_path = mc_file_path.with_extension("osu");
    println!("Generating .osu file at: {:?}", osu_path);
    osu_data.to_file(&osu_path.to_string_lossy())?;

    Ok((osu_path, osu_data))
}
