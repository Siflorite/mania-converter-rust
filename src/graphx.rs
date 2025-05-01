mod info_generation;

use std::io;
use std::path::{Path, PathBuf};

pub use self::info_generation::generate_info_abstract;
use crate::osu_func::parse_osz_postprocess;

pub fn generate_osz_info(osz_path: &Path) -> io::Result<PathBuf> {
    let save_pic_path = osz_path.parent().unwrap();
    let mut pic_path = PathBuf::new();
    parse_osz_postprocess(
        osz_path, 
        |info_vec, temp_dir_path| {
            pic_path = generate_info_abstract(info_vec, temp_dir_path, save_pic_path)?;
            Ok(())
        }
    )?;
    Ok(pic_path)
}