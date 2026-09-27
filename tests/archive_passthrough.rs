use mania_converter::malody::{McData, process_mcz_file, process_mcz_file_postprocess};
use mania_converter::osu::{process_osz_file, process_osz_file_postprocess};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

fn write_archive(path: &Path, entries: &[(&str, &[u8])]) -> io::Result<()> {
    let mut archive = ZipWriter::new(File::create(path)?);
    archive.set_comment("Preserve this archive comment too")?;
    // A directory named like a source chart must not prevent passthrough.
    archive.add_directory("directory.mc/", SimpleFileOptions::default())?;
    archive.add_directory("directory.osu/", SimpleFileOptions::default())?;
    for &(name, data) in entries {
        archive.start_file(name, SimpleFileOptions::default())?;
        archive.write_all(data)?;
    }
    archive.finish()?;
    Ok(())
}

#[test]
fn mislabeled_archives_are_copied_byte_for_byte_in_both_directions() -> io::Result<()> {
    for (extension, target, chart) in [
        ("mcz", "osz", "charts/map.osu"),
        ("osz", "mcz", "charts/map.mc"),
        ("mcz", "osz", "charts/map.OSU"),
        ("osz", "mcz", "charts/map.MC"),
    ] {
        let temp = tempfile::tempdir()?;
        let input = temp.path().join(format!("original.{extension}"));
        // Passthrough must not depend on parsing or rewriting chart contents.
        write_archive(
            &input,
            &[
                (chart, b"chart bytes are opaque during passthrough"),
                ("audio/song.ogg", b"audio"),
                ("one/background.jpg", b"first background"),
                ("two/background.jpg", b"second background"),
                ("extra/unreferenced.txt", b"keep this too"),
            ],
        )?;
        let original = fs::read(&input)?;
        let (output, infos) = if extension == "mcz" {
            process_mcz_file(&input, true)?
        } else {
            process_osz_file(&input, true)?
        };
        assert_eq!(output, input.with_extension(target));
        assert_eq!(fs::read(&output)?, original);
        assert_eq!(fs::read(&input)?, original, "input must be retained");
        assert!(infos.is_empty(), "passthrough does not parse charts");
    }
    Ok(())
}

#[test]
fn passthrough_still_calls_postprocess_with_original_files() -> io::Result<()> {
    for (extension, chart) in [("mcz", "map.osu"), ("osz", "map.mc")] {
        let temp = tempfile::tempdir()?;
        let input = temp.path().join(format!("original.{extension}"));
        write_archive(&input, &[(chart, b"original chart")])?;
        let mut calls = 0;
        let mut callback = |infos: &[mania_converter::BeatMapInfo], extracted: &Path| {
            calls += 1;
            assert!(infos.is_empty());
            assert_eq!(fs::read(extracted.join(chart))?, b"original chart");
            Ok(())
        };
        let output = if extension == "mcz" {
            process_mcz_file_postprocess(&input, false, &mut callback)?
        } else {
            process_osz_file_postprocess(&input, false, &mut callback)?
        };
        assert_eq!(calls, 1);
        assert_eq!(fs::read(input)?, fs::read(output)?);
    }
    Ok(())
}

