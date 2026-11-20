use std::collections::HashMap;

/// Helper to get a trimmed env value with fallback.
pub fn get_env<'a>(map: &'a HashMap<String, String>, key: &str, fallback: &'a str) -> &'a str {
    match map.get(key) {
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                fallback
            } else {
                trimmed
            }
        }
        None => fallback,
    }
}

/// Parse leading digits like JavaScript Number.parseInt(val, 10).
pub fn parse_js_int(val: &str) -> Option<i64> {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut chars = trimmed.chars().peekable();
    let mut sign = 1i64;
    if let Some(&c) = chars.peek() {
        if c == '-' {
            sign = -1;
            chars.next();
        } else if c == '+' {
            chars.next();
        }
    }

    let mut digits = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            digits.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if digits.is_empty() {
        return None;
    }

    digits.parse::<i64>().ok().map(|n| n * sign)
}

/// Parse an integer env variable with JS parseInt semantics and clamping.
pub fn get_env_int(
    map: &HashMap<String, String>,
    key: &str,
    fallback: i64,
    min: Option<i64>,
    max: Option<i64>,
) -> i64 {
    let raw = get_env(map, key, "");
    let parsed = match parse_js_int(raw) {
        Some(n) => n,
        None => fallback,
    };

    let mut clamped = parsed;
    if let Some(m) = min {
        if clamped < m {
            clamped = m;
        }
    }
    if let Some(m) = max {
        if clamped > m {
            clamped = m;
        }
    }
    clamped
}

/// Parse a boolean env variable: 1, true, yes, on are true. Any other non-empty is false.
pub fn get_env_bool(map: &HashMap<String, String>, key: &str, fallback: bool) -> bool {
    let raw = get_env(map, key, "");
    if raw.is_empty() {
        return fallback;
    }
    let lower = raw.to_ascii_lowercase();
    matches!(lower.as_str(), "1" | "true" | "yes" | "on")
}

/// Parse a comma-separated list of strings, trimmed and non-empty.
pub fn get_env_list(map: &HashMap<String, String>, key: &str) -> Vec<String> {
    let raw = get_env(map, key, "");
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse hex color like #5865F2 or 5865F2 into a u32 RGB value. Default is 0x5865F2.
pub fn parse_color(val: &str) -> u32 {
    let clean = val.trim().trim_start_matches('#');
    if clean.len() == 6 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        u32::from_str_radix(clean, 16).unwrap_or(0x5865F2)
    } else {
        0x5865F2
    }
}

/// Parse .env file contents matching Node dotenv behavior.
pub fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let mut trimmed = line.trim_start();
        if trimmed.starts_with("export ") {
            trimmed = trimmed["export ".len()..].trim_start();
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some(eq_idx) = trimmed.find('=') else {
            continue;
        };

        let key = trimmed[..eq_idx].trim().to_string();
        if key.is_empty() {
            continue;
        }

        let val_part = trimmed[eq_idx + 1..].trim_start();
        let value = if let Some(stripped) = val_part.strip_prefix('"') {
            // Double-quoted
            if let Some(end_idx) = stripped.find('"') {
                stripped[..end_idx]
                    .replace("\\n", "\n")
                    .replace("\\r", "\r")
                    .replace("\\t", "\t")
                    .replace("\\\"", "\"")
            } else {
                stripped.to_string()
            }
        } else if let Some(stripped) = val_part.strip_prefix('\'') {
            // Single-quoted
            if let Some(end_idx) = stripped.find('\'') {
                stripped[..end_idx].to_string()
            } else {
                stripped.to_string()
            }
        } else if let Some(stripped) = val_part.strip_prefix('`') {
            // Backtick-quoted
            if let Some(end_idx) = stripped.find('`') {
                stripped[..end_idx].to_string()
            } else {
                stripped.to_string()
            }
        } else {
            // Unquoted: up to first `#` comment
            let unquoted = if let Some(hash_idx) = val_part.find('#') {
                &val_part[..hash_idx]
            } else {
                val_part
            };
            unquoted.trim_end().to_string()
        };

        map.insert(key, value);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_js_int() {
        assert_eq!(parse_js_int("12abc"), Some(12));
        assert_eq!(parse_js_int("-5xyz"), Some(-5));
        assert_eq!(parse_js_int("+42"), Some(42));
        assert_eq!(parse_js_int("abc"), None);
        assert_eq!(parse_js_int(""), None);
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("#5865F2"), 0x5865F2);
        assert_eq!(parse_color("5865F2"), 0x5865F2);
        assert_eq!(parse_color("#ff0000"), 0xFF0000);
        assert_eq!(parse_color("invalid"), 0x5865F2);
        assert_eq!(parse_color("123"), 0x5865F2);
    }
}
