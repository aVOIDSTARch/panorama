use axum::{middleware, routing, Router};
use tower_http::trace::TraceLayer;

use datastore::config::DatastoreConfig;
use datastore::routes;
use datastore::state::AppState;

#[tokio::main]
async fn main() {
    panorama_logging::init("datastore", Some("data/panorama_logs.db"));

    let config = DatastoreConfig::from_env().expect("failed to load config");
    let port = config.port;

    let state = AppState::init(config)
        .await
        .expect("failed to initialize Datastore");

    // Authenticated routes
    let api = Router::new()
        // Schema
        .route("/schema", routing::get(routes::list_tables))
        .route("/schema", routing::post(routes::create_table))
        // Object CRUD
        .route("/objects/:table", routing::get(routes::list_objects))
        .route("/objects/:table", routing::post(routes::insert_object))
        .route("/objects/:table/:id", routing::get(routes::get_object))
        .route(
            "/objects/:table/:id",
            routing::delete(routes::delete_object),
        )
        // Queries
        .route("/queries", routing::post(routes::execute_query))
        // Blobs
        .route("/blobs/:namespace", routing::post(routes::upload_blob))
        .route(
            "/blobs/:namespace/:blob_id",
            routing::get(routes::get_blob),
        )
        .route(
            "/blobs/:namespace/:blob_id",
            routing::delete(routes::delete_blob),
        )
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::cloak_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::halt_guard,
        ));

    // Health (no auth)
    let health = Router::new().route("/health", routing::get(routes::health_handler));

    let app = Router::new()
        .merge(health)
        .merge(api)
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Datastore listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}
