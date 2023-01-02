#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

use axum::{http::StatusCode, routing::get, Router};
use pixum::{work, AppState};
use std::sync::Arc;

async fn handle_anyhow_error(err: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

#[tokio::main]
async fn main() {
    let shared_state = Arc::new(AppState::new());

    let app = Router::new()
        .route("/", get(|| async { "Welcome to Pixum" }))
        .route("/:work_id", get(work).with_state(shared_state))
        .with_state(shared_state);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
