#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

use axum::{routing::get, Router};
use pixum::AppState;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .with_state(Arc::new(AppState::new()));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
