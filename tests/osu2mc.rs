use std::fs::File;
use std::io::BufWriter;

use anyhow::Result;
use mania_converter::malody::{Beat, McData, Note};
use mania_converter::osu::{
    OsuDataLegacy, OsuDataV128, OsuHitObjectLegacy, OsuTimingPoint, calculate_from_data,
};

#[test]
fn osu_to_grid_nosv() -> Result<()> {
    let file = "./tests/beatmaps/1705852671.mc";
    let mc_data = McData::from_file(file)?;
    let osu_data = mc_data.to_osu_data()?;
    let column_num = osu_data.misc.circle_size;

    let original_timings = osu_data
        .timings
        .iter()
        .filter(|t| t.is_timing)
        .collect::<Vec<_>>();
    let original_offset = original_timings.first().unwrap().time;
    let original_interval = original_timings.first().unwrap().val;
    let offset_beats = if original_offset > 0.0 {
        (original_offset / original_interval).ceil()
    } else {
        0.0
    };
    let offset = offset_beats * original_interval;
    let timings = original_timings
        .iter()
        .map(|t| OsuTimingPoint {
            time: t.time - offset,
            val: t.val,
            is_timing: t.is_timing,
        })
        .collect::<Vec<_>>();

    let beat_counts = timings
        .windows(2)
        .map(|t| {
            let start_time = t[0].time;
            let interval = t[0].val;
            let end_time = t[1].time;
            (end_time - start_time) / interval
        })
        .collect::<Vec<_>>();
    let mut beat_counts_fold = beat_counts
        .iter()
        .scan(0.0, |sum, &x| {
            *sum += x;
            Some(*sum)
        })
        .collect::<Vec<_>>();
    beat_counts_fold.insert(0, 0.0);
    let notes = osu_data.notes;

    let to_grid = |note: &OsuHitObjectLegacy| -> Note {
        let start_time = note.time;
        let start_timing_index = timings
            .partition_point(|t| t.time < start_time as f64)
            .saturating_sub(1);
        let start_timing = &timings[start_timing_index];
        let start_beats = beat_counts_fold[start_timing_index];
        let start_grid = calc_grid(start_timing.time, start_timing.val, start_time, start_beats);

        let end_grid = if let Some(end_time) = note.end_time {
            let end_timing_index = timings
                .partition_point(|t| t.time < end_time as f64)
                .saturating_sub(1);
            let end_timing = &timings[end_timing_index];
            let end_beats = beat_counts_fold[end_timing_index];
            let end_grid = calc_grid(end_timing.time, end_timing.val, end_time, end_beats);
            Some(end_grid)
        } else {
            None
        };

        let beat = Vec::from([start_grid.0, start_grid.1, start_grid.2]);
        let endbeat = end_grid.map(|e| Vec::from([e.0, e.1, e.2]));
        let column = note.x_pos * column_num / 512;

        Note {
            beat,
            endbeat,
            column: Some(column as u8),
            sound: None,
            vol: None,
            offset: None,
            r#type: None,
        }
    };

    let new_notes = notes.iter().map(to_grid).collect::<Vec<_>>();

    for (note, new_note) in mc_data.note.iter().zip(new_notes.iter()) {
        if note.beat_to_float() != new_note.beat_to_float()
            || note.end_beat_to_float() != new_note.end_beat_to_float()
        {
            println!("{:?} -> {:?}", note, new_note);
        }
    }

    println!("{:?}", timings);
    Ok(())
}

#[test]
fn osu_to_grid_sv() -> Result<()> {
    let file = "./tests/beatmaps/hesitation.mc";
    let mc_data = McData::from_file(file)?;
    let osu_data = mc_data.to_osu_data()?;
    let column_num = osu_data.misc.circle_size;

    let original_timings = osu_data
        .timings
        .iter()
        .filter(|t| t.is_timing)
        .collect::<Vec<_>>();
    let original_offset = original_timings.first().unwrap().time;
    let original_interval = original_timings.first().unwrap().val;
    let offset_beats = if original_offset > 0.0 {
        (original_offset / original_interval).ceil()
    } else {
        0.0
    };
    let offset = offset_beats * original_interval;
    let timings = original_timings
        .iter()
        .enumerate()
        .map(|(i, t)| OsuTimingPoint {
            time: if i == 0 {
                t.time - offset.floor()
            } else {
                t.time
            },
            val: t.val,
            is_timing: t.is_timing,
        })
        .collect::<Vec<_>>();
    println!("{:?}", timings);
    println!("");

    let beat_counts = timings
        .windows(2)
        .map(|t| {
            let start_time = t[0].time;
            let interval = t[0].val;
            let end_time = t[1].time;
            let val = (end_time - start_time) / interval;
            println!("{}", val);
            Beat::from_float(val)
        })
        .collect::<Vec<_>>();

    let mut beat_counts_fold = beat_counts
        .iter()
        .scan(Beat::default(), |sum, &x| {
            *sum += x;
            Some(*sum)
        })
        .collect::<Vec<_>>();
    beat_counts_fold.insert(0, Beat::default());
    println!("{:?}", beat_counts_fold);

    let notes = osu_data.notes;

    let time_to_beat = |time: f64| -> Beat {
        let timing_index = timings.partition_point(|t| t.time < time).saturating_sub(1);
        let timing = &timings[timing_index];
        let beats = beat_counts_fold[timing_index];
        beats + Beat::from_float((time - timing.time) / timing.val)
    };

    let to_grid = |note: &OsuHitObjectLegacy| -> Note {
        let start_time = note.time;
        let start_grid = time_to_beat(start_time as f64);

        let end_grid = if let Some(end_time) = note.end_time {
            let end_grid = time_to_beat(end_time as f64);
            Some(end_grid)
        } else {
            None
        };

        let column = note.x_pos * column_num / 512;

        Note {
            beat: start_grid.to_vec(),
            endbeat: end_grid.map(|e| e.to_vec()),
            column: Some(column as u8),
            sound: None,
            vol: None,
            offset: None,
            r#type: None,
        }
    };

    let new_notes = notes
        .iter()
        .enumerate()
        .map(|(_i, n)| to_grid(n))
        .collect::<Vec<_>>();

    for i in 900..notes.len() - 1 {
        let _ = to_grid(&notes[i]);
    }

    for (note, new_note) in mc_data.note.iter().zip(new_notes.iter()) {
        if note.beat_to_float() != new_note.beat_to_float()
            || note.end_beat_to_float() != new_note.end_beat_to_float()
        {
            eprintln!("{:?} -> {:?}", note, new_note);
            panic!();
        }
    }
    // println!("");
    // println!("Original: {:?}\n", &mc_data.note[mc_data.note.len() - 11..mc_data.note.len() - 1]);
    // println!("Osu: {:?}\n", &notes[notes.len() - 10..]);
    // println!("Transferred: {:?}\n", &new_notes[new_notes.len() - 10..]);

    // println!("Timings: {:?}", timings);
    println!("{}", new_notes.len());
    Ok(())
}

