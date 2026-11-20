//! Módulo de seguridad y autenticación para la API REST (Spec S-26).

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

pub const MIN_SECRET_LENGTH: usize = 32;
pub const SNOWFLAKE_MIN_LEN: usize = 17;
pub const SNOWFLAKE_MAX_LEN: usize = 20;
pub const CABECERA_USER_ID: &str = "x-user-discord-id";

/// Identificador de usuario de Discord adjunto a peticiones de mutación validadas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsuarioDiscord(pub String);

/// Comparación en tiempo constante de dos cadenas UTF-8.
///
/// Primero se comparan las longitudes; si difieren, falla inmediatamente
/// sin filtrar longitudes ni realizar iteraciones. Si las longitudes coinciden,
/// los bytes se comparan en tiempo constante mediante acumulador XOR.
pub fn safe_equal(a: &str, b: &str) -> bool {
    let left = a.as_bytes();
    let right = b.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let diff = left
        .iter()
        .zip(right.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y));
    diff == 0
}

/// Extrae el token de una cabecera `Authorization: Bearer <token>`.
///
/// El esquema "Bearer" se lee sin distinguir mayúsculas/minúsculas.
/// Devuelve `None` si la cabecera está ausente, usa otro esquema, o el token
/// está vacío o contiene espacios en blanco.
pub fn extract_bearer(header: &str) -> Option<&str> {
    let trimmed = header.trim();
    if trimmed.len() < 7 {
        return None;
    }
    let (prefix, rest) = trimmed.split_at(6);
    if !prefix.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = rest.trim();
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return None;
    }
    Some(token)
}

/// Valida si una cadena es un snowflake de Discord válido (17 a 20 dígitos ASCII).
pub fn es_snowflake_valido(s: &str) -> bool {
    (SNOWFLAKE_MIN_LEN..=SNOWFLAKE_MAX_LEN).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_digit())
}

/// Determina si un método HTTP es una mutación (POST, PUT, PATCH o DELETE).
pub fn es_metodo_mutacion(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// Capa de autenticación Bearer y validación de usuario de Discord.
pub async fn middleware_autenticacion_y_usuario(
    State(config): State<Arc<nucleo::Config>>,
    mut req: Request,
    next: Next,
) -> Response {
    // 1. Verificación de longitud del secreto configurado
    if config.api.secret.len() < MIN_SECRET_LENGTH {
        tracing::error!("API_SECRET is missing or too short; refusing every API request.");
        return crate::api::respuesta_problema(
            503,
            "AUTH_NOT_CONFIGURED",
            "API authentication is not configured.",
        );
    }

    // 2. Extracción y comparación en tiempo constante del token Bearer
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token_valido = match auth_header.and_then(extract_bearer) {
        Some(token) => safe_equal(token, &config.api.secret),
        None => false,
    };

    if !token_valido {
        return crate::api::respuesta_problema(
            401,
            "UNAUTHORIZED",
            "Invalid or missing bearer token.",
        );
    }

    // 3. Mutaciones requieren cabecera X-User-Discord-Id válida
    if es_metodo_mutacion(req.method()) {
        let user_header = req
            .headers()
            .get(CABECERA_USER_ID)
            .and_then(|h| h.to_str().ok())
            .map(str::trim)
            .map(str::to_string);

        match user_header {
            Some(snowflake) if es_snowflake_valido(&snowflake) => {
                req.extensions_mut().insert(UsuarioDiscord(snowflake));
            }
            _ => {
                return crate::api::respuesta_problema(
                    400,
                    "INVALID_USER",
                    "Missing or invalid X-User-Discord-Id header (Discord snowflake required).",
                );
            }
        }
    }

    next.run(req).await
}
