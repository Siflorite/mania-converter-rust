use rayon::prelude::*;
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

use crate::BeatMapInfo;
use crate::malody::McData;
use crate::misc::sanitize_filename;
use crate::osu::OsuDataV128;
use crate::zip_utils::{chart_formats, convert_mixed_archive, create_archive, extract_archive};

/// Convert all .osz files under the given directory to .mcz files.
/// "." or "" uses the run directory.
pub fn process_whole_dir_osz(dir: &str, b_calc_sr: bool, b_print_results: bool) -> io::Result<()> {
    let current_dir = if dir.is_empty() { "." } else { dir };

    let processed: Vec<_> = WalkDir::new(current_dir)
        .into_iter()
        .par_bridge()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            if path.extension() == Some(std::ffi::OsStr::new("osz")) {
                match process_osz_file(path, b_calc_sr) {
                    Ok(result) => Some(result),
                    Err(error) => {
                        eprintln!("Error processing {}: {}", path.display(), error);
                        None
                    }
                }
            } else {
                None
            }
        })
        .collect();

    if b_print_results {
        println!("\nConversion Summary:");
        println!("{:-<80}", "");

        for (path, infos) in &processed {
            println!("MCZ File: {}", path.display());
            println!("Contains {} beatmaps:", infos.len());

            for info in infos {
                println!("\n{info}");
            }

            println!("{:-<80}\n", "");
        }

        let total_beatmaps: usize = processed.iter().map(|(_, infos)| infos.len()).sum();
        println!("Total processed files: {}", processed.len());
        println!("Total converted beatmaps: {}", total_beatmaps);
    }

    Ok(())
}

/// Convert one .osz file, then call post_process before removing the temporary directory.
///
/// Archives containing .mc charts but no .osu charts are copied byte-for-byte to .mcz.
/// In that case no charts are parsed or converted, and the callback receives empty
/// beatmap information plus the extracted original files in the temporary directory.
/// Mixed archives retain original .mc charts and all assets with their paths;
/// beatmap information describes converted .osu charts only. Conversion errors propagate.
pub fn process_osz_file_postprocess<F>(
    path: &Path,
    b_calc_sr: bool,
    mut post_process: F,
) -> io::Result<PathBuf>
where
    F: FnMut(&[BeatMapInfo], &Path) -> io::Result<()>,
{
    let temp_dir = tempfile::tempdir()?;

    let (mcz_path, mut beatmap_infos) = process_osz_core(path, temp_dir.path(), b_calc_sr)?;

    if b_calc_sr {
        beatmap_infos.sort_by(|a, b| a.sr.partial_cmp(&b.sr).unwrap_or(std::cmp::Ordering::Equal));
    }

    post_process(&beatmap_infos, temp_dir.path())?;
    Ok(mcz_path)
}

/// Convert one .osz file; return the generated .mcz path and beatmap information.
///
/// Archives containing only target-format charts are copied unchanged; the returned
/// beatmap information is empty because no charts were parsed or converted.
/// Mixed archives retain existing target charts and assets unchanged. Returned
/// information describes converted charts only; a failed conversion returns an error.
pub fn process_osz_file(path: &Path, b_calc_sr: bool) -> io::Result<(PathBuf, Vec<BeatMapInfo>)> {
    let mut beatmap_infos = Vec::new();

    let mcz_path = process_osz_file_postprocess(path, b_calc_sr, |infos, _| {
        beatmap_infos = infos.to_vec();
        Ok(())
    })?;

    Ok((mcz_path, beatmap_infos))
}

