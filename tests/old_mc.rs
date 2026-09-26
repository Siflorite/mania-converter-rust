use mania_converter::malody::McData;

#[test]
fn test_mc() {
    let data = McData::from_file("./tests/beatmaps/4835 Biemote - 6K Another Lv.29.mc").unwrap();
    let osu_data = data.to_osu_data().unwrap();
    println!("{:?}", osu_data);
}
