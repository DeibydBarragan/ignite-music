//! Módulo de comandos de barra (Spec S-22).

pub mod acciones;
pub mod ayuda;
pub mod buscar;
pub mod cola;
pub mod definiciones;
pub mod idioma;
pub mod letras;
pub mod listas;
pub mod listas_crear;
pub mod registro;
pub mod reproducir;

use nucleo::Idiomas;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Contexto {
    pub idiomas: Arc<Idiomas>,
    pub default_language: String,
    pub gestor_colas: Arc<player::GestorColas>,
    pub almacen_sesiones: Arc<player::AlmacenSesiones>,
    pub config_bot: nucleo::BotConfig,
    pub command_ids: Arc<RwLock<HashMap<String, String>>>,
    pub client_id: String,
    pub token: String,
    pub root: std::path::PathBuf,
    pub data_dir: std::path::PathBuf,
    pub legacy_languages: Arc<nucleo::LegacyLanguages>,
    pub gestor_listas: Option<Arc<crate::datos::GestorListas>>,
    pub gestor_favoritos: Option<Arc<crate::datos::GestorFavoritos>>,
    pub driver: Option<Arc<player::SongbirdDriver>>,
    pub ytdlp_settings: medios::ytdlp_args::YtDlpSettings,
    pub ytdlp_args: Vec<String>,
}

impl Contexto {
    pub fn t(&self, lang: &str, key: &str, vars: &HashMap<String, String>) -> String {
        self.idiomas.t(lang, key, vars)
    }
}

pub fn categoria_comando(nombre: &str) -> Option<&'static str> {
    match nombre {
        "autoplay" | "filter" | "loop" | "lyrics" | "new-playlist" | "nowplaying" | "pause"
        | "play" | "playlists" | "playnext" | "previous" | "resume" | "search" | "seek"
        | "skip" | "stop" | "volume" => Some("playback"),
        "clear" | "jump" | "move" | "queue" | "remove" | "shuffle" => Some("queue"),
        "help" | "language" => Some("settings"),
        _ => None,
    }
}

pub fn tiene_autocompletado(nombre: &str) -> bool {
    matches!(nombre, "play" | "playnext")
}

pub async fn despachar_comando(
    ctx: &Contexto,
    guild_id: &str,
    nombre: &str,
    opciones: &HashMap<String, serde_json::Value>,
) -> Result<Option<String>, String> {
    let lang = &ctx.default_language;

    match nombre {
        "autoplay" => acciones::ejecutar_autoplay(ctx, guild_id, lang)
            .await
            .map(Some),
        "clear" => acciones::ejecutar_limpiar(ctx, guild_id, lang)
            .await
            .map(Some),
        "filter" => {
            let val = opciones
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            acciones::ejecutar_filtro(ctx, guild_id, lang, val)
                .await
                .map(Some)
        }
        "jump" => {
            let pos = opciones
                .get("position")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
            acciones::ejecutar_saltar_a(ctx, guild_id, lang, pos)
                .await
                .map(Some)
        }
        "loop" => {
            let modo = opciones
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("off");
            acciones::ejecutar_bucle(ctx, guild_id, lang, modo)
                .await
                .map(Some)
        }
        "move" => {
            let from = opciones.get("from").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let to = opciones.get("to").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            acciones::ejecutar_mover(ctx, guild_id, lang, from, to)
                .await
                .map(Some)
        }
        "pause" => acciones::ejecutar_pausa(ctx, guild_id, lang)
            .await
            .map(Some),
        "play" => {
            let q = opciones.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let next = opciones
                .get("next")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            reproducir::reproducir_desde_interaccion(ctx, guild_id, lang, q, next)
                .await
                .map(Some)
        }
        "playnext" => {
            let q = opciones.get("query").and_then(|v| v.as_str()).unwrap_or("");
            reproducir::reproducir_desde_interaccion(ctx, guild_id, lang, q, true)
                .await
                .map(Some)
        }
        "previous" => acciones::ejecutar_anterior(ctx, guild_id, lang)
            .await
            .map(Some),
        "remove" => {
            let pos = opciones
                .get("position")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
            acciones::ejecutar_quitar(ctx, guild_id, lang, pos)
                .await
                .map(Some)
        }
        "resume" => acciones::ejecutar_reanudar(ctx, guild_id, lang)
            .await
            .map(Some),
        "shuffle" => acciones::ejecutar_mezclar(ctx, guild_id, lang)
            .await
            .map(Some),
        "skip" => acciones::ejecutar_saltar(ctx, guild_id, lang)
            .await
            .map(Some),
        "stop" => acciones::ejecutar_parar(ctx, guild_id, lang)
            .await
            .map(Some),
        "volume" => {
            let lvl = opciones
                .get("level")
                .and_then(|v| v.as_u64())
                .unwrap_or(100) as u8;
            acciones::ejecutar_volumen(ctx, guild_id, lang, lvl)
                .await
                .map(Some)
        }
        "queue" => {
            let page = opciones.get("page").and_then(|v| v.as_i64());
            let payload = cola::ejecutar_cola(ctx, guild_id, lang, page).await;
            Ok(payload.content.or(Some("queue".to_string())))
        }
        "help" => Ok(Some(ctx.t(lang, "commands.help.title", &HashMap::new()))),
        "language" => Ok(Some(ctx.t(
            lang,
            "commands.language.title",
            &HashMap::new(),
        ))),
        "lyrics" => Ok(Some(ctx.t(
            lang,
            "buttonhandler.lyrics_title",
            &HashMap::new(),
        ))),
        "search" => Ok(Some(ctx.t(lang, "commands.search.title", &HashMap::new()))),
        "playlists" => Ok(Some(ctx.t(lang, "ui.playlists_title", &HashMap::new()))),
        "new-playlist" => Ok(Some("new-playlist".to_string())),
        "nowplaying" | "seek" => Ok(Some(ctx.t(
            lang,
            "commands.nowplaying.title",
            &HashMap::new(),
        ))),
        _ => {
            tracing::warn!("Comando desconocido recibido: {}", nombre);
            Ok(None)
        }
    }
}

pub async fn autocompletar_comando(
    nombre: &str,
    consulta: &str,
) -> Vec<reproducir::EleccionAutocompletado> {
    if tiene_autocompletado(nombre) {
        reproducir::autocompletar(consulta).await
    } else {
        Vec::new()
    }
}
