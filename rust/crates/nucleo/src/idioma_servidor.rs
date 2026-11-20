use crate::almacen::AlmacenJson;
use crate::idioma::Idiomas;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdiomaDesconocido(pub String);

impl fmt::Display for IdiomaDesconocido {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown language: {}", self.0)
    }
}

impl std::error::Error for IdiomaDesconocido {}

#[derive(Debug, Clone, Default)]
pub struct LegacyLanguages {
    servers: HashMap<String, String>,
}

impl LegacyLanguages {
    pub fn cargar(root_dir: &Path) -> Self {
        let legacy_file = root_dir.join("database").join("languages.json");
        let mut servers = HashMap::new();
        if let Ok(content) = fs::read_to_string(&legacy_file) {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                if let Some(srv_map) = val.get("servers").and_then(|s| s.as_object()) {
                    for (gid, entry) in srv_map {
                        if let Some(lang) = entry.get("language").and_then(|l| l.as_str()) {
                            servers.insert(gid.clone(), lang.to_string());
                        }
                    }
                }
            }
        }
        Self { servers }
    }

    pub fn get(&self, guild_id: &str) -> Option<&str> {
        self.servers.get(guild_id).map(|s| s.as_str())
    }
}

pub fn guild_language<'a>(
    idiomas: &'a Idiomas,
    data_dir: &Path,
    legacy: &'a LegacyLanguages,
    guild_id: Option<&str>,
    guild_locale: Option<&str>,
) -> &'a str {
    if let Some(gid) = guild_id {
        let settings_file = data_dir.join("guilds").join(format!("{gid}.json"));
        if let Ok(content) = fs::read_to_string(&settings_file) {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                if let Some(lang) = val.get("language").and_then(|l| l.as_str()) {
                    if let Some(code) = idiomas.find_language_code(lang) {
                        return code;
                    }
                }
            }
        }

        if let Some(old_lang) = legacy.get(gid) {
            if let Some(code) = idiomas.find_language_code(old_lang) {
                return code;
            }
        }
    }

    if let Some(loc) = guild_locale {
        if let Some(mapped) = idiomas.from_discord_locale(loc) {
            return mapped;
        }
    }

    idiomas.default_language()
}

pub async fn set_guild_language_store(
    idiomas: &Idiomas,
    store: &AlmacenJson,
    guild_id: &str,
    code: &str,
) -> Result<(), IdiomaDesconocido> {
    if !idiomas.has_language(code) {
        return Err(IdiomaDesconocido(code.to_string()));
    }

    let mut current = match store.get(guild_id) {
        Ok(Some(Value::Object(map))) => map,
        _ => serde_json::Map::new(),
    };
    current.insert("language".to_string(), Value::String(code.to_string()));

    store
        .set(guild_id, Value::Object(current))
        .await
        .map_err(|_| IdiomaDesconocido(code.to_string()))?;

    Ok(())
}

pub async fn set_guild_language(
    idiomas: &Idiomas,
    data_dir: &Path,
    guild_id: &str,
    code: &str,
) -> Result<(), IdiomaDesconocido> {
    let store = AlmacenJson::new(data_dir.join("guilds"));
    set_guild_language_store(idiomas, &store, guild_id, code).await
}
