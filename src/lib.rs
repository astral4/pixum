#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

mod routes;
pub use routes::*;

use reqwest::Client;
use std::time::Duration;

pub struct AppState {
    client: Client,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        let client = Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build reqwest Client");
        Self { client }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
