use mania_converter::malody::{McData, process_mcz_file};
use mania_converter::osu::OsuDataLegacy;
use serde_json::{Value, json};
use std::fs::File;
use std::io::{Read, Write};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

fn chart(effects: Value) -> McData {
    serde_json::from_value(json!({
        "meta": {
            "creator": "Test", "background": "", "version": "SV test",
            "mode": 0, "mode_ext": {"column": 6},
            "song": {"title": "SV", "artist": "Test"}
        },
        "time": [
            {"beat": [0, 0, 1], "bpm": 120},
            {"beat": [4, 0, 1], "bpm": 240},
            {"beat": [8, 0, 1], "bpm": 120},
            {"beat": [12, 0, 1], "bpm": 240}
        ],
        "effect": effects,
        "note": [
            {"beat": [0, 0, 1], "column": 0},
            {"beat": [3, 0, 1], "endbeat": [5, 0, 1], "column": 1},
            {"beat": [13, 0, 1], "column": 2}
        ]
    }))
    .unwrap()
}

fn rows(data: &OsuDataLegacy) -> Vec<(f64, f64, bool)> {
    data.timings
        .iter()
        .map(|p| (p.time, p.val, p.is_timing))
        .collect()
}

#[test]
fn scroll_survives_multiple_bpm_changes_without_moving_notes() {
    let source = chart(json!([
        {"beat": [2, 0, 1], "scroll": 2.0},
        {"beat": [10, 0, 1], "scroll": 0.5}
    ]));
    let converted = source.to_osu_data().unwrap();
    assert_eq!(
        rows(&converted),
        vec![
            (0., 500., true),
            (1000., -50., false),
            (2000., 250., true),
            (2000., -50., false),
            (3000., 500., true),
            (3000., -50., false),
            (4000., -200., false),
            (5000., 250., true),
            (5000., -200., false),
        ]
    );
    let plain = chart(json!([])).to_osu_data().unwrap();
    let notes = |data: &OsuDataLegacy| {
        data.notes
            .iter()
            .map(|n| (n.x_pos, n.time, n.end_time))
            .collect::<Vec<_>>()
    };
    assert_eq!(notes(&converted), notes(&plain));
    assert_eq!(
        notes(&converted),
        vec![(42, 0, None), (128, 1500, Some(2250)), (213, 5250, None)]
    );
}

#[test]
fn explicit_sv_at_bpm_change_wins_and_becomes_the_persistent_value() {
    // Effects need not be sorted; the last authored effect wins at equal times.
    let converted = chart(json!([
        {"beat": [4, 0, 1], "scroll": 0.5},
        {"beat": [0, 0, 1], "scroll": 2.0},
        {"beat": [4, 0, 1], "scroll": 1.0}
    ]))
    .to_osu_data()
    .unwrap();
    assert_eq!(
        rows(&converted),
        vec![
            (0., 500., true),
            (0., -50., false),
            (2000., 250., true),
            (2000., -200., false),
            (2000., -100., false),
            (3000., 500., true),
            (3000., -100., false),
            (5000., 250., true),
            (5000., -100., false),
        ]
    );
}

#[test]
fn no_scroll_effects_does_not_add_green_lines() {
    for effects in [Value::Null, json!([])] {
        assert_eq!(
            rows(&chart(effects).to_osu_data().unwrap()),
            vec![
                (0., 500., true),
                (2000., 250., true),
                (3000., 500., true),
                (5000., 250., true),
            ]
        );
    }
}

#[test]
fn archive_conversion_writes_restored_sv_after_red_lines() -> std::io::Result<()> {
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("sv.mcz");
    let mut archive = ZipWriter::new(File::create(&input)?);
    archive.start_file("sv.mc", SimpleFileOptions::default())?;
    archive.write_all(&serde_json::to_vec(&chart(json!([
        {"beat": [0, 0, 1], "scroll": 2.0}
    ])))?)?;
    archive.finish()?;

    let (output, infos) = process_mcz_file(&input, false)?;
    assert_eq!(infos.len(), 1);
    let mut archive = ZipArchive::new(File::open(output)?)?;
    let mut text = String::new();
    archive.by_name("sv.osu")?.read_to_string(&mut text)?;
    let timings = text
        .split("[TimingPoints]")
        .nth(1)
        .unwrap()
        .split("[HitObjects]")
        .next()
        .unwrap();
    let rows: Vec<Vec<&str>> = timings
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("//"))
        .map(|line| line.split(',').collect())
        .collect();
    assert_eq!(rows.len(), 8);
    for pair in rows.chunks_exact(2) {
        assert_eq!(pair[0][0], pair[1][0]);
        assert_eq!(pair[0][6], "1");
        assert_eq!(pair[1][6], "0");
        assert_eq!(pair[1][1], "-50");
    }
    Ok(())
}
