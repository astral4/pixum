#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

use axum::{routing::get, Router, Server};
use pixum::{work, AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let shared_state = Arc::new(AppState::new());

    let app = Router::new()
        .route("/", get(|| async { "Welcome to Pixum" }))
        .route("/:work_id", get(work).with_state(shared_state.clone()))
        .with_state(shared_state);

    Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
