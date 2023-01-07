#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

use axum::{error_handling::HandleErrorLayer, http::StatusCode, routing::get, Router, Server};
use pixum::{work, AppState};
use std::{sync::Arc, time::Duration};
use tower::ServiceBuilder;

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
        .fallback(fallback)
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
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|_| async {
                    StatusCode::TOO_MANY_REQUESTS
                }))
                .buffer(50)
                .rate_limit(50, Duration::from_secs(10))
                .layer(HandleErrorLayer::new(|_| async {
                    StatusCode::REQUEST_TIMEOUT
                }))
                .timeout(Duration::from_secs(10))
                .concurrency_limit(100),
        );

    Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
