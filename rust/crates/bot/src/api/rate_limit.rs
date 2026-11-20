//! Capa de limitación de tasa (rate limiting) para la API REST (Spec S-26).

use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

const HEADER_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
const HEADER_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
const HEADER_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");
const HEADER_RETRY_AFTER: HeaderName = HeaderName::from_static("retry-after");

#[derive(Clone, Copy)]
struct VentanaIp {
    inicio: Instant,
    cuenta: u32,
    reset_epoch: u64,
}

#[derive(Clone, Default)]
pub struct EstadoRateLimit {
    tabla: Arc<RwLock<HashMap<IpAddr, VentanaIp>>>,
}

fn extraer_ip(req: &Request) -> IpAddr {
    if let Some(fwd) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = fwd.split(',').next() {
            if let Ok(ip) = first.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }
    if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return addr.ip();
    }
    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
}

pub async fn middleware_rate_limit(
    State((estado, limit)): State<(EstadoRateLimit, u32)>,
    req: Request,
    next: Next,
) -> Response {
    let ip = extraer_ip(&req);
    let now_inst = Instant::now();
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let (cuenta, reset_epoch) = {
        let mut mapa = estado.tabla.write().await;
        let entrada = mapa.entry(ip).or_insert(VentanaIp {
            inicio: now_inst,
            cuenta: 0,
            reset_epoch: now_epoch + 60,
        });
        if now_inst.duration_since(entrada.inicio) >= Duration::from_secs(60) {
            entrada.inicio = now_inst;
            entrada.cuenta = 0;
            entrada.reset_epoch = now_epoch + 60;
        }
        entrada.cuenta += 1;
        (entrada.cuenta, entrada.reset_epoch)
    };

    let remaining = limit.saturating_sub(cuenta);
    let retry_after = reset_epoch.saturating_sub(now_epoch).max(1);

    if cuenta > limit {
        let mut res = crate::api::respuesta_problema(
            429,
            "RATE_LIMITED",
            "Demasiadas peticiones. Inténtalo más tarde.",
        );
        let h = res.headers_mut();
        h.insert(HEADER_LIMIT, HeaderValue::from(limit));
        h.insert(HEADER_REMAINING, HeaderValue::from(0));
        h.insert(HEADER_RESET, HeaderValue::from(reset_epoch));
        h.insert(HEADER_RETRY_AFTER, HeaderValue::from(retry_after));
        return res;
    }

    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(HEADER_LIMIT, HeaderValue::from(limit));
    h.insert(HEADER_REMAINING, HeaderValue::from(remaining));
    h.insert(HEADER_RESET, HeaderValue::from(reset_epoch));
    res
}
