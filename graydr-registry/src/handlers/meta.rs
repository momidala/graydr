use std::sync::Arc;
use axum::{extract::{Path, State}, Json};
use crate::{AppError, AppState, model::{ModuleCoord, ModuleMeta}};

pub async fn handle(
    Path((org, name, version_str)): Path<(String, String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModuleMeta>, AppError> {
    let version = semver::Version::parse(&version_str)
        .map_err(|_| AppError::InvalidSemVer(version_str.clone()))?;
    let coord = ModuleCoord { org, name, version };
    let meta = state.store.get_meta(&coord).await
        .map_err(|e| match e {
            crate::store::StoreError::NotFound => AppError::NotFound,
            other => AppError::Internal(other),
        })?;
    Ok(Json(meta))
}
