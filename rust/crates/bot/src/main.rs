//! Punto de entrada de ignite-music en Rust (Spec S-21 y S-22).

use bot::acciones_voz::AccionesVozBot;
use bot::apagado::PasosApagadoReal;
use bot::arranque::{supervisar_gateway, HooksBotReal};
use bot::cliente::{
    crear_cache, crear_shard, ejecutar_bucle_eventos, limpiar_cache_antigua,
    verificar_variables_sharding_obsoletas, DespachadorEventos,
};
use bot::interacciones::RouterInteraccionesReal;
use bot::restauracion::ProveedorRestauracionReal;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use twilight_model::id::Id;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let debug = std::env::var("DEBUG")
        .map(|v| v != "0" && !v.is_empty())
        .unwrap_or(false);
    let no_color = std::env::var("NO_COLOR").is_ok();
    nucleo::registro::inicializar(debug, no_color);

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("Pánico detectado: {}", info);
        default_hook(info);
    }));

    let root = std::env::current_dir()?;
    let mut config = match nucleo::arrancar(root).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Error en preflight de arranque: {}", e);
            std::process::exit(1);
        }
    };

    verificar_variables_sharding_obsoletas();
    limpiar_cache_antigua(&config.root);

    let rutas_binarios = match bot::binarios::asegurar_binarios(&config).await {
        Ok(rutas) => rutas,
        Err(e) => {
            tracing::error!("Error de binarios en arranque: {}", e);
            std::process::exit(1);
        }
    };

    config.ytdlp.path = rutas_binarios.ytdlp.to_string_lossy().to_string();
    config.ffmpeg_path = rutas_binarios.ffmpeg.to_string_lossy().to_string();
    std::env::set_var("FFMPEG_PATH", &config.ffmpeg_path);
    std::env::set_var("YTDLP_PATH", &config.ytdlp.path);
    if let Some(ref cf) = rutas_binarios.cloudflared {
        std::env::set_var("CLOUDFLARED_PATH", cf);
    }
    if let Some(ref vosk) = rutas_binarios.libvosk {
        std::env::set_var("VOSK_DLL_PATH", vosk);
    }

    let gid_opt = config
        .discord
        .guild_id
        .clone()
        .filter(|s| !s.trim().is_empty());
    let idiomas = Arc::new(
        nucleo::idioma::Idiomas::cargar(
            &config.root.join("languages"),
            Some(&config.bot.default_language),
        )
        .expect("No se pudieron cargar los idiomas del bot"),
    );

    let cuerpos = bot::comandos::definiciones::todos_los_cuerpos(&idiomas, config.bot.max_volume);
    if let Err(e) = bot::comandos::registro::desplegar_comandos(
        &config.discord.token,
        &config.discord.client_id,
        gid_opt.as_deref(),
        &config.data_dir,
        &cuerpos,
        false,
    )
    .await
    {
        tracing::warn!("No se pudieron registrar los comandos de barra: {}", e);
    }

    let logs_dir = config.data_dir.join("voice-logs");
    let exportador_csv = Arc::new(voice::ExportadorCsvVoz::with_dir(logs_dir));
    let _retencion = exportador_csv
        .clone()
        .iniciar_tarea_retencion(30, Duration::from_secs(86_400));

    let gestor_colas = Arc::new(player::GestorColas::nuevo());
    let almacen_json = nucleo::AlmacenJson::new(config.data_dir.join("sessions"));
    let almacen_sesiones = Arc::new(player::AlmacenSesiones::nuevo(almacen_json));
    if let Err(e) = almacen_sesiones.initialize().await {
        tracing::warn!("No se pudo leer el almacén de sesiones: {}", e);
    }
    let command_ids = Arc::new(RwLock::new(HashMap::new()));

    let (shard, sender) = crear_shard(&config.discord.token, &config.bot.status);
    let cache = Arc::new(crear_cache());
    let apagando = Arc::new(AtomicBool::new(false));

    let bot_id_u64: u64 = config.discord.client_id.parse().unwrap_or(0);
    let mut senders = HashMap::new();
    senders.insert(0, sender.clone());
    let songbird = player::SongbirdDriver::create_songbird(Id::new(bot_id_u64), senders);
    let http = Arc::new(twilight_http::Client::new(config.discord.token.clone()));
    let (driver, filtro_voz, fragmentos) = bot::recepcion_voz::preparar_recepcion(
        player::SongbirdDriver::new(songbird),
        &config.voice,
        bot_id_u64,
        http.clone(),
    );
    let driver = Arc::new(driver);

    let presencia_cfg = player::ConfiguracionPresencia {
        leave_on_empty_ms: config.bot.leave_on_empty_ms,
        leave_on_finish_ms: config.bot.leave_on_finish_ms,
        allow_bot_input: config.voice.allow_bot_input,
        sound_bot_id: Some(config.voice.sound_bot_id.clone()),
        bot_user_id: config.discord.client_id.clone(),
    };
    let gestor_presencia = Arc::new(player::GestorPresencia::nuevo(presencia_cfg));

    let ytdlp_settings = bot::binarios::configurar_ytdlp(&config);
    let ytdlp_args = medios::base_args(&ytdlp_settings, None, None);
    let prov_restauracion = Arc::new(ProveedorRestauracionReal::nuevo(
        cache.clone(),
        Some(driver.clone()),
        ytdlp_settings.clone(),
        ytdlp_args.clone(),
    ));

    let hooks = Arc::new(HooksBotReal {
        gestor_colas: gestor_colas.clone(),
        almacen_sesiones: almacen_sesiones.clone(),
        command_ids: command_ids.clone(),
        token: config.discord.token.clone(),
        client_id: config.discord.client_id.clone(),
        guild_id: gid_opt,
        proveedor_restauracion: prov_restauracion,
    });

    let legacy_languages = Arc::new(nucleo::LegacyLanguages::cargar(&config.root));
    let sb_client = bot::datos::ClienteSupabaseRest::desde_config(&config.supabase).map(Arc::new);
    let gestor_favoritos = Some(Arc::new(bot::datos::GestorFavoritos::nuevo(
        sb_client.clone(),
    )));
    let gestor_listas = Some(Arc::new(bot::datos::GestorListas::nuevo(sb_client)));
    let ctx = Arc::new(bot::comandos::Contexto {
        idiomas: idiomas.clone(),
        default_language: config.bot.default_language.clone(),
        gestor_colas: gestor_colas.clone(),
        almacen_sesiones: almacen_sesiones.clone(),
        config_bot: config.bot.clone(),
        command_ids,
        client_id: config.discord.client_id.clone(),
        token: config.discord.token.clone(),
        root: config.root.clone(),
        data_dir: config.data_dir.clone(),
        legacy_languages: legacy_languages.clone(),
        gestor_listas,
        gestor_favoritos,
        driver: Some(driver.clone()),
        ytdlp_settings: ytdlp_settings.clone(),
        ytdlp_args,
    });
    let servicios_api = bot::api::ServiciosApiBot::iniciar(
        Arc::new(config.clone()),
        cache.clone(),
        Some(http.clone()),
        Some(driver.clone()),
        gestor_colas.clone(),
        Some(exportador_csv.clone()),
        ytdlp_settings,
        bot::api::proveedor_control::ConfiguracionControles {
            idiomas: idiomas.clone(),
            legacy: legacy_languages.clone(),
            data_dir: config.data_dir.clone(),
            max_volumen: config.bot.max_volume as u8,
        },
    )
    .await
    .expect("Fallo al inicializar servicios API");
    let _voz =
        bot::comandos_voz::iniciar(fragmentos, &config, rutas_binarios.libvosk, &servicios_api);

    let cliente_panel = Arc::new(bot::panel::ClientePanelHttp::nuevo(http.clone()));
    let gestor_paneles = Arc::new(
        bot::panel::GestorPaneles::nuevo(cliente_panel, idiomas.clone())
            .con_contexto_api(servicios_api.contexto.clone()),
    );

    let latencia = Arc::new(AtomicU64::new(0));
    let router = Arc::new(
        RouterInteraccionesReal::nuevo(ctx, Some(http))
            .con_recursos(
                gestor_paneles.clone(),
                Some(cache.clone()),
                Some(driver.clone()),
            )
            .con_latencia(latencia.clone())
            .con_proveedor_musica(servicios_api.proveedor_real.clone()),
    );

    let acciones_voz = Arc::new(AccionesVozBot::nuevo(
        gestor_colas.clone(),
        gestor_paneles,
        Some(driver.clone()),
        idiomas,
        config.data_dir.clone(),
        legacy_languages,
    ));

    let despachador = Arc::new(DespachadorEventos {
        cache,
        ready_hecho: Arc::new(AtomicBool::new(false)),
        contador_ready: Arc::new(AtomicUsize::new(0)),
        hooks_ready: hooks,
        router,
        driver: Some(driver.clone()),
        gestor_presencia: Some(gestor_presencia.clone()),
        acciones_voz: Some(acciones_voz),
        filtro_voz: Some(filtro_voz),
        latencia,
    });

    let pasos_apagado = PasosApagadoReal {
        apagando: apagando.clone(),
        gestor_colas: Some(gestor_colas),
        almacen_sesiones: Some(almacen_sesiones),
        exportador_csv: Some(exportador_csv),
        driver: Some(driver),
        sender: Some(sender),
    };

    let tarea_gateway = tokio::spawn(ejecutar_bucle_eventos(shard, despachador, apagando));
    let codigo = supervisar_gateway(tarea_gateway, servicios_api, &pasos_apagado).await;
    std::process::exit(codigo);
}
