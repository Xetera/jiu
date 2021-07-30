#![feature(type_alias_impl_trait)]
#![feature(async_closure)]
pub mod db;
pub mod models;
pub mod request;
pub mod scraper;
pub mod server;
pub mod webhook;
pub use dotenv::dotenv;
pub use std::env;
