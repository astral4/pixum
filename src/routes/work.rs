use crate::{AppError, AppResult, AppState};
use ahash::HashMap;
use axum::http::header::{self, HeaderValue};
use axum::{
    extract::{rejection::PathRejection, Path, State},
    response::{IntoResponse, Response as AxumResponse},
    Json,
};
use bytes::Bytes;
use deadpool_redis::{redis::Cmd, Connection};
use futures::future::join_all;
use mime_guess::Mime;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::{path::Path as StdPath, sync::Arc};

type PathResult<T> = Result<Path<T>, PathRejection>;

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
    illust_title: String,
    upload_date: String,
    #[serde(rename = "sl")]
    num_entries: u16,
    urls: WorkUrls,
    user_id: String,
    user_name: String,
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
    body: BodyData,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    artist_name: String,
    artist_id: Option<u32>,
    work_id: u32,
    title: String,
    upload_time: String,
    length: u16,
}

/// # Errors
/// This function fails if:
/// - `work_id` is invalid
/// - HTTP request to Pixiv's API fails
/// - Server returns an HTTP error
/// - Data of the work is unavailable
pub async fn info(
    work_id: PathResult<u32>,
    State(state): State<Arc<AppState>>,
) -> AppResult<AxumResponse> {
    if let Ok(id) = work_id {
        let data = fetch_work_info(&state.client, id.0).await?;

        let mut response = Json(InfoResponse {
            artist_name: data.user_name,
            artist_id: data.user_id.parse().ok(),
            work_id: id.0,
            title: data.illust_title,
            upload_time: data.upload_date,
            // The value of num_entries is 1 more than the actual number of images
            length: data.num_entries - 1,
        })
        .into_response();

        // Axum's `Json` handler only sets the Content-Type header to "application/json".
        // JSON is supposed to be interpreted as UTF-8 by default (see https://www.rfc-editor.org/rfc/rfc8259#section-8.1),
        // but Safari does not obey this, so the header is adjusted for cross-browser compatibility.
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );

        Ok(response)
    } else {
        Err(AppError::InvalidUrl)
    }
}

/// # Errors
/// This function fails if:
/// - `work_id` or `index` are invalid
/// - Database connection fails
/// - HTTP request to Pixiv's API fails
/// - Server returns an HTTP error
/// - Data of the work is unavailable
pub async fn source(
    work_info: PathResult<(u32, u16)>,
    State(state): State<Arc<AppState>>,
) -> AppResult<impl IntoResponse> {
    if let Ok(Path((work_id, index))) = work_info {
        // The application can work without a database, but a connection error
        // indicates something is wrong, so the server will immediately return an error.
        let mut connection = state.pool.get().await.map_err(|_| AppError::Internal)?;
        get_image_data(&state.client, &mut connection, work_id, index).await
    } else {
        Err(AppError::InvalidUrl)
    }
}

async fn get_image_data(
    client: &Client,
    connection: &mut Connection,
    work_id: u32,
    index: u16,
) -> AppResult<impl IntoResponse> {
    if index == 0 {
        return Err(AppError::ZeroQuery);
    }

    let file_name;
    let image_data;
    let mime_type;

    {
        let cache_entry_name = format!("{work_id}_{index}");

        // Checks if the requested image's URL is already cached
        if let Ok(url) = Cmd::get(&cache_entry_name)
            .query_async::<_, String>(connection)
            .await
        {
            return match fetch_image_data(client, connection, &url, work_id, index, true, false)
                .await
            {
                Ok((file_name, image_data)) => {
                    mime_type = mime_guess::from_path(url).first_or_octet_stream();
                    Ok((generate_http_headers(&file_name, &mime_type), image_data))
                }
                Err(err) => {
                    #[allow(unused_must_use)]
                    if let AppError::WrongArtworkUrl = err {
                        Cmd::unlink(cache_entry_name)
                            .query_async::<_, ()>(connection)
                            .await;
                    }
                    Err(err)
                }
            };
        }
    }

    let data = fetch_work_info(client, work_id).await?;

    // The value of num_entries is 1 more than the actual number of images
    if index > data.num_entries - 1 {
        return Err(AppError::TooHighQuery {
            max: data.num_entries - 1,
        });
    }

    if let Some(link) = data.urls.original {
        (file_name, image_data) =
            fetch_image_data(client, connection, &link, work_id, index, true, true).await?;
        mime_type = mime_guess::from_path(link).first_or_octet_stream();
    } else {
        // Original image URLs on Pixiv follow a certain pattern.
        // If the master/thumbnail image URL is present, the original link can be obtained.
        let target_link = data
            .user_illusts
            .get(&work_id.to_string())
            .ok_or(AppError::ArtworkUnavailable)?
            .as_ref()
            .ok_or(AppError::ArtworkUnavailable)?
            .url
            .replace("c/250x250_80_a2/img-master", "img-original")
            .replace("c/250x250_80_a2/custom-thumb", "img-original")
            .replace("_square1200", "")
            .replace("_custom1200", "");

        (file_name, image_data) = fetch_image_data(
            client,
            connection,
            &target_link,
            work_id,
            index,
            false,
            true,
        )
        .await?;
        mime_type = mime_guess::from_path(target_link).first_or_octet_stream();
    }

    Ok((generate_http_headers(&file_name, &mime_type), image_data))
}

