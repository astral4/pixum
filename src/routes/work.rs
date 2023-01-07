use crate::{AppError, AppResult, AppState};
use ahash::HashMap;
use axum::{
    extract::{rejection::PathRejection, Path, State},
    http::header,
    response::IntoResponse,
};
use bytes::Bytes;
use deadpool_redis::{redis::Cmd, Connection};
use futures::future::join_all;
use mime_guess::Mime;
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use std::{path::Path as StdPath, sync::Arc};

type PathResult<T> = Result<Path<T>, PathRejection>;

const NUM_SECONDS_IN_ONE_YEAR: usize = 60 * 60 * 24 * 365;

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
    #[serde(rename = "sl")]
    num_entries: u8,
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
    body: BodyData,
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
) -> AppResult<String> {
    if let Ok(id) = work_id {
        let data = fetch_work_info(&state.client, id.0).await?;

        if let Some(link) = data.urls.original {
            Ok(link)
        } else {
            Ok(String::from("No link found"))
        }
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
    work_info: PathResult<(u32, u8)>,
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
    index: u8,
) -> AppResult<impl IntoResponse> {
    if index == 0 {
        return Err(AppError::ZeroQuery);
    }

    let image_data;
    let mime_type;

    // Checks if the requested image's URL is already cached
    if let Ok(url) = Cmd::get(format!("{work_id}_{index}"))
        .query_async::<_, String>(connection)
        .await
    {
        image_data =
            fetch_image_data(client, connection, &url, work_id, index, true, false).await?;
        mime_type = mime_guess::from_path(url).first_or_octet_stream();

        return Ok((
            generate_http_headers(&format!("{work_id}-{index}"), &mime_type),
            image_data,
        ));
    }

    let data = fetch_work_info(client, work_id).await?;

    // The value of num_entries is 1 more than the actual number of images
    if index > data.num_entries - 1 {
        return Err(AppError::TooHighQuery {
            max: data.num_entries - 1,
        });
    }

    if let Some(link) = data.urls.original {
        image_data =
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

        image_data = fetch_image_data(
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

    Ok((
        generate_http_headers(&format!("{work_id}-{index}"), &mime_type),
        image_data,
    ))
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

async fn fetch_image_data(
    client: &Client,
    connection: &mut Connection,
    url: &str,
    work_id: u32,
    index: u8,
    url_known: bool,
    update_cache: bool,
) -> AppResult<Bytes> {
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
                Cmd::set_ex(format!("{work_id}_{index}"), url, NUM_SECONDS_IN_ONE_YEAR)
                    .query_async::<_, ()>(connection)
                    .await;
            }

            return Ok(data);
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
            Cmd::set_ex(format!("{work_id}_{index}"), link, NUM_SECONDS_IN_ONE_YEAR)
                .query_async::<_, ()>(connection)
                .await;
        }

        Ok(data)
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
        _ => Err(AppError::ArtworkUnavailable),
    }
}

fn generate_http_headers(filename: &str, mime: &Mime) -> [(header::HeaderName, String); 5] {
    [
        (header::ACCESS_CONTROL_ALLOW_HEADERS, String::from("GET")),
        (
            header::CONTENT_DISPOSITION,
            format!(
                r#"inline; filename="{filename}{}""#,
                mime.suffix()
                    .map_or_else(String::new, |ext| format!(".{ext}"))
            ),
        ),
        (header::CONTENT_TYPE, mime.to_string()),
        (
            header::STRICT_TRANSPORT_SECURITY,
            String::from("max-age=63072000; includeSubDomains; preload"),
        ),
        (header::X_CONTENT_TYPE_OPTIONS, String::from("nosniff")),
    ]
}
