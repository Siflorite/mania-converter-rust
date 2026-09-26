use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::misc::sanitize_filename;

/// Extract every legal file from `path` to `destination`, flattened and file name sanitized.
/// Rejects entries with path traversal or other dirty stuff by [`zip::read::ZipFile::enclosed_name()`].
pub(crate) fn extract_archive(path: &Path, destination: &Path) -> io::Result<()> {
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
        let target_path = destination.join(sanitize_filename(pure_file_name));

        let mut output = File::create(&target_path)?;
        io::copy(&mut entry, &mut output)?;
    }

    Ok(())
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
