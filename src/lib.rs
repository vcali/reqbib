pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod app;
mod cli;
mod config;
mod database;
mod github;
mod keywords;
mod postman_import;

pub use app::run;
