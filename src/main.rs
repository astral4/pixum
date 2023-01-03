#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

use axum::{http::StatusCode, routing::get, Router, Server};
use pixum::{work, AppState};
use std::sync::Arc;

#[allow(clippy::unused_async)]
async fn fallback() -> (StatusCode, String) {
    (
        StatusCode::NOT_FOUND,
        String::from("The requested URL is invalid."),
    )
}

#[tokio::main]
async fn main() {
    let shared_state = Arc::new(AppState::new());

    let app = Router::new()
        .route("/", get(|| async { "Welcome to Pixum" }))
        .route(
            "/:work_id",
            get(work::info).with_state(shared_state.clone()),
        )
        .route(
            "/:work_id/:index",
            get(work::source).with_state(shared_state.clone()),
        )
        .with_state(shared_state)
        .fallback(fallback);

    Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
