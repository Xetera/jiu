use std::sync::Arc;

use actix::{Actor, StreamHandler};
use actix_web::{get, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use log::error;

use crate::db::{latest_requests, Database};

struct Context {
    db: Arc<Database>,
}

#[get("/requests")]
async fn hello(ctx: web::Data<Context>) -> impl Responder {
    match latest_requests(&ctx.db).await {
        Ok(data) => HttpResponse::Ok().body(serde_json::to_value(data).unwrap()),
        Err(err) => {
            error!("{:?}", err);
            HttpResponse::InternalServerError().body("[]")
        }
    }
}

async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

pub async fn run_server(db: Arc<Database>) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .service(hello)
            .data(Context {
                db: Arc::clone(&db),
            })
            .route("/hey", web::get().to(manual_hello))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
