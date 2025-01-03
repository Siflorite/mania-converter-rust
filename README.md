# mania-converter-rust
A rust realization of former osu-mania-editor.<br>
## Standalone Usage
Download the `converter_rust_x.x.x_standalone.exe`. Simply put this executable file under the same directory of mcz file and double click, all mcz files will be converted to osz files.
## Webapp Deployment
Download `converter_rust_x.x.x_webapp.exe`. Run the application as Administrator, and upload your mcz file at [localhost](http:://localhost/). If you want to deploy it on a server, you can either compile and run it in unix-like systems or run the exe on Windows Server. You need to do port forwarding if you don't have a public IP (like at home) to make it accesible to Internet, and keep port 80 open for TCP protocol at firewall since it is the port for http.
## Compiling Issues
From ver 0.2.0 `mod`s are used to make it easier to infer the processing functions.<br> 
`mcz2osz` provides two public functions: `process_whole_dir_mcz` produces the osz transform of all mcz files under the working directory; `process_mcz_file` takes in a `&Path` parameter and transforms this specific mcz file into osz format. An example can be given as `process_whole_dir_mcz()`:
```rust
pub fn process_whole_dir_mcz() -> io::Result<()> {
    let current_dir = "."; 
    
    for entry in WalkDir::new(current_dir) {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension() == Some(std::ffi::OsStr::new("mcz")) {
            process_mcz_file(path)?;
        }
    }
    
    Ok(())
}
```
`webapp` uses `process_mcz_file()` and sets up a web application to receive .mcz file uploads and transform it into .osz file. Just run the main function and it does all the job.
