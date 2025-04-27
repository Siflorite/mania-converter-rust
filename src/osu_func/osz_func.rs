use crate::BeatMapInfo;
use crate::osu_func::OsuData;

use std::fs::File;
use std::io;
use std::path::Path;
use std::str;
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use walkdir::WalkDir;
use zip::ZipArchive;


pub fn parse_osz_file(path: &Path, b_calc_sr: bool) -> io::Result<Vec<BeatMapInfo>> {
    let temp_dir = tempdir::TempDir::new("parse_osz")?;
    let temp_dir_path = temp_dir.path();
    let beatmap_data_vec: Arc<Mutex<Vec<BeatMapInfo>>> = Arc::new(Mutex::new(Vec::new()));

    let file = File::open(path)?;
    let mut zip_archive = ZipArchive::new(file)?;

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

        let target_path = temp_dir_path.join(pure_file_name);

        // 将文件解压到临时目录中
        if file.is_file() {
            let mut output = File::create(&target_path)?;
            io::copy(&mut file, &mut output)?;
        }
    }

    WalkDir::new(temp_dir_path).into_iter().par_bridge()
        .for_each(|entry| {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        if entry_path.extension() == Some(std::ffi::OsStr::new("osu")) {
            let osu_path_str = match entry_path.to_str() {
                Some(s) => s,
                None => { return; }
            };

            let osu_data = match OsuData::from_file(osu_path_str) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Cannot get osu data: {e}");
                    return;
                }
            };
            let beatmap_data = osu_data.to_beatmap_info(b_calc_sr);
            {
                let mut beatmap_data_vec = beatmap_data_vec.lock().unwrap();
                beatmap_data_vec.push(beatmap_data);
            }
        }
    
    });

    Ok(Arc::try_unwrap(beatmap_data_vec).unwrap().into_inner().unwrap())
}