async fn fetch_work_info(client: &Client, work_id: u32) -> AppResult<WorkInfo> {
    let response: QueryResponse = client
        .get(format!("https://www.pixiv.net/ajax/illust/{work_id}"))
        .send()
        .await
        .map_err(|_| AppError::Internal)?
        .json()
        .await
        .map_err(|_| AppError::ServerUnreachable)?;

    if response.error {
        return Err(AppError::ArtworkUnavailable);
    }

    if let BodyData::Success(data) = response.body {
        Ok(data)
    } else {
        Err(AppError::ArtworkUnavailable)
    }
}

fn get_image_name_from_url(url: &str, fallback: String) -> String {
    url.split('/')
        .next_back()
        .map_or_else(|| fallback, ToString::to_string)
}

async fn fetch_image_data(
    client: &Client,
    connection: &mut Connection,
    url: &str,
    work_id: u32,
    index: u16,
    url_known: bool,
    update_cache: bool,
) -> AppResult<(String, Bytes)> {
    let referer_string =
        format!("https://www.pixiv.net/member_illust.php?mode=medium&illust_id={work_id}");

    if url_known {
        if let Ok(data) = fetch_image(client, url.to_string(), &referer_string)
            .await
            .map_err(|_| AppError::Internal)?
            .bytes()
            .await
        {
            #[allow(unused_must_use)]
            if update_cache {
                Cmd::set(format!("{work_id}_{index}"), url)
                    .query_async::<_, ()>(connection)
                    .await;
            }

            return Ok((
                get_image_name_from_url(url, format!("{work_id}_p{}", index - 1)),
                data,
            ));
        }
        return Err(AppError::Internal);
    }

    // Only the link for the first image in a collection is given,
    // so links to other images in the collection must be derived
    let target_link = url.replace(
        format!("{work_id}_p0").as_str(),
        format!("{work_id}_p{}", index - 1).as_str(),
    );

    // Pixiv images are either JPG, PNG, or GIF. The correct extension is
    // not known at first because the original URL is not provided.
    // Links with all three file extensions are tested.
    // If a valid link exists, it is cached and the image there is returned.
    if let Some(response) = join_all(["jpg", "png", "gif"].into_iter().map(|ext| {
        let link = StdPath::new(&target_link)
            .with_extension(ext)
            .to_string_lossy()
            .to_string();

        fetch_image(client, link, &referer_string)
    }))
    .await
    .into_iter()
    .find_map(Result::ok)
    {
        let link = response.url().as_str().to_string();
        let data = response.bytes().await.map_err(|_| AppError::Internal)?;

        #[allow(unused_must_use)]
        {
            Cmd::set(format!("{work_id}_{index}"), &link)
                .query_async::<_, ()>(connection)
                .await;
        }

        Ok((
            get_image_name_from_url(&link, format!("{work_id}_p{}", index - 1)),
            data,
        ))
    } else {
        Err(AppError::ArtworkUnavailable)
    }
}

async fn fetch_image(client: &Client, url: String, referer: &str) -> AppResult<Response> {
    let response = client
        .get(url)
        // Including this Referer header is important because the response will return 403 Forbidden otherwise
        .header("Referer", referer)
        .send()
        .await
        .map_err(|_| AppError::Internal)?;

    match response.status() {
        StatusCode::OK => Ok(response),
        StatusCode::NOT_FOUND => Err(AppError::WrongArtworkUrl),
        _ => Err(AppError::ArtworkUnavailable),
    }
}

fn generate_http_headers(filename: &str, mime: &Mime) -> [(header::HeaderName, String); 3] {
    [
        (
            header::CONTENT_DISPOSITION,
            format!(r#"inline; filename="{filename}""#),
        ),
        (header::CONTENT_TYPE, mime.to_string()),
        (
            header::CACHE_CONTROL,
            String::from("max-age=31536000, public, immutable, no-transform"),
        ),
    ]
}