#[test]
fn mixed_archives_still_convert_source_charts() -> io::Result<()> {
    let mc: McData = serde_json::from_value(serde_json::json!({
        "meta": {
            "creator": "Test", "background": "", "version": "Archive test",
            "mode": 0, "mode_ext": {"column": 6},
            "song": {"title": "Archive", "artist": "Test"}
        },
        "time": [{"beat": [0, 0, 1], "bpm": 120}],
        "note": [
            {"beat": [1, 0, 1], "column": 0},
            {"beat": [2, 0, 1], "endbeat": [3, 0, 1], "column": 1}
        ]
    }))?;
    let temp = tempfile::tempdir()?;
    let osu_path = temp.path().join("source.osu");
    mc.to_osu_data()?.to_file(&osu_path.to_string_lossy())?;
    let osu_bytes = fs::read(osu_path)?;
    let mc_bytes = serde_json::to_vec(&mc)?;
    for (extension, generated, original_chart) in [
        ("mcz", "malody.osu", "malody.mc"),
        ("osz", "osu.mc", "osu.osu"),
    ] {
        let input = temp.path().join(format!("mixed.{extension}"));
        write_archive(&input, &[("malody.mc", &mc_bytes), ("osu.osu", &osu_bytes)])?;
        let before = fs::read(&input)?;
        let (output, infos) = if extension == "mcz" {
            process_mcz_file(&input, false)?
        } else {
            process_osz_file(&input, false)?
        };
        assert_eq!(infos.len(), 1);
        assert_ne!(fs::read(&output)?, before);
        assert_eq!(fs::read(&input)?, before);
        let mut archive = ZipArchive::new(File::open(output)?)?;
        assert!(archive.by_name(generated).is_ok());
        assert!(archive.by_name(original_chart).is_err());
        let (retained, expected) = if extension == "mcz" {
            ("osu.osu", &osu_bytes)
        } else {
            ("malody.mc", &mc_bytes)
        };
        let mut bytes = Vec::new();
        archive.by_name(retained)?.read_to_end(&mut bytes)?;
        assert_eq!(&bytes, expected);
    }
    Ok(())
}

#[test]
fn uppercase_source_charts_prevent_passthrough_of_mixed_archives() -> io::Result<()> {
    for (extension, source, target) in [
        ("mcz", "source.MC", "target.osu"),
        ("osz", "source.OSU", "target.mc"),
    ] {
        let temp = tempfile::tempdir()?;
        let input = temp.path().join(format!("mixed.{extension}"));
        write_archive(&input, &[(source, b"invalid"), (target, b"invalid")])?;
        let result = if extension == "mcz" {
            process_mcz_file(&input, false)
        } else {
            process_osz_file(&input, false)
        };
        // Invalid source charts must fail, rather than silently output a partial pack.
        assert!(result.is_err());
        assert!(
            !input
                .with_extension(if extension == "mcz" { "osz" } else { "mcz" })
                .exists()
        );
    }
    Ok(())
}

