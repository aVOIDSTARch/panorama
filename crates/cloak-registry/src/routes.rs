use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use cloak_core::{CloakError, RegistrationRequest, RegistrationResponse};

use crate::sse;
use crate::store::ServiceStore;

/// Router state for registry routes.
#[derive(Clone)]
pub struct RegistryRouterState {
    pub store: ServiceStore,
    pub server_port: u16,
}

pub fn routes() -> Router<RegistryRouterState> {
    Router::new()
        .route("/cloak/services/register", post(register_service))
        .route(
            "/cloak/services/{service_id}/halt-stream",
            get(halt_stream),
        )
        .route("/cloak/services", get(list_services))
}

async fn register_service(
    State(state): State<RegistryRouterState>,
    Json(req): Json<RegistrationRequest>,
) -> Result<Json<RegistrationResponse>, CloakError> {
    let (resp, _signing_key) =
        crate::registration::handle_register(&state.store, req, state.server_port)?;
    Ok(Json(resp))
}

async fn halt_stream(
    State(state): State<RegistryRouterState>,
    Path(service_id): Path<String>,
) -> Result<
    axum::response::sse::Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    CloakError,
> {
    sse::halt_stream(&state.store, &service_id)
        .ok_or_else(|| CloakError::ServiceNotRegistered(service_id))
}

async fn list_services(
    State(state): State<RegistryRouterState>,
) -> Json<Vec<String>> {
    Json(state.store.list_ids())
}
