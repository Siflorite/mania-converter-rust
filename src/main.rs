pub mod mcz2osz;
pub mod webapp;

fn main() -> std::io::Result<()>{
    // mcz2osz::process_whole_dir_mcz();
    webapp::main()
}