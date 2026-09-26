use std::fs::{self, File};

use mania_converter::malody::McData;
use mania_converter::osu::process_osz_file;
use zip::ZipArchive;

#[test]
fn converts_osz_to_mcz() -> std::io::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/beatmaps/Yomitan Akane - Chilly.osz"
    );
    let input = temp_dir.path().join("test.osz");
    fs::copy(source, &input)?;

    let (output, infos) = process_osz_file(&input, false)?;
    assert_eq!(output, input.with_extension("mcz"));
    assert!(!infos.is_empty());

    let mut archive = ZipArchive::new(File::open(&output)?)?;
    let mut mc_count = 0;

    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.name().ends_with(".mc") {
            let _: McData = serde_json::from_reader(entry)?;
            mc_count += 1;
        }
    }

    assert!(mc_count > 0, "MCZ contains no .mc charts");
    assert!(archive.len() > mc_count, "MCZ contains no resource files");

    Ok(())
}
