//! Capas HTTP de la API REST: seguridad, CORS, cuerpo y errores (Spec S-26).

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::header::{
    HeaderName, HeaderValue, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_LENGTH, CONTENT_TYPE, ORIGIN, VARY,
};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use futures_util::FutureExt;
use std::sync::Arc;

pub const MAX_BODY_BYTES: usize = 100 * 1024; // 100 kB

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestId(pub String);

pub fn generar_request_id() -> String {
    format!("{:08x}", fastrand::u32(..))
}

pub async fn middleware_request_id(mut req: Request, next: Next) -> Response {
    let req_id = RequestId(generar_request_id());
    req.extensions_mut().insert(req_id);
    next.run(req).await
}

pub async fn middleware_cabeceras_seguridad(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self';base-uri 'self';font-src 'self' https: data:;form-action 'self';frame-ancestors 'self';img-src 'self' data:;object-src 'none';script-src 'self';script-src-attr 'none';style-src 'self' https: 'unsafe-inline';upgrade-insecure-requests",
        ),
    );
    h.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    h.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );
    h.insert(
        HeaderName::from_static("origin-agent-cluster"),
        HeaderValue::from_static("?1"),
    );
    h.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    h.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        HeaderName::from_static("x-dns-prefetch-control"),
        HeaderValue::from_static("off"),
    );
    h.insert(
        HeaderName::from_static("x-download-options"),
        HeaderValue::from_static("noopen"),
    );
    h.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("SAMEORIGIN"),
    );
    h.insert(
        HeaderName::from_static("x-permitted-cross-domain-policies"),
        HeaderValue::from_static("none"),
    );
    h.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("0"),
    );
    h.remove(HeaderName::from_static("x-powered-by"));
    res
}

pub async fn middleware_cors(
    State(config): State<Arc<nucleo::Config>>,
    req: Request,
    next: Next,
) -> Response {
    let origin_hdr = req
        .headers()
        .get(ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let allowed = match &origin_hdr {
        None => false,
        Some(o) => config
            .api
            .allowed_origins
            .iter()
            .any(|a| a == "*" || a == o),
    };

    if req.method() == Method::OPTIONS && allowed {
        if let Some(origin_str) = origin_hdr {
            let mut res = Response::new(Body::empty());
            *res.status_mut() = StatusCode::NO_CONTENT;
            let h = res.headers_mut();
            if let Ok(val) = HeaderValue::from_str(&origin_str) {
                h.insert(ACCESS_CONTROL_ALLOW_ORIGIN, val);
            }
            h.insert(
                ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET,HEAD,PUT,PATCH,POST,DELETE"),
            );
            let req_headers = req
                .headers()
                .get("access-control-request-headers")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("Authorization, Content-Type, X-User-Discord-Id");
            if let Ok(val) = HeaderValue::from_str(req_headers) {
                h.insert(ACCESS_CONTROL_ALLOW_HEADERS, val);
            }
            h.insert(
                VARY,
                HeaderValue::from_static("Origin, Access-Control-Request-Headers"),
            );
            return res;
        }
    }

    let mut res = next.run(req).await;
    if allowed {
        if let Some(o) = origin_hdr {
            if let Ok(val) = HeaderValue::from_str(&o) {
                res.headers_mut().insert(ACCESS_CONTROL_ALLOW_ORIGIN, val);
                res.headers_mut()
                    .insert(VARY, HeaderValue::from_static("Origin"));
            }
        }
    }
    res
}

pub async fn middleware_limite_cuerpo_y_json(req: Request, next: Next) -> Response {
    if let Some(cl) = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(bytes_len) = cl.parse::<usize>() {
            if bytes_len > MAX_BODY_BYTES {
                return crate::api::respuesta_problema(
                    413,
                    "PAYLOAD_TOO_LARGE",
                    "Cuerpo de la petición demasiado grande.",
                );
            }
        }
    }

    let is_json = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("json"))
        .unwrap_or(false);

    let (parts, body) = req.into_parts();
    let bytes = match to_bytes(body, MAX_BODY_BYTES + 1).await {
        Ok(b) if b.len() > MAX_BODY_BYTES => {
            return crate::api::respuesta_problema(
                413,
                "PAYLOAD_TOO_LARGE",
                "Cuerpo de la petición demasiado grande.",
            );
        }
        Ok(b) => b,
        Err(_) => {
            return crate::api::respuesta_problema(
                413,
                "PAYLOAD_TOO_LARGE",
                "Cuerpo de la petición demasiado grande.",
            );
        }
    };

    if is_json && !bytes.is_empty() && serde_json::from_slice::<serde_json::Value>(&bytes).is_err()
    {
        return crate::api::respuesta_problema(400, "INVALID_JSON", "JSON mal formado.");
    }

    let new_req = Request::from_parts(parts, Body::from(bytes));
    next.run(new_req).await
}

pub async fn middleware_manejo_errores(req: Request, next: Next) -> Response {
    let req_id = req
        .extensions()
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(generar_request_id);
    let method = req.method().clone();
    let uri = req.uri().clone();

    std::panic::AssertUnwindSafe(next.run(req))
        .catch_unwind()
        .await
        .unwrap_or_else(|_| {
            tracing::error!(
                "Unhandled API error ({} {}) [req {}]: internal server error",
                method,
                uri,
                req_id
            );
            crate::api::respuesta_problema(500, "INTERNAL_ERROR", "Error interno del servidor.")
        })
}
