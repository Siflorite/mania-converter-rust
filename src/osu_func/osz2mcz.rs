use crate::malody_func::{McData, Meta, Song, ModeExt, Beat, Effect};
use crate::osu_func::{OsuDataLegacy};

fn convert_osu_to_mc(osu_data: &OsuDataLegacy) -> McData {
    let mc_data_meta = Meta {
        creator: osu_data.misc.creator.clone(),
        background: osu_data.misc.background.clone(),
        version: osu_data.misc.version.clone(),
        preview: Some(osu_data.misc.preview_time),
        mode: 0,
        mode_ext: ModeExt { column: osu_data.misc.circle_size as u8 },
        song: Song {
            title: osu_data.misc.title_unicode.clone(),
            artist: osu_data.misc.artist_unicode.clone(),
            titleorg: Some(osu_data.misc.title.clone()),
            artistorg: Some(osu_data.misc.artist.clone()),
        }
    };
    let mut mc_data = McData {
        meta: mc_data_meta,
        time: vec![],
        effect: Some(vec![]),
        note: vec![],
    };
    mc_data
}

fn calc_grid(timing_point: f64, interval: f64, note_time: u32) -> (u32, u8, u8) {
    let delta_time = note_time as f64 - timing_point;
    let total_beats = delta_time / interval;
    let beat: u32 = (delta_time / interval).floor() as u32;
    let fraction = total_beats.fract();
    let (numerator, denominator) = (1..=16)
        .flat_map(|d| (0..=d).map(move |n| (n, d)))
        .min_by(|&a, &b| {
            let residual_a = (fraction - a.0 as f64 / a.1 as f64).abs();
            let residual_b = (fraction - b.0 as f64 / b.1 as f64).abs();
            residual_a.partial_cmp(&residual_b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0, 1));
    (beat, numerator, denominator)
}