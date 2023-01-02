use crate::AppState;
use axum::extract::{Path, State};
use std::sync::Arc;

pub async fn work(Path(work_id): Path<u32>, State(state): State<Arc<AppState>>) {
    todo!();
}
