// use actix_files as fs;
// use actix_web::{
//     get,
//     web::{self, Data},
//     App, HttpResponse, HttpServer, Responder,
// };
// use log::{debug, error, info};
// use std::sync::Arc;
//
// use crate::db::{latest_requests, Database};
//
// struct Context {
//     db: Arc<Database>,
// }
//
// #[get("/api/requests")]
// async fn get_requests(ctx: web::Data<Context>) -> impl Responder {
//     match latest_requests(&ctx.db, true).await {
//         Ok(data) => {
//             debug!("Got response from latest_request");
//             HttpResponse::Ok().body(serde_json::to_value(data).unwrap().to_string())
//         }
//         Err(err) => {
//             error!("{:?}", err);
//             HttpResponse::InternalServerError().body("[]")
//         }
//     }
// }
//
// pub async fn run_server(db: Arc<Database>) -> std::io::Result<()> {
//     info!("Starting server");
//     HttpServer::new(move || {
//         App::new()
//             .service(get_requests)
//             .service(fs::Files::new("", "./dist").prefer_utf8(true))
//             .app_data(Data::new(Context {
//                 db: Arc::clone(&db),
//             }))
//     })
//     .bind("127.0.0.1:8080")?
//     .run()
//     .await
// }
