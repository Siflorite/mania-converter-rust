use mania_converter::graphx::generate_osz_info;

#[test]
fn render_osz_info() {
    use std::path::Path;
    let file = "./tests/beatmaps/Yomitan Akane - Chilly.osz";
    let info_path = generate_osz_info(Path::new(file)).unwrap();
    println!("{}", info_path.to_string_lossy());
}
