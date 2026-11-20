//! Validación de parámetros y protección SSRF para la API REST (Spec S-27).

use std::net::{Ipv4Addr, Ipv6Addr};
use url::Url;

pub const SNOWFLAKE_MIN_LEN: usize = 17;
pub const SNOWFLAKE_MAX_LEN: usize = 20;

/// Valida si una cadena es un snowflake de Discord válido (17-20 dígitos ASCII).
pub fn es_snowflake(s: &str) -> bool {
    let len = s.len();
    (SNOWFLAKE_MIN_LEN..=SNOWFLAKE_MAX_LEN).contains(&len) && s.bytes().all(|b| b.is_ascii_digit())
}

/// Valida un snowflake opcional: `None`, cadena vacía o snowflake válido.
pub fn es_snowflake_opcional(s: Option<&str>) -> bool {
    match s {
        None => true,
        Some(v) => v.is_empty() || es_snowflake(v),
    }
}

/// Valida el identificador único de una pista en cola (`trackId`: 8-64 caracteres alfanuméricos, `_` o `-`).
pub fn es_track_id_valido(s: &str) -> bool {
    let len = s.len();
    (8..=64).contains(&len)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Detecta URLs no soportadas (ej. SoundCloud descontinuado).
pub fn razon_url_no_soportada(input: &str) -> Option<&'static str> {
    if input.to_ascii_lowercase().contains("soundcloud.com") {
        Some("SoundCloud is no longer supported. Use a YouTube or Spotify link, or search by name.")
    } else {
        None
    }
}

fn es_ipv4_privada(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 0
        || a == 10
        || a == 127
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 169 && b == 254)
}

fn es_ipv6_privada(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    let octets = ip.octets();
    // Unique local (fc00::/7) -> primeros 7 bits son 1111110 -> 0xfc o 0xfd
    if octets[0] == 0xfc || octets[0] == 0xfd {
        return true;
    }
    // Link local (fe80::/10) -> primeros 10 bits son 1111111010 -> 0xfe y bits 7-6 en 0b10 (0x80..0xbf)
    if octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80 {
        return true;
    }
    // Mapped IPv4 (::ffff:a.b.c.d)
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return es_ipv4_privada(ipv4);
    }
    false
}

/// Comprueba si un hostname apunta a bucle local, redes privadas o metadatos de nube.
pub fn es_hostname_privado(hostname: &str) -> bool {
    let host = hostname
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }

    if host == "localhost"
        || host == "localhost.localdomain"
        || host == "metadata.google.internal"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".home.arpa")
    {
        return true;
    }

    if let Ok(ip4) = host.parse::<Ipv4Addr>() {
        return es_ipv4_privada(ip4);
    }

    if let Ok(ip6) = host.parse::<Ipv6Addr>() {
        return es_ipv6_privada(ip6);
    }

    false
}

fn tiene_esquema_explicito(val: &str) -> bool {
    if let Some(pos) = val.find("://") {
        let prefix = &val[..pos];
        !prefix.is_empty()
            && prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'.' || b == b'-')
    } else {
        false
    }
}

/// Inspecciona un destino para evitar peticiones maliciosas o ataques SSRF.
///
/// Devuelve `None` si el destino es seguro (texto de búsqueda normal o URL pública HTTP/HTTPS).
/// Si es inseguro o bloqueado, devuelve `Some((código, detalle))`.
pub fn inspeccionar_destino(raw: &str) -> Option<(&'static str, String)> {
    let val = raw.trim();
    if val.is_empty() {
        return None;
    }

    let lower = val.to_ascii_lowercase();
    if lower.starts_with("file:")
        || lower.starts_with("data:")
        || lower.starts_with("javascript:")
        || lower.starts_with("vbscript:")
        || lower.starts_with("blob:")
    {
        return Some((
            "BLOCKED_URL",
            "Local file and script URLs are not allowed.".to_string(),
        ));
    }

    if !tiene_esquema_explicito(val) {
        return None;
    }

    let parsed = match Url::parse(val) {
        Ok(u) => u,
        Err(_) => return None,
    };

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Some((
            "BLOCKED_URL",
            format!("URL scheme \"{}:\" is not allowed.", scheme),
        ));
    }

    if let Some(host) = parsed.host_str() {
        if es_hostname_privado(host) {
            return Some((
                "BLOCKED_URL",
                "URLs pointing to local or private networks are not allowed.".to_string(),
            ));
        }
    }

    None
}
