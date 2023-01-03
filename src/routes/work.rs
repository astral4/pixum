use crate::{AppError, AppResult, AppState};
use axum::extract::{Path, State};
use reqwest::Client;
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
pub async fn work(
    Path(work_id): Path<u32>,
    State(state): State<Arc<AppState>>,
) -> AppResult<String> {
    let data = fetch_work_info(&state.client, work_id).await?;

    Ok(data.urls.original)
}

async fn fetch_work_info(client: &Client, work_id: u32) -> AppResult<WorkInfo> {
    let response: QueryResponse = client
        .get(format!("https://www.pixiv.net/ajax/illust/{work_id}"))
        .send()
        .await
        .map_err(|e| AppError::Internal { msg: e.to_string() })?
        .json()
        .await
        .map_err(|_| AppError::ServerUnreachable)?;

    if response.error {
        return Err(AppError::ArtworkUnavailable {
            msg: response.message,
        });
    }

    if let BodyData::Success(data) = response.body {
        Ok(data)
    } else {
        Err(AppError::ArtworkUnavailable {
            msg: response.message,
        })
    }
}