fn calc_grid(
    timing_point: f64,
    interval: f64,
    note_time: u32,
    append_beats: f64,
) -> (u32, u32, u32) {
    const MAXIMUM_DIVISION: u32 = 16;
    const MAXIMUM_RESIDUAL: f64 = 0.5 / MAXIMUM_DIVISION as f64;

    let delta_time = note_time as f64 - timing_point;
    let total_beats = delta_time / interval + append_beats;
    let beat: u32 = total_beats.floor() as u32 + 1;
    let fraction = total_beats.fract();
    if (1.0 - fraction) < MAXIMUM_RESIDUAL {
        return (beat + 1, 0, 1);
    }

    let (numerator, denominator) = (1..=16)
        .flat_map(|d| (0..d).map(move |n| (n, d)))
        .min_by(|&a, &b| {
            let residual_a = (fraction - a.0 as f64 / a.1 as f64).abs();
            let residual_b = (fraction - b.0 as f64 / b.1 as f64).abs();
            residual_a
                .partial_cmp(&residual_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0, 1));
    (beat, numerator, denominator)
}
#[test]
fn test_osu_legacy_to_mc() -> std::io::Result<()> {
    let file = "./tests/beatmaps/hesitation.mc";
    let mc_data = McData::from_file(file)?;
    let osu_data = mc_data.to_osu_data()?;
    let new_mc_data = osu_data.to_mc_data();
    println!("{:?}", new_mc_data);
    Ok(())
}

#[test]
fn parse_osu_file_with_hitsound() {
    let osu_hs_file = "./tests/beatmaps/hitsound_test/Never-Ending Performance/HOYO-MiX - Never-Ending Performance (_IceRain) [[14]].osu";
    let reconverted_mc = "./test_stuff/hs/regular.mc";
    let reconverted_osu = "./test_stuff/hs/regular.osu";

    let osu_data = OsuDataV128::from_file(osu_hs_file).unwrap();
    println!("{:?}", osu_data);
    let osu_data_legacy = OsuDataLegacy::from(osu_data);
    println!("{:?}", osu_data_legacy);

    let mc_data = osu_data_legacy.to_mc_data();
    let mc_file = File::create(reconverted_mc).unwrap();
    let writer = BufWriter::new(mc_file);
    serde_json::to_writer(writer, &mc_data).unwrap();

    let _reconverted = mc_data.to_osu_data().unwrap().to_file(reconverted_osu);
}

#[test]
fn parse_piano() {
    let osu_hs_file = "./tests/beatmaps/hitsound_test/2336416_Jerico_-_Soaring/Jerico - Soaring (_IceRain) [Nostalgia].osu";
    let reconverted_mc = "./test_stuff/hs/wtf.mc";
    let reconverted_osu = "./test_stuff/hs/wtf.osu";

    let osu_data = OsuDataV128::from_file(osu_hs_file).unwrap();
    let osu_data_legacy = OsuDataLegacy::from(osu_data);
    println!("{:?}", osu_data_legacy.storyboard_samples);

    let mc_data = osu_data_legacy.to_mc_data();
    let mc_file = File::create(reconverted_mc).unwrap();
    let writer = BufWriter::new(mc_file);
    serde_json::to_writer(writer, &mc_data).unwrap();

    let _reconverted = mc_data.to_osu_data().unwrap().to_file(reconverted_osu);
}

#[test]
fn double_convert() {
    let malody_hs_file = "./tests/beatmaps/hitsound_test/1116 DJ TOTTO VS TOTTO - Vajra/106641 Biemote - 6K SPECIAL(Remake) Lv.31.mc";
    let converted_osu = "./test_stuff/hs/mc_converted.osu";
    let converted_mc = "./test_stuff/hs/mc_converted.mc";

    let mc_data = McData::from_file(malody_hs_file).unwrap();
    let osu_data = mc_data.to_osu_data().unwrap();
    osu_data.to_file(converted_osu).unwrap();

    let mc_file = File::create(converted_mc).unwrap();
    let writer = BufWriter::new(mc_file);
    serde_json::to_writer(writer, &osu_data.to_mc_data()).unwrap();

    let sr = calculate_from_data(&osu_data, 1.0);
    println!("{:?}", sr);
}