fn process_osz_core(
    osz_path: &Path,
    temp_dir_path: &Path,
    b_calc_sr: bool,
) -> io::Result<(PathBuf, Vec<BeatMapInfo>)> {
    let (has_osu, has_mc) = chart_formats(osz_path, "osu", "mc")?;
    if !has_osu && has_mc {
        // Keep postprocess access to extracted files, but never repack the output.
        extract_archive(osz_path, temp_dir_path)?;
        let output = osz_path.with_extension("mcz");
        if output != osz_path {
            std::fs::copy(osz_path, &output)?;
        }
        return Ok((output, Vec::new()));
    }
    if has_osu && has_mc {
        return convert_mixed_archive(
            osz_path,
            temp_dir_path,
            "osu",
            "mc",
            "mcz",
            |source, target| {
                let (_, info) = process_osu_file_self(source, target, true, b_calc_sr, |_| {})?;
                Ok(info)
            },
        );
    }

    let beatmap_data_vec: Arc<Mutex<Vec<BeatMapInfo>>> = Arc::new(Mutex::new(Vec::new()));

    let required_files: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

    let add_files_to_required = |path: &Path| {
        if path.is_file() {
            required_files.lock().unwrap().insert(path.to_path_buf());
        }
    };

    extract_archive(osz_path, temp_dir_path)?;

    WalkDir::new(temp_dir_path)
        .into_iter()
        .par_bridge()
        .for_each(|entry| {
            let entry = entry.unwrap();
            let entry_path = entry.path();

            if entry_path.extension() == Some(std::ffi::OsStr::new("osu")) {
                let (mc_file_path, beatmap_info) = match process_osu_file_self(
                    entry_path,
                    &entry_path.with_extension("mc"),
                    false,
                    b_calc_sr,
                    add_files_to_required,
                ) {
                    Ok(result) => result,
                    Err(error) => {
                        eprintln!(
                            "Failed to convert .osu file {}: {}.",
                            entry_path.display(),
                            error
                        );
                        return;
                    }
                };

                beatmap_data_vec.lock().unwrap().push(beatmap_info);
                required_files.lock().unwrap().insert(mc_file_path);
            }
        });

    let mcz_file_path = osz_path.with_extension("mcz");
    println!("Generating .mcz at: {:?}", mcz_file_path);
    create_archive(&mcz_file_path, &required_files.lock().unwrap())?;

    Ok((
        mcz_file_path,
        Arc::try_unwrap(beatmap_data_vec)
            .unwrap()
            .into_inner()
            .unwrap(),
    ))
}

/// Convert one .osu file and report every asset referenced by the resulting .mc file.
fn process_osu_file_self<F>(
    osu_file_path: &Path,
    mc_path: &Path,
    preserve_paths: bool,
    b_calc_sr: bool,
    callback: F,
) -> io::Result<(PathBuf, BeatMapInfo)>
where
    F: Fn(&Path),
{
    let osu_data = OsuDataV128::from_file(&osu_file_path.to_string_lossy())?;

    // to_mc_data() currently indexes the first BPM timing point.
    // Without one it panics, so skip that chart instead.
    if !osu_data.timings.iter().any(|timing| timing.is_timing) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The osu! chart has no BPM timing point",
        ));
    }

    let mut beatmap_info = osu_data.get_beatmap_info(b_calc_sr);
    let mut mc_data: McData = osu_data.to_legacy().to_mc_data();

    let parent_path = osu_file_path.parent().unwrap_or(Path::new("."));

    // Match zip_utils::extract_archive: keep the basename and sanitize it.
    let add_asset = |name: &mut String| {
        if name.is_empty() {
            return;
        }
        if preserve_paths {
            callback(&parent_path.join(&*name));
            return;
        }

        let file_name = Path::new(name)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(name);

        let sanitized = sanitize_filename(file_name);
        let path = parent_path.join(&sanitized);

        if !path.is_file() {
            eprintln!(
                "Warning: Resource file {} missing in {}.",
                path.display(),
                osu_file_path.display()
            );
        }

        callback(&path);
        *name = sanitized;
    };

    add_asset(&mut mc_data.meta.background);

    // Includes the type=1 main audio, type=1 storyboard samples,
    // and custom sounds on regular notes.
    for note in &mut mc_data.note {
        if let Some(sound) = &mut note.sound {
            add_asset(sound);
        }
    }

    beatmap_info.bg_name = mc_data.meta.background.clone();

    println!("Generating .mc file at: {:?}", mc_path);
    mc_data.to_file(&mc_path.to_string_lossy())?;

    Ok((mc_path.to_path_buf(), beatmap_info))
}
