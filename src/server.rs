use actix::{Actor, StreamHandler};
use actix_files as fs;
use actix_web::{
    get,
    web::{self, Data},
    App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use log::{error, info};
use std::sync::Arc;

use crate::db::{latest_requests, Database};

struct Context {
    db: Arc<Database>,
}

#[get("/requests")]
async fn get_requests(ctx: web::Data<Context>) -> impl Responder {
    match latest_requests(&ctx.db, true).await {
        Ok(data) => HttpResponse::Ok().body(serde_json::to_value(data).unwrap().to_string()),
        Err(err) => {
            error!("{:?}", err);
            HttpResponse::InternalServerError().body("[]")
        }
    }
}

pub async fn run_server(db: Arc<Database>) -> std::io::Result<()> {
    info!("Starting server");
    HttpServer::new(move || {
        App::new()
            .service(get_requests)
            .service(fs::Files::new("", "./dist").prefer_utf8(true))
            .app_data(Data::new(Context {
                db: Arc::clone(&db),
            }))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
