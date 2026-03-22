use crate::providers::{self, ProviderError};
use crate::routes::health::HealthMap;
use crate::routes::store::RouteStore;
use crate::types::{ProviderResponse, SanitizedRequest};

/// Dispatch with fallback chain support.
/// Tries the primary route, then each fallback in order.
pub async fn dispatch_with_fallback(
    client: &reqwest::Client,
    route_store: &RouteStore,
    health_map: &HealthMap,
    request: &SanitizedRequest,
) -> Result<ProviderResponse, ProviderError> {
    let primary = route_store
        .get_route(&request.route_key)
        .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?
        .ok_or_else(|| ProviderError::InvalidResponse(format!("route '{}' not found", request.route_key)))?;

    if !primary.active {
        return Err(ProviderError::InvalidResponse(format!(
            "route '{}' is inactive",
            request.route_key
        )));
    }

    // Collect all routes to try: primary + fallbacks
    let mut routes_to_try = vec![primary.clone()];
    for fallback_key in &primary.fallback_chain {
        if let Ok(Some(fb_route)) = route_store.get_route(fallback_key) {
            if fb_route.active {
                routes_to_try.push(fb_route);
            }
        }
    }

    let mut last_error = None;

    for (attempt, route) in routes_to_try.iter().enumerate() {
        // Check health status
        if !crate::routes::health::is_route_healthy(health_map, &route.route_key) {
            tracing::info!(
                route = %route.route_key,
                attempt = attempt,
                "skipping unhealthy route"
            );
            last_error = Some(ProviderError::ServerError {
                status: 503,
                body: format!("route '{}' is unhealthy", route.route_key),
            });
            continue;
        }

        match providers::dispatch(client, route, request).await {
            Ok(mut resp) => {
                resp.fallback_attempt = attempt as u8;
                resp.route_key = route.route_key.clone();
                return Ok(resp);
            }
            Err(e) => {
                tracing::warn!(
                    route = %route.route_key,
                    attempt = attempt,
                    error = %e,
                    "dispatch failed, trying next"
                );
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or(ProviderError::InvalidResponse("no routes available".into())))
}
