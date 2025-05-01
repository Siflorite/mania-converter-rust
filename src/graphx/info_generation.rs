use::handlebars::Handlebars;
use resvg::{usvg, tiny_skia};
use serde_json::json;
use std::{env, fs, io, path::{Path, PathBuf}, sync::Arc};

use crate::BeatMapInfo;

const INFO_TEMPLATE_PATH: &str = "./svg/info_card.svg";
const NO_IMAGE_PATH: &str = "./svg/no_image.jpg";
const FONT_DIR_PATH: &str = "./font";
const CARD_HEIGHT: f64 = 300.0;

#[derive(serde::Serialize)]
struct CardData {
    bg_image: String,
    title: String,
    artist: String,
    creator: String,
    version: String,
    column_count: u8,
    bpm: String,
    length: String,
    sr_gradient: String,
    sr: String,
    note_str: String,
    ln_str: String,
    len_pos: u32,
    y_offset: f64,
}

pub fn generate_info_abstract(info_vec: &[BeatMapInfo], temp_dir_path: &Path, save_pic_path: &Path) -> io::Result<PathBuf> {
    let mut reg = Handlebars::new();
    reg.register_template_file("template", INFO_TEMPLATE_PATH)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let card_vec: Vec<CardData> = info_vec.iter().enumerate().map(|(i, info)| {
        let bg_name = match &info.bg_name {
            Some(s) => s.as_str(),
            None => ""
        };
        let bg_path = temp_dir_path.join(Path::new(bg_name));
        let default_path = env::current_dir().unwrap().join(Path::new(NO_IMAGE_PATH));
        let final_path = if bg_path.exists() { bg_path } else { default_path };
        let bg_path_string = final_path.to_string_lossy().into_owned();

        let title = info.title_unicode.as_ref().unwrap_or(&info.title);
        let artist = info.artist_unicode.as_ref().unwrap_or(&info.artist);
        let bpm_str = format_bpm_str(info.min_bpm, info.max_bpm);
        let delta_len = bpm_str.len() as u32 * 12;
        let length_str = format_length_str(info.length);
        let sr = info.sr.unwrap_or(0.0);

        let total_count = info.note_count + info.ln_count;
        let note_str = format!("{} ({:.02}%)", info.note_count, info.note_count as f64 / total_count as f64 * 100.0);
        let ln_str = format!("{} ({:.02}%) = {}", info.ln_count, info.ln_count as f64 / total_count as f64 * 100.0, total_count);

        CardData {
            bg_image: bg_path_string,
            title: title.into(),
            artist: artist.into(),
            creator: info.creator.clone(),
            version: info.version.clone(),
            column_count: info.column_count,
            bpm: bpm_str,
            length: length_str,
            sr_gradient: format_sr_gredient(sr),
            sr: format!("{:.02}", sr),
            note_str: note_str,
            ln_str: ln_str,
            len_pos: 190 + delta_len,
            y_offset: i as f64 * CARD_HEIGHT
        }
    }).collect();

    // 渲染SVG
    let total_height = card_vec.len() as f64 * CARD_HEIGHT;
    let svg_content = reg.render("template", &json!({
        "total_height": total_height,
        "cards": card_vec
    })).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut fontdb_origin = usvg::fontdb::Database::new();
    fontdb_origin.load_fonts_dir(FONT_DIR_PATH);

    // 渲染选项
    let options = usvg::Options {
        fontdb: Arc::new(fontdb_origin),
        resources_dir: Some(temp_dir_path.to_path_buf()),
        ..Default::default()
    };

    // 解析并渲染SVG
    let tree = usvg::Tree::from_str(&svg_content, &options)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut pixmap = tiny_skia::Pixmap::new(1200, total_height as u32)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to create pixmap"))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // 确保输出目录存在
    if let Some(parent) = save_pic_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // 保存为PNG
    let pic_name = format!("{}.png", info_vec[0].title);
    let pic_path = save_pic_path.join(pic_name);
    pixmap.save_png(&pic_path)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(pic_path)
}

fn format_bpm_str(min_bpm: f64, max_bpm: Option<f64>) -> String {
    let m_bpm = match max_bpm {
        Some(v) => v,
        None => min_bpm
    };
    let min_bpm_str = format!("{:.1}", min_bpm).trim_matches('0').trim_matches('.').to_string();
    
    if (m_bpm * 10.0).round() as i32 == (min_bpm * 10.0).round() as i32 {
        format!("{}", min_bpm_str)
    } else {
        let max_bpm_str = format!("{:.1}", m_bpm).trim_matches('0').trim_matches('.').to_string();
        format!("{}-{}", min_bpm_str, max_bpm_str)
    }
}

fn format_length_str(length: u32) -> String {
    let mins = length / 60000;
    let secs = (length - 60000 * mins) / 1000;
    let msecs = length % 1000;
    format!("{}:{:02}.{:03}", mins, secs, msecs)
}

fn format_sr_gredient(sr: f64) -> String {
    let colors = [
        (79.0, 192.0, 255.0), (124.0, 255.0, 79.0), (246.0, 240.0, 92.0), 
        (255.0, 78.0, 111.0), (198.0, 69.0, 184.0), (101.0, 99.0, 222.0), (0.0, 0.0, 0.0), (0.0, 0.0, 0.0)];
    let sr = sr.clamp(0.0, 10.0);
    let interval = 10.0 / (colors.len() - 2) as f64;
    let section = (sr / interval) as usize;
    let partial = (sr - interval * section as f64) / 2.0;
    let r = colors[section].0 + (colors[section + 1].0 - colors[section].0) * partial;
    let g = colors[section].1 + (colors[section + 1].1 - colors[section].1) * partial;
    let b = colors[section].2 + (colors[section + 1].2 - colors[section].2) * partial;
    format!("rgb({},{},{})", r.round() as u8, g.round() as u8, b.round() as u8)
}