use std::sync::Arc;
use axum::{extract::{Path, State}, response::IntoResponse, http::StatusCode};
use crate::{AppError, AppState, model::{ModuleCoord, LifecycleState}};

pub async fn handle(
    Path((org, name, version_str)): Path<(String, String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let version = semver::Version::parse(&version_str)
        .map_err(|_| AppError::InvalidSemVer(version_str.clone()))?;
    let coord = ModuleCoord { org, name, version };
    // Retirement gate: check lifecycle BEFORE serving bytes (server-side enforcement)
    let meta = state.store.get_meta(&coord).await
        .map_err(|e| match e {
            crate::store::StoreError::NotFound => AppError::NotFound,
            other => AppError::Internal(other),
        })?;
    if meta.lifecycle == LifecycleState::Retired {
        return Err(AppError::Retired);
    }
    let content = state.store.get_content(&coord).await
        .map_err(|e| match e {
            crate::store::StoreError::NotFound => AppError::NotFound,
            other => AppError::Internal(other),
        })?;
    Ok((StatusCode::OK, content))
}
