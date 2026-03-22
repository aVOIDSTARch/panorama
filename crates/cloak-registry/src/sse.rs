use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use cloak_core::HaltEvent;

use crate::store::{ServiceStore, SseEvent};

/// Create an SSE response stream for a service's halt channel.
pub fn halt_stream(
    store: &ServiceStore,
    service_id: &str,
) -> Option<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>> {
    let rx = store.subscribe(service_id)?;

    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(sse_event) => Some(Ok(Event::default().data(sse_event.data))),
            Err(_) => None, // lagged receiver, skip
        }
    });

    Some(
        Sse::new(stream).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        ),
    )
}

/// Build an SSE event payload for a halt signal.
pub fn halt_event(service_id: Option<&str>, reason: &str) -> SseEvent {
    let event = HaltEvent {
        event_type: "halt".into(),
        service_id: service_id.map(String::from),
        reason: Some(reason.into()),
        new_key: None,
    };
    SseEvent {
        data: serde_json::to_string(&event).unwrap_or_default(),
    }
}

/// Build an SSE event payload for a key rotation signal.
pub fn key_rotation_event(service_id: &str, new_key_b64: &str) -> SseEvent {
    let event = HaltEvent {
        event_type: "key_rotation".into(),
        service_id: Some(service_id.into()),
        reason: None,
        new_key: Some(new_key_b64.into()),
    };
    SseEvent {
        data: serde_json::to_string(&event).unwrap_or_default(),
    }
}
