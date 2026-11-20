use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

pub const FALLBACK: &str = "en";

pub const DISCORD_LOCALES: &[(&str, &str)] = &[
    ("en-US", "en"),
    ("en-GB", "en"),
    ("tr", "tr"),
    ("de", "de"),
    ("es-ES", "es"),
    ("es-419", "es"),
    ("fr", "fr"),
    ("it", "it"),
    ("nl", "nl"),
    ("pl", "pl"),
    ("pt-BR", "pt"),
    ("ru", "ru"),
    ("sv-SE", "sv"),
    ("no", "no"),
    ("da", "da"),
    ("fi", "fi"),
    ("cs", "cs"),
    ("ja", "ja"),
    ("ko", "ko"),
    ("zh-CN", "zh_CN"),
    ("zh-TW", "zh_TW"),
    ("hi", "hi"),
    ("th", "th"),
    ("id", "id"),
];

static EMOJI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\p{Extended_Pictographic}\p{Regional_Indicator}\u{1F3FB}-\u{1F3FF}\u{20E3}\u{FE0E}\u{FE0F}\u{200D}]").unwrap()
});
static MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]{2,}").unwrap());
static LINE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[ \t]+").unwrap());
static VAR_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{(\w+)\}").unwrap());

pub fn clean(text: &str) -> String {
    let no_emoji = EMOJI.replace_all(text, "");
    let single_spaces = MULTI_SPACE.replace_all(&no_emoji, " ");
    let no_line_start = LINE_START.replace_all(&single_spaces, "");
    no_line_start.trim().to_string()
}

pub fn lookup<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    let mut current = value;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    current.as_str()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdiomaInfo {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Idiomas {
    languages: HashMap<String, serde_json::Value>,
    discord_locale_map: HashMap<String, String>,
    default_language: String,
}

impl Idiomas {
    pub fn cargar(lang_dir: &Path, default_lang: Option<&str>) -> Result<Self, String> {
        let mut languages = HashMap::new();
        let mut discord_locale_map = HashMap::new();

        let mut file_names: Vec<String> = fs::read_dir(lang_dir)
            .map_err(|e| format!("Failed to read language dir {}: {e}", lang_dir.display()))?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".json") {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        file_names.sort();

        for name in file_names {
            let file = lang_dir.join(&name);
            let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let code = stem.to_string();
            match fs::read_to_string(&file) {
                Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(val) => {
                        languages.insert(code.clone(), val);
                        for &(locale, target) in DISCORD_LOCALES {
                            if target == code {
                                discord_locale_map.insert(locale.to_string(), code.clone());
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Invalid language file {}: {e}", file.display());
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read language file {}: {e}", file.display());
                }
            }
        }

        if !languages.contains_key(FALLBACK) {
            return Err("languages/en.json is required".to_string());
        }

        let def = default_lang.unwrap_or(FALLBACK);
        let default_language = if languages.contains_key(def) {
            def.to_string()
        } else {
            FALLBACK.to_string()
        };

        Ok(Self {
            languages,
            discord_locale_map,
            default_language,
        })
    }

    pub fn t(&self, lang: &str, key: &str, vars: &HashMap<String, String>) -> String {
        let raw = self
            .languages
            .get(lang)
            .and_then(|v| lookup(v, key))
            .or_else(|| self.languages.get(FALLBACK).and_then(|v| lookup(v, key)));

        let Some(text) = raw else {
            return key.to_string();
        };

        let cleaned = clean(text);
        VAR_PATTERN
            .replace_all(&cleaned, |caps: &regex::Captures| {
                let var_name = &caps[1];
                if let Some(val) = vars.get(var_name) {
                    val.clone()
                } else {
                    caps[0].to_string()
                }
            })
            .to_string()
    }

    pub fn from_discord_locale(&self, locale: &str) -> Option<&str> {
        self.discord_locale_map.get(locale).map(|s| s.as_str())
    }

    pub fn list(&self) -> Vec<IdiomaInfo> {
        let mut list = Vec::new();
        for (code, data) in &self.languages {
            let name = lookup(data, "language.name").unwrap_or(code);
            list.push(IdiomaInfo {
                code: code.clone(),
                name: name.to_string(),
            });
        }
        list.sort_by(|a, b| a.code.cmp(&b.code));
        list
    }

    pub fn localizations(&self, key: &str) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for &(locale, target) in DISCORD_LOCALES {
            if target == FALLBACK {
                continue;
            }
            if let Some(data) = self.languages.get(target) {
                if let Some(text) = lookup(data, key) {
                    let cleaned = clean(text);
                    let limited: String = cleaned.chars().take(100).collect();
                    out.insert(locale.to_string(), limited);
                }
            }
        }
        out
    }

    pub fn has_language(&self, code: &str) -> bool {
        self.languages.contains_key(code)
    }

    pub fn find_language_code<'a>(&'a self, code: &str) -> Option<&'a str> {
        self.languages.get_key_value(code).map(|(k, _)| k.as_str())
    }

    pub fn default_language(&self) -> &str {
        &self.default_language
    }

    pub fn len(&self) -> usize {
        self.languages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.languages.is_empty()
    }

    pub fn raw_language_data(&self, code: &str) -> Option<&serde_json::Value> {
        self.languages.get(code)
    }

    pub fn iter_languages(&self) -> impl Iterator<Item = (&String, &serde_json::Value)> {
        self.languages.iter()
    }
}
