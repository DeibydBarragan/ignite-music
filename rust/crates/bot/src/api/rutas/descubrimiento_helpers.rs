use std::collections::HashMap;

pub(super) fn texto_query<'a>(params: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    let value = params.get(key).map(String::as_str).unwrap_or("").trim();
    (value.encode_utf16().count() <= 500).then_some(value)
}

pub(super) fn limite_query(
    params: &HashMap<String, String>,
    default: usize,
    max: usize,
) -> Option<usize> {
    let Some(raw) = params.get("limit") else {
        return Some(default);
    };
    raw.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && n.fract() == 0.0 && *n >= 1.0 && *n <= max as f64)
        .map(|n| n as usize)
}

pub(super) fn consulta_busqueda_valida(query: &str) -> bool {
    let length = query.encode_utf16().count();
    (2..=100).contains(&length)
        && !query.to_ascii_lowercase().starts_with("http://")
        && !query.to_ascii_lowercase().starts_with("https://")
}

pub(super) fn puede_controlar_voz(
    status: &crate::api::formas::EstadoVozRespuesta,
    dev_user: bool,
) -> bool {
    dev_user
        || status
            .bot
            .as_ref()
            .zip(status.user.as_ref())
            .is_some_and(|(bot, user)| bot.id == user.id)
}
