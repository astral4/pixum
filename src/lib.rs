#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

mod routes;
pub use routes::*;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use deadpool_redis::{Config, Runtime, Pool};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use std::time::Duration;

pub struct AppState {
    client: Client,
    pool: Pool,
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

        let config = Config::from_url("redis://redis:6379/");
        let pool = config.create_pool(Some(Runtime::Tokio1)).expect("Failed to create database pool");
        
        Self { client, pool }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

type AppResult<T> = Result<T, AppError>;

pub enum AppError {
    InvalidUrl,
    ArtworkUnavailable,
    // This variant is essentially ArtworkUnavailable but for cache invalidation purposes
    WrongArtworkUrl,
    ServerUnreachable,
    ZeroQuery,
    TooHighQuery { max: u16 },
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        {
            match self {
                Self::InvalidUrl => (
                    StatusCode::NOT_FOUND,
                    String::from("The requested URL is invalid.")
                ),
                Self::ArtworkUnavailable | Self::WrongArtworkUrl => (
                    StatusCode::NOT_FOUND,
                    String::from("Information of the requested work could not be retrieved. The work might be deleted or have limited visibility."),
                ),
                Self::ServerUnreachable => (
                    StatusCode::BAD_GATEWAY,
                    String::from("Failed to get response from Pixiv server."),
                ),
                Self::ZeroQuery => (
                    StatusCode::BAD_REQUEST,
                    String::from("The index of the requested image must be at least 1."),
                ),
                Self::TooHighQuery { max } => (
                    StatusCode::BAD_REQUEST,
                    {
                        if max > 1 {
                            format!("The index of the requested image is too high; there are {max} images in this collection.")
                        } else {
                            String::from("The index of the requested image is too high; there is 1 image in this collection.")
                        }
                    }
                    
                ),
                Self::Internal => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    String::from("An internal server error occurred."),
                ),
            }
        }
        .into_response()
    }
}
