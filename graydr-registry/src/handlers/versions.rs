use std::sync::Arc;
use axum::{extract::{Path, State}, Json};
use crate::{AppError, AppState, model::VersionEntry};

pub async fn handle(
    Path((org, name)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<VersionEntry>>, AppError> {
    let versions = state.store.list_versions(&org, &name).await
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(versions))
}
