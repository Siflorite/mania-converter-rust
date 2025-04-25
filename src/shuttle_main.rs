// use mania_converter::{mcz2osz, webapp::{download_osz, upload_mcz, upload_page}};
// use shuttle_actix_web::ShuttleActixWeb;
// use actix_web::web;

// #[shuttle_runtime::main]
// async fn main() -> ShuttleActixWeb<impl FnOnce(&mut web::ServiceConfig) + Send + Clone + 'static> {
//     let config = move |cfg: &mut web::ServiceConfig| {
//         // set up your service here, e.g.:
//         cfg.service(
//             web::scope("")
//                 .service(upload_page)
//                 .service(upload_mcz)
//                 .service(download_osz)
//         );
//     };

//     Ok(config.into())
// }