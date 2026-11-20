pub mod almacen;
pub mod almacen_disco;
pub mod cache;
pub mod config;
pub mod config_model;
pub mod env;
pub mod error;
pub mod error_api;
pub mod formato;
pub mod idioma;
pub mod idioma_servidor;
pub mod preflight;
pub mod preflight_rules;
pub mod registro;
pub mod report;
pub mod token;

pub use almacen::{AlmacenError, AlmacenJson, ClaveInvalidaError};
pub use cache::CacheTtl;
pub use config::{BotConfig, Config, DiscordConfig};
pub use error::ArranqueError;
pub use error_api::{
    default_type_for_code, error_title_for_status, from_any_error, problem, status_by_code,
    ApiError, ProblemDocument, ERROR_BASE,
};
pub use idioma::{clean, IdiomaInfo, Idiomas, DISCORD_LOCALES, FALLBACK};
pub use idioma_servidor::{guild_language, set_guild_language, IdiomaDesconocido, LegacyLanguages};
pub use preflight::run_preflight;
pub use report::Reporter;
pub use token::{check_token, ApplicationInfo, TokenCheckResult};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn cargar_mapa_env(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let env_file = root.join(".env");
    if env_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&env_file) {
            map.extend(crate::env::parse_dotenv(&content));
        }
    }
    // Variables from the system environment win over .env file
    for (k, v) in std::env::vars() {
        map.insert(k, v);
    }
    map
}

pub async fn arrancar(root: PathBuf) -> Result<Config, ArranqueError> {
    let map = cargar_mapa_env(&root);
    let mut config = Config::from_map(root, &map);
    let reporter = Reporter::new(config.no_color);
    run_preflight(&mut config, &reporter, &token::DiscordTokenChecker).await?;
    Ok(config)
}
