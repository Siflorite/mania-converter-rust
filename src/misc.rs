//! General Functions

/// Sanitize a filename by replacing non-ASCII and illegal characters with underscores.
/// Mainly for osu!.
///
/// A common problem is that osu! uses SHIFT-JIS to decode all possible Chinese characters,
/// while Windows zip softwares often use GBK to encode (in China).
/// Therefore file name will be corrupted when loading `.osz`s in osu!, leading to unable to find the file.
///
/// Another problem is that Windows do not allow specific characters in file names, such as `/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`.
/// But `.mcz` files generated in Malody Android version may contain these characters in file names.
///
/// A safe solution is to replace all non-ASCII and illegal characters with underscores.
pub(crate) fn sanitize_filename(file_name: &str) -> String {
    // 将文件名中的非ASCII字符替换为下划线
    file_name
        .chars()
        .map(|c| {
            if c.is_ascii() && !r#"\/:*?"<>|"#.contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}
