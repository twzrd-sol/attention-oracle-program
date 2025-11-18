use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use crate::state::AppState;

pub async fn health() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

pub async fn not_implemented() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({ "ok": false, "error": "not_implemented" })))
}

pub async fn metrics(State(state): State<AppState>) -> (StatusCode, String) {
    let body = state.metrics.render();
    (StatusCode::OK, body)
}
