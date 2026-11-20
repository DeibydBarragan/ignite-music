//! Manejo de streams y respuesta HTTP para Server-Sent Events (Spec S-28).

use super::sse::HubSse;
use axum::body::Bytes;
use axum::http::header::{CACHE_CONTROL, CONNECTION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use futures_util::Stream;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc::Receiver;

pub struct SseStream {
    pub(crate) hub: Arc<HubSse>,
    pub(crate) guild_id: String,
    pub(crate) sub_id: u64,
    pub(crate) rx: Receiver<Bytes>,
}

pub(super) fn codificar_evento(evento: &str, data: &serde_json::Value) -> Bytes {
    Bytes::from(format!("event: {evento}\ndata: {data}\n\n"))
}

impl Stream for SseStream {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(b)) => Poll::Ready(Some(Ok(b))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for SseStream {
    fn drop(&mut self) {
        self.hub.desuscribir(&self.guild_id, self.sub_id);
    }
}

pub fn construir_respuesta_sse(stream: SseStream) -> Response {
    let body = axum::body::Body::from_stream(stream);
    let mut res = Response::new(body);
    *res.status_mut() = StatusCode::OK;
    let h = res.headers_mut();
    h.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    h.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    h.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
    h.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    res
}
