use crate::{AppError, AppResult, AppState};
use ahash::HashMap;
use axum::extract::{Path, State};
use bytes::Bytes;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct OtherWorkInfo {
    url: String,
}

#[derive(Deserialize)]
struct WorkUrls {
    original: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkInfo {
    urls: WorkUrls,
    user_illusts: HashMap<String, Option<OtherWorkInfo>>,
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
pub async fn info(
    Path(work_id): Path<u32>,
    State(state): State<Arc<AppState>>,
) -> AppResult<String> {
    let data = fetch_work_info(&state.client, work_id).await?;

    if let Some(link) = data.urls.original {
        Ok(link)
    } else {
        Ok(String::from("No link found"))
    }
}

/// # Errors
/// This function fails if:
/// - HTTP request to Pixiv's API fails
/// - Server returns an HTTP error
/// - Data of the work is unavailable
// TODO: add response headers (content-type, etc)
pub async fn source(
    Path((work_id, index)): Path<(u32, u8)>,
    State(state): State<Arc<AppState>>,
) -> AppResult<Bytes> {
    let data = fetch_work_info(&state.client, work_id).await?;

    if let Some(link) = data.urls.original {
        println!("Found original link");
        fetch_image_data(&state.client, &link, work_id, index).await
    } else {
        // TODO: Fix this
        println!("Did not find original link");
        let target_link = data
            .user_illusts
            .get(&work_id.to_string())
            .ok_or_else(|| AppError::ArtworkUnavailable { msg: String::new() })?
            .as_ref()
            .ok_or_else(|| AppError::ArtworkUnavailable { msg: String::new() })?
            .url
            .replace("c/250x250_80_a2/img-master", "img-original")
            .replace("c/250x250_80_a2/custom-thumb", "img-original")
            .replace("_square1200", "")
            .replace("_custom1200", "");

        println!("Might be {target_link}");
        fetch_image_data(&state.client, &target_link, work_id, index).await
    }
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

async fn fetch_image_data(client: &Client, url: &str, work_id: u32, index: u8) -> AppResult<Bytes> {
    let target_link = url.replace(
        format!("{work_id}_p0").as_str(),
        format!("{work_id}_p{}", index - 1).as_str(),
    );

    let data = client
        .get(target_link)
        .header(
            "Referer",
            format!("https://www.pixiv.net/member_illust.php?mode=medium&illust_id={work_id}"),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal { msg: e.to_string() })?
        .bytes()
        .await
        .map_err(|_| AppError::ServerUnreachable)?;

    Ok(data)
}
