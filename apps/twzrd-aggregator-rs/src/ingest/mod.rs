use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

#[derive(Debug, Deserialize, Serialize)]
pub struct IngestEvent {
    pub ts: Option<String>,
    pub signature: Option<String>,
    pub slot: Option<u64>,
    pub name: Option<String>,
    pub data: serde_json::Value,
}

pub async fn ingest_handler(State(_state): State<AppState>, Json(body): Json<IngestEvent>) -> (StatusCode, Json<serde_json::Value>) {
    // TODO: validate, map to domain rows, enqueue or insert
    // For bootstrap, just accept and return 202
    increment_counter!("ingest_accepted_total");
    let ok = serde_json::json!({ "ok": true, "accepted": body.name });
    (StatusCode::ACCEPTED, Json(ok))
}
