use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::Deserialize;
use std::sync::Arc;
use crate::{AppError, AppState, model::{ModuleCoord, LifecycleState}};

#[derive(Deserialize)]
pub struct LifecyclePatch {
    pub lifecycle: LifecycleState,
}

pub async fn handle(
    Path((org, name, version_str)): Path<(String, String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<LifecyclePatch>,
) -> Result<StatusCode, AppError> {
    let version = semver::Version::parse(&version_str)
        .map_err(|_| AppError::InvalidSemVer(version_str.clone()))?;
    let coord = ModuleCoord { org, name, version };
    state.store.update_lifecycle(&coord, body.lifecycle).await
        .map_err(|e| match e {
            crate::store::StoreError::NotFound => AppError::NotFound,
            crate::store::StoreError::InvalidTransition => AppError::InvalidLifecycleTransition,
            other => AppError::Internal(other),
        })?;
    Ok(StatusCode::OK)
}
