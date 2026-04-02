pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod app;
mod browse;
mod cli;
mod config;
mod curl_runner;
mod database;
mod github;
mod keywords;
mod postman_import;
mod web;

pub use app::run;
