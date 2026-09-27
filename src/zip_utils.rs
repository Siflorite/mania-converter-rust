use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::misc::sanitize_filename;

/// Whether an archive contains source-format and target-format charts, respectively.
/// Inspect entry names before extraction, which otherwise flattens and sanitizes names.
pub(crate) fn chart_formats(
    path: &Path,
    source_extension: &str,
    target_extension: &str,
) -> io::Result<(bool, bool)> {
    let mut archive = ZipArchive::new(File::open(path)?)?;
    let mut has_target = false;
    let mut has_source = false;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        if let Some(extension) = Path::new(entry.name()).extension().and_then(|s| s.to_str()) {
            has_source |= extension.eq_ignore_ascii_case(source_extension);
            has_target |= extension.eq_ignore_ascii_case(target_extension);
        }
    }
    Ok((has_source, has_target))
}

/// Extract every legal file from `path` to `destination`, flattened and file name sanitized.
/// Rejects entries with path traversal or other dirty stuff by [`zip::read::ZipFile::enclosed_name()`].
pub(crate) fn extract_archive(path: &Path, destination: &Path) -> io::Result<()> {
    extract_archive_impl(path, destination, false)
}

fn extract_archive_impl(path: &Path, destination: &Path, preserve_paths: bool) -> io::Result<()> {
    fs::create_dir_all(destination)?;

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;

        let Some(enclosed_file_name) = entry.enclosed_name() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Refuse to extract entry with unsafe path: {}", entry.name()),
            ));
        };

        if entry.is_dir() {
            continue;
        }

        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Symlink entry not supported: {}", entry.name()),
            ));
        }

        let Some(pure_file_name) = enclosed_file_name.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let target_path = if preserve_paths {
            destination.join(enclosed_file_name)
        } else {
            destination.join(sanitize_filename(pure_file_name))
        };
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut output = File::create(&target_path)?;
        io::copy(&mut entry, &mut output)?;
    }

    Ok(())
}

/// Convert source charts in a mixed archive while retaining existing target charts
/// and all assets, including paths and assets not understood by our chart parsers.
/// Metadata describes converted charts only; retained charts are never rewritten.
pub(crate) fn convert_mixed_archive<F>(
    input: &Path,
    directory: &Path,
    source_extension: &str,
    target_extension: &str,
    archive_extension: &str,
    mut convert: F,
) -> io::Result<(PathBuf, Vec<crate::BeatMapInfo>)>
where
    F: FnMut(&Path, &Path) -> io::Result<crate::BeatMapInfo>,
{
    extract_archive_impl(input, directory, true)?;
    // Snapshot before generating files, so generated charts are never input again.
    let mut original_files = Vec::new();
    for entry in walkdir::WalkDir::new(directory) {
        let entry = entry?;
        if entry.file_type().is_file() {
            original_files.push(entry.into_path());
        }
    }
    original_files.sort();
    let mut used_names: HashSet<_> = original_files
        .iter()
        .map(|path| path.to_string_lossy().to_lowercase())
        .collect();
    let mut output_files = Vec::new();
    let mut infos = Vec::new();
    for source in original_files {
        if !source
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(source_extension))
        {
            output_files.push(source);
            continue;
        }
        let mut target = source.with_extension(target_extension);
        let stem = source.file_stem().unwrap_or_default().to_string_lossy();
        let mut suffix = 1;
        while used_names.contains(&target.to_string_lossy().to_lowercase()) || target.exists() {
            target =
                source.with_file_name(format!("{stem} (converted {suffix}).{target_extension}"));
            suffix += 1;
        }
        used_names.insert(target.to_string_lossy().to_lowercase());
        // Fail before writing the output archive if any source chart cannot convert.
        let mut info = convert(&source, &target)?;
        // Postprocessors resolve backgrounds against the extraction root, not
        // the individual chart's directory. Keep chart references themselves intact.
        if !info.bg_name.is_empty() {
            let background = source.parent().unwrap_or(directory).join(&info.bg_name);
            info.bg_name = background
                .strip_prefix(directory)
                .map_err(io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");
        }
        infos.push(info);
        output_files.push(target);
    }

    output_files.sort();
    let output = input.with_extension(archive_extension);
    let mut writer = ZipWriter::new(File::create(&output)?);
    for path in output_files {
        let name = path.strip_prefix(directory).map_err(io::Error::other)?;
        writer.start_file(
            name.to_string_lossy().replace('\\', "/"),
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )?;
        io::copy(&mut File::open(&path)?, &mut writer)?;
    }
    writer.finish()?;
    Ok((output, infos))
}

/// Write files to zip archive in [`CompressionMethod::Stored`].
///
/// Files written in sorted order, so that the zip archive is reproducible.
pub(crate) fn create_archive(output: &Path, files: &HashSet<PathBuf>) -> io::Result<()> {
    let mut paths = files.iter().collect::<Vec<_>>();
    paths.sort();

    let mut files_dedup = HashMap::new();
    paths.into_iter().for_each(|path| {
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            files_dedup.insert(name, path);
        }
    });

    if files_dedup.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Provided files hashset empty",
        ));
    }

    let mut sorted_files = files_dedup.into_iter().collect::<Vec<_>>();
    sorted_files.sort_by_key(|(a, _)| *a);

    let file = File::create(output)?;
    let mut zip_writer = ZipWriter::new(file);

    for (file_name, path) in sorted_files {
        let mut file = File::open(path)?;
        zip_writer.start_file(
            file_name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )?;
        io::copy(&mut file, &mut zip_writer)?;
    }

    zip_writer.finish()?;
    Ok(())
}