#[test]
fn mixed_archives_preserve_nested_assets_and_resolve_chart_name_collisions() -> io::Result<()> {
    let mc: McData = serde_json::from_value(serde_json::json!({
        "meta": {
            "creator": "Test", "background": "pics/new.jpg", "version": "New",
            "mode": 0, "mode_ext": {"column": 6},
            "song": {"title": "Mixed", "artist": "Test"}
        },
        "time": [{"beat": [0, 0, 1], "bpm": 120}],
        "note": [
            {"beat": [0, 0, 1], "sound": "audio/new.ogg", "type": 1, "offset": 0},
            {"beat": [1, 0, 1], "column": 0, "sound": "sounds/new.wav"},
            {"beat": [2, 0, 1], "endbeat": [3, 0, 1], "column": 1}
        ]
    }))?;
    let temp = tempfile::tempdir()?;
    let osu_path = temp.path().join("source.osu");
    mc.to_osu_data()?.to_file(&osu_path.to_string_lossy())?;
    let osu_bytes = fs::read(osu_path)?;
    let mc_bytes = serde_json::to_vec(&mc)?;

    for (extension, source_extension, target_extension, source_bytes, target_bytes) in [
        ("mcz", "MC", "osu", &mc_bytes, &osu_bytes),
        ("osz", "OSU", "mc", &osu_bytes, &mc_bytes),
    ] {
        let input = temp.path().join(format!("nested.{extension}"));
        let source = format!("charts/map.{source_extension}");
        let target = format!("charts/map.{}", target_extension.to_uppercase());
        let reserved = format!("charts/map (converted 1).{target_extension}");
        let generated = format!("charts/map (converted 2).{target_extension}");
        let original_target = String::from_utf8(target_bytes.clone())
            .unwrap()
            .replace("new.", "old.");
        let mut entries: Vec<(&str, &[u8])> = vec![
            (&source, source_bytes),
            (&target, original_target.as_bytes()),
            (&reserved, original_target.as_bytes()),
        ];
        let assets: &[(&str, &[u8])] = &[
            ("charts/audio/new.ogg", b"new audio"),
            ("charts/audio/old.ogg", b"old audio"),
            ("charts/pics/new.jpg", b"new background"),
            ("charts/pics/old.jpg", b"old background"),
            ("charts/sounds/new.wav", b"new hitsound"),
            ("charts/sounds/old.wav", b"old hitsound"),
            ("one/shared.png", b"first image"),
            ("two/shared.png", b"second image"),
            ("storyboard.osb", b"storyboard bytes"),
            ("video.mp4", b"video bytes"),
        ];
        entries.extend_from_slice(assets);
        write_archive(&input, &entries)?;
        let before = fs::read(&input)?;
        let mut calls = 0;
        let mut callback = |infos: &[mania_converter::BeatMapInfo], extracted: &Path| {
            calls += 1;
            assert_eq!(infos.len(), 1);
            assert_eq!(infos[0].bg_name, "charts/pics/new.jpg");
            assert!(extracted.join(&infos[0].bg_name).is_file());
            assert_eq!(
                fs::read(extracted.join(&target))?,
                original_target.as_bytes()
            );
            assert!(extracted.join(&generated).is_file());
            assert!(extracted.join("charts/audio/old.ogg").is_file());
            Ok(())
        };
        let output = if extension == "mcz" {
            process_mcz_file_postprocess(&input, false, &mut callback)?
        } else {
            process_osz_file_postprocess(&input, false, &mut callback)?
        };
        assert_eq!(calls, 1);
        assert_eq!(fs::read(&input)?, before);
        let mut archive = ZipArchive::new(File::open(output)?)?;
        assert!(archive.by_name(&source).is_err());
        assert_eq!(archive.len(), assets.len() + 3);
        for (name, expected) in entries.iter().filter(|(name, _)| *name != source) {
            let mut actual = Vec::new();
            archive.by_name(name)?.read_to_end(&mut actual)?;
            assert_eq!(&actual, expected, "{name} must be preserved exactly");
        }
        let mut converted = String::new();
        archive
            .by_name(&generated)?
            .read_to_string(&mut converted)?;
        for reference in ["audio/new.ogg", "pics/new.jpg", "sounds/new.wav"] {
            assert!(
                converted.contains(reference),
                "missing reference: {reference}"
            );
        }
    }
    Ok(())
}

#[test]
fn mixed_conversion_failure_does_not_replace_an_existing_output() -> io::Result<()> {
    for (extension, target, source_chart, target_chart) in [
        ("mcz", "osz", "broken.mc", "keep.osu"),
        ("osz", "mcz", "broken.osu", "keep.mc"),
    ] {
        let temp = tempfile::tempdir()?;
        let input = temp.path().join(format!("mixed.{extension}"));
        write_archive(
            &input,
            &[(source_chart, b"invalid"), (target_chart, b"keep")],
        )?;
        let output = input.with_extension(target);
        fs::write(&output, b"previous output")?;
        let result = if extension == "mcz" {
            process_mcz_file(&input, false)
        } else {
            process_osz_file(&input, false)
        };
        assert!(result.is_err());
        assert_eq!(fs::read(output)?, b"previous output");
    }
    Ok(())
}

#[test]
fn archives_without_charts_are_not_copied() -> io::Result<()> {
    for extension in ["mcz", "osz"] {
        let temp = tempfile::tempdir()?;
        let input = temp.path().join(format!("empty.{extension}"));
        write_archive(&input, &[("readme.txt", b"no charts")])?;
        let result = if extension == "mcz" {
            process_mcz_file(&input, false)
        } else {
            process_osz_file(&input, false)
        };
        assert!(result.is_err());
        assert!(
            !input
                .with_extension(if extension == "mcz" { "osz" } else { "mcz" })
                .exists()
        );
    }
    Ok(())
}
