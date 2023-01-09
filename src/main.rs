#![warn(clippy::all, clippy::pedantic)]
#![forbid(unsafe_code)]

use axum::http::header::{self, HeaderName, HeaderValue};
use axum::{error_handling::HandleErrorLayer, http::StatusCode, routing::get, Router, Server};
use axum_extra::routing::RouterExt;
use pixum::{work, AppState};
use std::{sync::Arc, time::Duration};
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;

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
        .route_with_tsr(
            "/:work_id",
            get(work::info).with_state(shared_state.clone()),
        )
        .route_with_tsr(
            "/:work_id/:index",
            get(work::source).with_state(shared_state.clone()),
        )
        .with_state(shared_state)
        .layer(
            ServiceBuilder::new()
                .override_response_header(
                    header::STRICT_TRANSPORT_SECURITY,
                    HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
                )
                .insert_response_header_if_not_present(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_static("GET"),
                )
                .override_response_header(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static(
                        "default-src 'none'; frame-ancestors 'none'; upgrade-insecure-requests;",
                    ),
                )
                .override_response_header(
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                )
                .override_response_header(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"))
                .override_response_header(
                    HeaderName::from_static("x-robots-tag"),
                    HeaderValue::from_static("noindex"),
                )
                .compression()
                .layer(HandleErrorLayer::new(|_| async {
                    StatusCode::TOO_MANY_REQUESTS
                }))
                // Tower's rate-limiting middleware does not implement Clone
                // (required by HandleErrorLayer)
                // so a buffer middleware is also used.
                .buffer(100)
                .rate_limit(50, Duration::from_secs(10))
                .timeout(Duration::from_secs(15))
                .concurrency_limit(100),
        );

    Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
