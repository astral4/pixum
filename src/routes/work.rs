use crate::AppState;
use anyhow::{anyhow, ensure, Result};
use axum::extract::{Path, State};
use axum_macros::debug_handler;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct WorkUrls {
    original: String,
}

#[derive(Deserialize)]
struct WorkInfo {
    urls: WorkUrls,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BodyData {
    Error(Vec<()>),
    Success(WorkInfo),
}

#[derive(Deserialize)]
struct QueryResponse {
    error: bool,
    message: String,
    body: BodyData,
}

/// # Errors
/// This function fails if:
/// - HTTP request to Pixiv's API fails
/// - Server returns an HTTP error
/// - Data of the work is unavailable
#[debug_handler]
pub async fn work(Path(work_id): Path<u32>, State(state): State<Arc<AppState>>) -> Result<String> {
    let response: QueryResponse = state
        .client
        .get(format!("https://www.pixiv.net/ajax/illust/{work_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    ensure!(!response.error, response.message);

    if let BodyData::Success(data) = response.body {
        Ok(data.urls.original)
    } else {
        Err(anyhow!(
            "Information of the requested work could not be retrieved"
        ))
    }
}
