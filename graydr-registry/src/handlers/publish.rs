use std::sync::Arc;
use axum::{extract::{Multipart, Path, State}, http::StatusCode};
use crate::{AppError, AppState, model::{ModuleCoord, ModuleMeta}};

fn validate_segment(s: &str, field: &str) -> Result<(), AppError> {
    if s.is_empty() || s.contains("..") || s.contains('/') || s.contains('\\') {
        return Err(AppError::InvalidPathSegment(format!("{} contains invalid characters", field)));
    }
    Ok(())
}

pub async fn handle(
    Path((org, name, version_str)): Path<(String, String, String)>,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<StatusCode, AppError> {
    validate_segment(&org, "org")?;
    validate_segment(&name, "name")?;

    let version = semver::Version::parse(&version_str)
        .map_err(|_| AppError::InvalidSemVer(version_str.clone()))?;

    // Reject build metadata (e.g. 1.0.0+build.123) — no semantic meaning for dependency resolution
    if !version.build.is_empty() {
        return Err(AppError::InvalidSemVer(format!("build metadata not allowed: {}", version_str)));
    }

    let coord = ModuleCoord { org: org.clone(), name: name.clone(), version };

    let mut module_bytes: Option<bytes::Bytes> = None;
    while let Some(field) = multipart.next_field().await
        .map_err(|e| AppError::BadRequest(e.to_string()))? {
        if field.name() == Some("module") {
            module_bytes = Some(field.bytes().await
                .map_err(|e| AppError::BadRequest(e.to_string()))?);
        }
    }
    let content = module_bytes.ok_or(AppError::MissingModuleField)?;

    let meta = ModuleMeta {
        org,
        name,
        version: version_str,
        lifecycle: crate::model::LifecycleState::Active,
        published_at: chrono_free_now(),
    };

    state.store.put_module(&coord, content, &meta).await
        .map_err(|e| match e {
            crate::store::StoreError::AlreadyExists => AppError::AlreadyExists,
            other => AppError::Internal(other),
        })?;

    Ok(StatusCode::OK)
}

/// Returns current UTC time as ISO 8601 string without pulling in chrono.
fn chrono_free_now() -> String {
    // Use std::time — no chrono dependency needed for a simple timestamp
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as rough ISO 8601 — sufficient for meta.json; not used for sorting
    let (y, mo, d, h, mi, s) = unix_to_ymd_hms(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

fn unix_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Simplified Gregorian calendar calculation
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days_in_month = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u64;
    for &dim in &days_in_month {
        if remaining < dim { break; }
        remaining -= dim;
        mo += 1;
    }
    (y, mo, remaining + 1, h, m, s)
}
