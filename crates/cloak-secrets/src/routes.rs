use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use cloak_core::CloakError;

use crate::cache::SecretCache;

pub fn routes() -> Router<SecretCache> {
    Router::new().route("/cloak/secrets/{key}", get(get_secret))
}

async fn get_secret(
    State(cache): State<SecretCache>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, CloakError> {
    let value = cache.get(&key).await?;
    Ok(Json(serde_json::json!({
        "key": key,
        "value": value,
    })))
}
