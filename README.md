# mania-converter-rust
A rust realization of former osu-mania-editor.<br>
## Standalone Usage
Download the `converter_rust_x.x.x_standalone.exe`. Simply put this executable file under the same directory of mcz file and double click, all mcz files will be converted to osz files.
## Webapp Deployment
Download `converter_rust_x.x.x_webapp.exe`. Run the application as Administrator, and upload your mcz file at [localhost](http:://localhost/). If you want to deploy it on a server, you can either compile and run it in unix-like systems or run the exe on Windows Server. You need to do port forwarding if you don't have a public IP (like at home) to make it accesible to Internet, and keep port 80 open for TCP protocol at firewall since it is the port for http.
## Shuttle Deployment
[Shuttle](https://www.shuttle.dev/) is a service for deploying free serverless rust backends. You can copy the contents in shuttle_main.rs to main.rs, annotating the main function in webapp.rs (as there will be two `main` entries), then run `shuttle deploy` after configuring shuttle cli following Shuttle's documentations, to deploy this to shuttle. I personally don't recommend this deployment, as it won't provide a fancy frontend (since it's mainly a backend project), and the upload speed is rather limited on shuttle, which takes minutes to upload a file to the website.
## Compiling Issues
From ver 0.2.0 `mod`s are used to make it easier to infer the processing functions.<br> 
`mcz2osz` provides two public functions: `process_whole_dir_mcz` produces the osz transform of all mcz files under the working directory; `process_mcz_file` takes in a `&Path` parameter and transforms this specific mcz file into osz format.<br>
From ver 0.4.0, as SR calculation algorithm translated from sunnyxxy's [Python verion](https://github.com/sunnyxxy/Star-Rating-Rebirth) has been used, the functions have been developed with new parameters `b_calc_sr` and `b_print_results`. They are boolean values indicating whether to calculate SR(makes the program a bit slower) and to print datas after processing.<br>
An example of using `process_mcz_file` and `process_whole_dir_mcz()` is given as follow:
```rust
use std::path::Path;
pub fn main() -> io::Result<()> {
    let p = Path::new("");
    let (calc_sr, print_results) = (true, true);

    // The return value is a tuple of the path of converted osz file,
    // And a Vec<BeatMapInfo> containing the information of maps inside.
    let (osz_dir, info_vec) = process_mcz_file(p, calc_sr);
    println!("{info_vec}");
    
    // You can use either "" or "." as the first parameter to indicater the working directory.
    // Then the compiled binary file will convert all mcz files under its directory.
    process_whole_dir_mcz("", calc_sr, print_results)?;
    Ok(())
}
```
`webapp` uses `process_mcz_file()` and sets up a web application to receive .mcz file uploads and transform it into .osz file. Just run the main function and it does all the job.<br>
From version 0.4.0, webapp is no longer combined to mania-converter, making it standalone to reduce size and dependencies used. Now to compile webapp, just go to the `/webapp` folder and run `cargo build -r`.
## Using as dependency
As you may have found, I silently added lib.rs to `/src`, meaning it can be used as a library now...<br>
It is very easy to use: first add it to the `[dependencies]` in your `Cargo.toml`
```TOML
[dependencies]
mania-converter = {git = "https://github.com/Siflorite/mania-converter-rust"}
# Or you can download it if you fail to fetch it through git
# mania-converter = {path = "Path to mania-converter's Cargo.toml"}
```
Then you can just use it in your project:
```Rust
use mania_converter::mcz2osz::process_mcz_file;
// Application
pub fn process(file_path: &str) -> io::Result<PathBuf> {
    let mcz_path = Path::new(&file_path).to_path_buf();
    process_mcz_file(mcz_path)
}
```

## TODO
~~SR Calculation.~~ Added in v0.4.0
.osu/.osz to .mc/.mcz
Some other converting stuff?