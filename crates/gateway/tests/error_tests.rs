use axum::http::StatusCode;
use axum::response::IntoResponse;
use gateway::error::GatewayApiError;

fn status_of(err: GatewayApiError) -> StatusCode {
    err.into_response().status()
}

#[test]
fn bad_request_is_400() {
    assert_eq!(
        status_of(GatewayApiError::BadRequest("test".into())),
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn unauthorized_is_401() {
    assert_eq!(
        status_of(GatewayApiError::Unauthorized("test".into())),
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn not_found_is_404() {
    assert_eq!(
        status_of(GatewayApiError::NotFound("test".into())),
        StatusCode::NOT_FOUND
    );
}

#[test]
fn rate_limited_is_429() {
    assert_eq!(
        status_of(GatewayApiError::RateLimited {
            retry_after_secs: 5
        }),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[test]
fn conflict_is_409() {
    assert_eq!(
        status_of(GatewayApiError::Conflict {
            original_request_id: "req-1".into()
        }),
        StatusCode::CONFLICT
    );
}

#[test]
fn payment_required_is_402() {
    assert_eq!(
        status_of(GatewayApiError::PaymentRequired("over budget".into())),
        StatusCode::PAYMENT_REQUIRED
    );
}

#[test]
fn service_unavailable_is_503() {
    assert_eq!(
        status_of(GatewayApiError::ServiceUnavailable("draining".into())),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[test]
fn bad_gateway_is_502() {
    assert_eq!(
        status_of(GatewayApiError::BadGateway("upstream down".into())),
        StatusCode::BAD_GATEWAY
    );
}

#[test]
fn internal_error_is_500() {
    assert_eq!(
        status_of(GatewayApiError::Internal("oops".into())),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn from_anyhow_becomes_internal() {
    let err: GatewayApiError = anyhow::anyhow!("something failed").into();
    assert_eq!(status_of(err), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn from_rusqlite_becomes_internal() {
    let sqlite_err = rusqlite::Error::QueryReturnedNoRows;
    let err: GatewayApiError = sqlite_err.into();
    assert_eq!(status_of(err), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn response_body_has_json_structure() {
    let err = GatewayApiError::BadRequest("invalid prompt".into());
    let response = err.into_response();
    let (parts, body) = response.into_parts();
    assert_eq!(parts.status, StatusCode::BAD_REQUEST);

    // Body should be JSON with error.code, error.kind, error.message, error.retryable
    let rt = tokio::runtime::Runtime::new().unwrap();
    let bytes = rt.block_on(async {
        axum::body::to_bytes(body, 10_000).await.unwrap()
    });
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], 400);
    assert_eq!(json["error"]["kind"], "bad_request");
    assert_eq!(json["error"]["message"], "invalid prompt");
    assert_eq!(json["error"]["retryable"], false);
}
