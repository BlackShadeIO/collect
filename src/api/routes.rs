//! REST API endpoint handlers.

use std::path::Path;

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::api::server::AppState;
use crate::types::gamma::current_epoch;

/// GET /health — no auth required
pub async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let uptime = state.started_at.elapsed().as_secs();
    let orch = state.orch_state_rx.borrow().clone();

    Json(serde_json::json!({
        "status": "ok",
        "uptime_secs": uptime,
        "current_epoch": current_epoch(),
        "active_processes": orch.slots.len(),
    }))
}

/// GET /status — auth required
pub async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let orch = state.orch_state_rx.borrow().clone();
    let indicator = state.indicator_rx.borrow().clone();

    Json(serde_json::json!({
        "orchestrator": orch,
        "latest_indicator": indicator,
    }))
}

/// GET /stats — auth required
pub async fn stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let data_dir = &state.config.data_dir;

    let mut total_files = 0u64;
    let mut total_size = 0u64;
    let mut categories: std::collections::HashMap<String, CategoryStats> = std::collections::HashMap::new();

    if let Ok(mut epochs) = tokio::fs::read_dir(data_dir).await {
        while let Ok(Some(entry)) = epochs.next_entry().await {
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                if let Ok(mut files) = tokio::fs::read_dir(entry.path()).await {
                    while let Ok(Some(file_entry)) = files.next_entry().await {
                        if let Ok(meta) = file_entry.metadata().await {
                            let name = file_entry.file_name().to_string_lossy().to_string();
                            let source = name.trim_end_matches(".jsonl").to_string();
                            let size = meta.len();

                            total_files += 1;
                            total_size += size;

                            let cat = categories.entry(source).or_default();
                            cat.file_count += 1;
                            cat.total_size += size;
                        }
                    }
                }
            }
        }
    }

    Json(serde_json::json!({
        "uptime_secs": state.started_at.elapsed().as_secs(),
        "total_files": total_files,
        "total_size_bytes": total_size,
        "categories": categories,
    }))
    .pipe_ok()
}

#[derive(Default, Serialize)]
struct CategoryStats {
    file_count: u64,
    total_size: u64,
}

trait PipeOk: Sized {
    fn pipe_ok(self) -> Result<Self, StatusCode> {
        Ok(self)
    }
}
impl<T> PipeOk for T {}

/// GET /market — list all epochs with data
pub async fn list_markets(State(state): State<AppState>) -> Json<serde_json::Value> {
    let data_dir = &state.config.data_dir;
    let mut markets = Vec::new();

    if let Ok(mut epochs) = tokio::fs::read_dir(data_dir).await {
        while let Ok(Some(entry)) = epochs.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(epoch) = name.parse::<u64>() {
                let mut files = Vec::new();
                let mut total_size = 0u64;

                if let Ok(mut dir) = tokio::fs::read_dir(entry.path()).await {
                    while let Ok(Some(f)) = dir.next_entry().await {
                        if let Ok(meta) = f.metadata().await {
                            let size = meta.len();
                            total_size += size;
                            files.push(serde_json::json!({
                                "name": f.file_name().to_string_lossy(),
                                "size_bytes": size,
                            }));
                        }
                    }
                }

                markets.push(serde_json::json!({
                    "epoch": epoch,
                    "slug": format!("btc-updown-5m-{epoch}"),
                    "total_size_bytes": total_size,
                    "files": files,
                }));
            }
        }
    }

    markets.sort_by_key(|m| m["epoch"].as_u64().unwrap_or(0));
    Json(serde_json::json!({ "markets": markets }))
}

/// GET /market/{epoch} — specific market details
pub async fn get_market(
    State(state): State<AppState>,
    AxumPath(epoch): AxumPath<u64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let dir = state.config.data_dir.join(epoch.to_string());
    if !dir.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut files = Vec::new();
    let mut total_size = 0u64;
    let mut total_lines = 0u64;

    if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(meta) = entry.metadata().await {
                let size = meta.len();
                let path = entry.path();
                let line_count = count_lines(&path).await.unwrap_or(0);
                total_size += size;
                total_lines += line_count;
                files.push(serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "size_bytes": size,
                    "line_count": line_count,
                }));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "epoch": epoch,
        "slug": format!("btc-updown-5m-{epoch}"),
        "total_size_bytes": total_size,
        "total_lines": total_lines,
        "files": files,
    })))
}

async fn count_lines(path: &Path) -> std::io::Result<u64> {
    let content = tokio::fs::read(path).await?;
    Ok(content.iter().filter(|&&b| b == b'\n').count() as u64)
}

/// GET /download?epoch=X&category=Y
#[derive(Deserialize)]
pub struct DownloadParams {
    pub epoch: Option<u64>,
    pub category: Option<String>,
}

pub async fn download(
    State(state): State<AppState>,
    Query(params): Query<DownloadParams>,
) -> Result<impl IntoResponse, StatusCode> {
    let data_dir = &state.config.data_dir;

    // Collect matching files
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();

    if let Some(epoch) = params.epoch {
        let dir = data_dir.join(epoch.to_string());
        if dir.exists() {
            collect_files_from_dir(&dir, &params.category, &mut file_paths).await;
        }
    } else {
        // All epochs
        if let Ok(mut epochs) = tokio::fs::read_dir(data_dir).await {
            while let Ok(Some(entry)) = epochs.next_entry().await {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    collect_files_from_dir(&entry.path(), &params.category, &mut file_paths).await;
                }
            }
        }
    }

    if file_paths.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Concatenate all JSONL files into a single response
    let mut output = Vec::new();
    for path in &file_paths {
        if let Ok(content) = tokio::fs::read(path).await {
            output.extend_from_slice(&content);
        }
    }

    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        output,
    ))
}

async fn collect_files_from_dir(
    dir: &Path,
    category: &Option<String>,
    out: &mut Vec<std::path::PathBuf>,
) {
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".jsonl") {
                continue;
            }
            if let Some(cat) = category {
                let source = name.trim_end_matches(".jsonl");
                if source != cat.as_str() {
                    continue;
                }
            }
            out.push(entry.path());
        }
    }
}
