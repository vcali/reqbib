pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod app;
mod cli;
mod config;
mod database;
mod github;
mod history;
mod keywords;

pub use app::run;
