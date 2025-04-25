use mania_converter::{mcz2osz::process_whole_dir_mcz, webapp};
use std::io;

fn main() -> io::Result<()> {
    process_whole_dir_mcz("")?;
    // webapp::main();
    Ok(())
}