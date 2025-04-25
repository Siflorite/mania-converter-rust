use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder, get, post};
use actix_multipart::Multipart;
use actix_files::NamedFile;
use futures_util::stream::StreamExt as _;
use std::fs::{self, File};
use std::io::Write;
use std::env;
use uuid::Uuid;
use std::sync::Mutex;
use std::collections::HashMap;
use lazy_static::lazy_static;

use crate::mcz2osz::process_mcz_file;

lazy_static! {
    #[derive(Debug)]
    static ref FILE_NAME_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

fn generate_unique_filename(extension: &str) -> String {
    let unique_id = Uuid::new_v4();
    format!("{}.{}", unique_id, extension)
}

// HTML 页面：上传文件表单
#[get("/")]
pub async fn upload_page() -> impl Responder {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Upload MCZ File</title>
        </head>
        <body>
            <h1>Upload your .mcz file</h1>
            <form action="/upload" method="post" enctype="multipart/form-data">
                <input type="file" name="file" accept=".mcz" required>
                <button type="submit">Upload</button>
            </form>
        </body>
        </html>
    "#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

// 上传文件的处理逻辑
#[post("/upload")]
pub async fn upload_mcz(mut payload: Multipart) -> impl Responder {
    // 创建临时目录用于存储上传的文件
    let temp_dir_path = env::temp_dir().join("upload_temp");
    if !temp_dir_path.exists() {
        fs::create_dir_all(&temp_dir_path).unwrap();
    }

    // 遍历 multipart 数据流
    while let Some(Ok(mut field)) = payload.next().await {
        let content_disposition = field.content_disposition();
        if let Some(original_filename) = content_disposition.get_filename() {
            // 将 UUID 文件名和原始文件名存入映射
            let unique_filename = generate_unique_filename("mcz");
            FILE_NAME_MAP.lock().unwrap().insert(unique_filename.clone(), original_filename.to_string());
            {
            let map = FILE_NAME_MAP.lock().unwrap();
            println!("HashMap content: {:?}", *map);
            }
           // 保存上传的文件到临时目录
            let filepath = temp_dir_path.join(unique_filename);
            println!("File uploading: {}", filepath.display());
            let mut f = File::create(&filepath).unwrap();
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                f.write_all(&data).unwrap();
            }
            println!("File uploaded: {}", filepath.display());
           
            // 检查扩展名是否为 .mcz
            if filepath.extension() == Some(std::ffi::OsStr::new("mcz")) {
                // 转换为 .osz 文件
                if let Err(e) = process_mcz_file(&filepath) {
                    return HttpResponse::InternalServerError().body(format!("Error: {:?}", e));
                }
                // 返回跳转到下载页面的 HTML
                let osz_filename = filepath.with_extension("osz").file_name().unwrap().to_string_lossy().to_string();
                let download_url = format!("/download/{}", osz_filename);
                let html = format!(
                    r#"
                    <!DOCTYPE html>
                    <html>
                    <head>
                        <title>Download OSZ File</title>
                    </head>
                    <body>
                        <h1>File converted successfully!</h1>
                        <p><a href="{}">Click here to download your .osz file</a></p>
                    </body>
                    </html>
                    "#,
                    download_url
                );

                return HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(html);
            } else {
                return HttpResponse::BadRequest().body("File is not a .mcz file");
            }
        }
    }

    HttpResponse::BadRequest().body("No file uploaded")
}

// 提供下载 .osz 文件的处理逻辑
#[get("/download/{filename}")]
pub async fn download_osz(filename: web::Path<String>, req: HttpRequest) -> impl Responder {
    let filename = filename.into_inner(); // 提取路径参数
    let temp_dir = env::temp_dir().join("upload_temp");
    let file_path = temp_dir.join(&filename);

    if !file_path.exists() {
        return HttpResponse::NotFound().body("File not found");
    }
    let mut key = std::path::PathBuf::from(file_path.file_name().unwrap());
    key.set_extension("mcz");
    let original_filename = {
        let mut map = FILE_NAME_MAP.lock().unwrap();
        match map.remove(key.to_string_lossy().as_ref()) {
            Some(name) => {
                print!("Original name found: {}", name);
                // 将文件名的后缀从 mcz 修改为 osz
                let mut original_name = std::path::PathBuf::from(name.clone());
                original_name.set_extension("osz"); // 修改扩展名为 osz
                original_name.file_name().unwrap().to_string_lossy().to_string() // 转换为字符串
            }
            None => filename.clone(), // 如果找不到映射，则使用生成的 UUID 文件名
        }
    };
    match NamedFile::open(&file_path) {
        Ok(named_file) => {
            // 设置 Content-Disposition 头，指定下载文件名
            let response = named_file
                .use_last_modified(true)
                .set_content_disposition(actix_web::http::header::ContentDisposition {
                    disposition: actix_web::http::header::DispositionType::Attachment,
                    parameters: vec![actix_web::http::header::DispositionParam::Filename(
                        original_filename.clone(),
                    )],
                });

            // 异步删除文件（.osz 和 .mcz 文件）
            let file_path_clone = file_path.clone();
            actix_web::rt::spawn(async move {
                // 删除 .osz 文件
                if let Err(e) = fs::remove_file(&file_path_clone) {
                    eprintln!("Error deleting .osz file: {}", e);
                }

                // 删除对应的 .mcz 文件
                let original_file_path = file_path_clone.with_extension("mcz");
                if original_file_path.exists() {
                    if let Err(e) = fs::remove_file(&original_file_path) {
                        eprintln!("Error deleting .mcz file: {}", e);
                    }
                }
            });

            response.into_response(&req)
        }
        Err(_) => HttpResponse::InternalServerError().body("Error opening file"),
    }       
}

// 启动 Web 服务
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().service(
            web::scope("")
                .service(upload_page)
                .service(upload_mcz)
                .service(download_osz)
        )
    })
    .bind(("0.0.0.0", 80))?
    .run()
    .await
}