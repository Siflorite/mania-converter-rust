// General Functions

pub fn sanitize_filename(file_name: &str) -> String {
    // 将文件名中的非ASCII字符替换为下划线
    file_name.chars()
        .map(|c| {
            if c.is_ascii() && !r#"\/:*?"<>|"#.contains(c) {
                c 
            } else {
                '_' 
            }
        })
        .collect()
}