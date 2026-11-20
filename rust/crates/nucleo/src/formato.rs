use url::Url;

pub fn formatear_duracion(total_segundos: u64) -> String {
    let h = total_segundos / 3600;
    let m = (total_segundos % 3600) / 60;
    let s = total_segundos % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn barra_progreso(actual: u64, total: u64, celdas: usize) -> String {
    if total == 0 || celdas == 0 {
        return "─".repeat(celdas);
    }
    let ratio = (actual as f64 / total as f64).clamp(0.0, 1.0);
    let max_pos = celdas.saturating_sub(1);
    let position = ((ratio * max_pos as f64).round() as usize).min(max_pos);
    let prev = "━".repeat(position);
    let post = "─".repeat(max_pos - position);
    format!("{prev}●{post}")
}

pub fn truncar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        return texto.to_string();
    }
    let limite = max.saturating_sub(1);
    let cortado: String = texto.chars().take(limite).collect();
    format!("{}…", cortado.trim_end())
}

pub fn escapar_markdown(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    for ch in texto.chars() {
        if matches!(
            ch,
            '\\' | '*' | '_' | '`' | '~' | '|' | '>' | '[' | ']' | '(' | ')'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

pub fn url_segura(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if matches!(parsed.scheme(), "http" | "https") {
        Some(parsed.as_str().replace(')', "%29"))
    } else {
        None
    }
}

fn es_caracter_invisible(c: char) -> bool {
    // \p{Cc}\p{Cf}\p{Zl}\p{Zp}\p{Co}
    matches!(c,
        '\u{0000}'..='\u{001F}' | '\u{007F}'..='\u{009F}' | // Cc
        '\u{00AD}' | '\u{0600}'..='\u{0605}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | // Cf
        '\u{2060}'..='\u{206F}' | '\u{FEFF}' |
        '\u{2028}' | // Zl
        '\u{2029}' | // Zp
        '\u{E000}'..='\u{F8FF}' // Co (Private Use)
    )
}

fn es_emoji(c: char) -> bool {
    matches!(c,
        '\u{1F1E6}'..='\u{1F1FF}' | // Regional Indicator
        '\u{1F3FB}'..='\u{1F3FF}' | // Skin tone modifiers
        '\u{20E3}' | '\u{FE0E}' | '\u{FE0F}' | '\u{200D}' | // Combining enclosing keycap, VS15/16, ZWJ
        '\u{00A9}' | '\u{00AE}' | '\u{203C}' | '\u{2049}' | '\u{2122}' | '\u{2139}' |
        '\u{2194}'..='\u{2199}' | '\u{21A9}'..='\u{21AA}' | '\u{231A}'..='\u{231B}' |
        '\u{2328}' | '\u{23CF}' | '\u{23E9}'..='\u{23F3}' | '\u{23F8}'..='\u{23FA}' |
        '\u{24C2}' | '\u{25AA}'..='\u{25AB}' | '\u{25B6}' | '\u{25C0}' |
        '\u{25FB}'..='\u{25FE}' | '\u{2600}'..='\u{27BF}' | '\u{2934}'..='\u{2935}' |
        '\u{2B05}'..='\u{2B07}' | '\u{2B1B}'..='\u{2B1C}' | '\u{2B50}' | '\u{2B55}' |
        '\u{3030}' | '\u{303D}' | '\u{3297}' | '\u{3299}' |
        '\u{1F000}'..='\u{1FAFF}'
    )
}

pub fn etiqueta_enlace(titulo: &str, max: usize) -> String {
    let mut filtrado = String::with_capacity(titulo.len());
    let mut chars = titulo.chars().peekable();
    while let Some(c) = chars.next() {
        if es_emoji(c) || es_caracter_invisible(c) {
            filtrado.push(' ');
        } else if c == '[' {
            filtrado.push('(');
        } else if c == ']' {
            filtrado.push(')');
        } else if c == '.' {
            // Inner dot between alphanumeric replaced by \u{2024}
            let prev_alnum = filtrado
                .chars()
                .last()
                .map(|p| p.is_alphanumeric())
                .unwrap_or(false);
            let next_alnum = chars.peek().map(|n| n.is_alphanumeric()).unwrap_or(false);
            if prev_alnum && next_alnum {
                filtrado.push('\u{2024}');
            } else {
                filtrado.push('.');
            }
        } else {
            filtrado.push(c);
        }
    }
    // Remove scheme ://
    while let Some(idx) = filtrado.find("://") {
        let start = filtrado[..idx]
            .rfind(|c: char| !c.is_alphanumeric() && c != '+' && c != '-' && c != '.')
            .map(|i| i + 1)
            .unwrap_or(0);
        filtrado.replace_range(start..idx + 3, " ");
    }
    // Collapse spaces & trim
    let mut colapsado = String::new();
    let mut en_espacio = false;
    for c in filtrado.trim().chars() {
        if c.is_whitespace() {
            if !en_espacio {
                colapsado.push(' ');
                en_espacio = true;
            }
        } else {
            colapsado.push(c);
            en_espacio = false;
        }
    }
    let label = if colapsado.is_empty() || !colapsado.chars().any(|c| c.is_alphanumeric()) {
        "Untitled".to_string()
    } else {
        colapsado
    };
    escapar_markdown(&truncar(&label, max))
}

pub fn enlace(titulo: &str, url: &str, max: usize) -> String {
    if let Some(href) = url_segura(url) {
        format!("[{}]({href})", etiqueta_enlace(titulo, max))
    } else {
        let limpio: String = titulo.split_whitespace().collect::<Vec<_>>().join(" ");
        escapar_markdown(&truncar(&limpio, max))
    }
}

pub fn parse_time(input: &str) -> Option<u64> {
    let value = input.trim();
    if value.is_empty() {
        return None;
    }
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if i > 0 && part.len() > 2 {
            return None;
        }
    }
    let numbers: Vec<u64> = parts
        .iter()
        .map(|p| p.parse().ok())
        .collect::<Option<Vec<_>>>()?;
    if numbers.iter().skip(1).any(|&n| n >= 60) {
        return None;
    }
    Some(
        numbers
            .into_iter()
            .fold(0u64, |total, part| total * 60 + part),
    )
}

pub fn chunk<T: Clone>(items: &[T], size: usize) -> Result<Vec<Vec<T>>, String> {
    if size < 1 {
        return Err(format!(
            "chunk size must be a positive integer, got: {size}"
        ));
    }
    let mut out = Vec::new();
    for window in items.chunks(size) {
        out.push(window.to_vec());
    }
    Ok(out)
}

pub fn formatear_uptime(ms: u64) -> String {
    let minutes = ms / 60_000;
    let days = minutes / 1440;
    let hours = (minutes % 1440) / 60;
    let mins = minutes % 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    parts.push(format!("{mins}m"));
    parts.join(" ")
}
