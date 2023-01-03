#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

mod routes;
pub use routes::*;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use std::time::Duration;

pub struct AppState {
    client: Client,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.append("Accept-Language", HeaderValue::from_static("en"));

        let client = Client::builder()
            .default_headers(headers)
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

type AppResult<T> = Result<T, AppError>;

pub enum AppError {
    ArtworkUnavailable { msg: String },
    ServerUnreachable,
    Internal { msg: String },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        {
            match self {
                Self::ArtworkUnavailable { msg } => (
                    StatusCode::NOT_FOUND,
                    format!("Information of the requested work could not be retrieved. {msg}"),
                ),
                Self::ServerUnreachable => (
                    StatusCode::BAD_GATEWAY,
                    String::from("Failed to get response from Pixiv server."),
                ),
                Self::Internal { msg } => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("An internal server error occurred. {msg}"),
                ),
            }
        }
        .into_response()
    }
}
