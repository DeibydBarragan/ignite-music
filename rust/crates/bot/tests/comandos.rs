//! Pruebas para comandos de barra, paridad con Node y registro (Spec S-22).

use bot::comandos::definiciones::todos_los_cuerpos;
use bot::comandos::registro::calcular_hash_despliegue;
use bot::comandos::{categoria_comando, tiene_autocompletado, Contexto};
use bot::interacciones::RouterInteraccionesReal;
use nucleo::Idiomas;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use twilight_model::application::interaction::Interaction;

fn root_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn fixture_idiomas() -> Idiomas {
    let lang_dir = root_dir().join("languages");
    Idiomas::cargar(&lang_dir, Some("en")).expect("Failed to load languages")
}

#[test]
fn test_paridad_con_node() {
    let idiomas = fixture_idiomas();
    let rust_cuerpos = todos_los_cuerpos(&idiomas, 100);

    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/comandos-node.json");
    let json_node: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&fixture_path).expect("Fixture comandos-node.json not found"),
    )
    .expect("Failed to parse fixture JSON");

    let json_rust = serde_json::to_value(&rust_cuerpos).expect("Failed to serialize Rust bodies");

    assert_eq!(
        json_rust, json_node,
        "El JSON de comandos de Rust no coincide con el de Node"
    );
}

#[test]
fn test_limites_discord() {
    let idiomas = fixture_idiomas();
    let cuerpos = todos_los_cuerpos(&idiomas, 100);

    assert_eq!(cuerpos.len(), 25);
    let mut nombres = std::collections::HashSet::new();

    for cmd in &cuerpos {
        assert!(!cmd.name.is_empty() && cmd.name.len() <= 32);
        assert!(cmd
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'));
        assert!(
            nombres.insert(cmd.name.clone()),
            "Nombre duplicado: {}",
            cmd.name
        );

        assert!(!cmd.description.is_empty() && cmd.description.len() <= 100);
        assert_eq!(cmd.contexts, vec![0]);

        for desc in cmd.description_localizations.values() {
            assert!(desc.chars().count() <= 100);
        }

        assert!(cmd.options.len() <= 25);
        for opt in &cmd.options {
            assert!(!opt.name.is_empty() && opt.name.len() <= 32);
            assert!(!opt.description.is_empty() && opt.description.chars().count() <= 100);
            if let Some(choices) = &opt.choices {
                assert!(choices.len() <= 25);
                for ch in choices {
                    assert!(ch.name.chars().count() <= 100);
                    assert!(ch.value.chars().count() <= 100);
                }
            }
        }
    }
}

#[test]
fn test_categorias_y_autocompletado() {
    let idiomas = fixture_idiomas();
    let cuerpos = todos_los_cuerpos(&idiomas, 100);

    let mut conteo_categorias = HashMap::new();
    for cmd in &cuerpos {
        let cat = categoria_comando(&cmd.name).expect("Categoría válida requerida");
        *conteo_categorias.entry(cat).or_insert(0) += 1;
    }

    assert_eq!(conteo_categorias.get("playback"), Some(&17));
    assert_eq!(conteo_categorias.get("queue"), Some(&6));
    assert_eq!(conteo_categorias.get("settings"), Some(&2));

    assert!(tiene_autocompletado("play"));
    assert!(tiene_autocompletado("playnext"));
    assert!(!tiene_autocompletado("pause"));
    assert!(!tiene_autocompletado("clear"));
}

#[test]
fn test_permisos_language() {
    let idiomas = fixture_idiomas();
    let cuerpos = todos_los_cuerpos(&idiomas, 100);
    let lang_cmd = cuerpos
        .iter()
        .find(|c| c.name == "language")
        .expect("Comando language");
    assert_eq!(lang_cmd.default_member_permissions.as_deref(), Some("32"));
}

#[test]
fn test_hash_despliegue() {
    let idiomas = fixture_idiomas();
    let cuerpos = todos_los_cuerpos(&idiomas, 100);

    let hash1 = calcular_hash_despliegue(&cuerpos, "12345", Some("67890"));
    let hash2 = calcular_hash_despliegue(&cuerpos, "12345", Some("67890"));
    assert_eq!(hash1, hash2, "El hash debe ser determinista");

    let hash_dif_guild = calcular_hash_despliegue(&cuerpos, "12345", None);
    assert_ne!(
        hash1, hash_dif_guild,
        "El hash debe variar si cambia el guild"
    );

    let mut cuerpos_mod = cuerpos.clone();
    cuerpos_mod[0].description = "Nueva descripción modificada".to_string();
    let hash_mod = calcular_hash_despliegue(&cuerpos_mod, "12345", Some("67890"));
    assert_ne!(hash1, hash_mod, "El hash debe variar si cambia el cuerpo");
}

fn crear_contexto_prueba() -> Contexto {
    let idiomas = fixture_idiomas();
    let gestor_colas = Arc::new(player::GestorColas::nuevo());
    let data_dir = root_dir().join("data");
    let almacen_json = nucleo::AlmacenJson::new(data_dir.join("sessions"));
    let almacen_sesiones = Arc::new(player::AlmacenSesiones::nuevo(almacen_json));
    let config_bot = nucleo::BotConfig {
        status: "Testing".into(),
        color: 0x5865F2,
        support_server: "".into(),
        website: "".into(),
        default_language: "en".into(),
        default_volume: 100,
        max_volume: 100,
        max_queue_size: 100,
        max_playlist_size: 100,
        leave_on_empty_ms: 30000,
        leave_on_finish_ms: 30000,
        panel_refresh_ms: 1000,
    };

    Contexto {
        idiomas: Arc::new(idiomas),
        default_language: "en".into(),
        gestor_colas,
        almacen_sesiones,
        config_bot,
        command_ids: Arc::new(RwLock::new(HashMap::new())),
        client_id: "test".into(),
        token: "test".into(),
        root: root_dir(),
        data_dir: data_dir.clone(),
        legacy_languages: Arc::new(nucleo::LegacyLanguages::cargar(&root_dir())),
        gestor_listas: None,
        gestor_favoritos: None,
        driver: None,
        ytdlp_settings: medios::ytdlp_args::YtDlpSettings::default(),
        ytdlp_args: Vec::new(),
    }
}

#[tokio::test]
async fn salto_y_anterior_rechazados_no_detienen_la_pista() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use twilight_model::id::Id;
    let mut ctx = crear_contexto_prueba();
    let driver = Arc::new(player::SongbirdDriver::new(
        player::SongbirdDriver::create_songbird(Id::new(1), HashMap::new()),
    ));
    let paradas = Arc::new(AtomicUsize::new(0));
    let observador = paradas.clone();
    driver.fijar_oyente_pista(Arc::new(move |_, evento, _| {
        if evento == "stop" {
            observador.fetch_add(1, Ordering::SeqCst);
        }
    }));
    ctx.driver = Some(driver);
    let cola = ctx.gestor_colas.obtener_cola("1").await;
    cola.lock()
        .await
        .canciones
        .push(player::ElementoCola::nuevo(
            medios::Cancion {
                fuente: "youtube".into(),
                id: "a".into(),
                nombre: "Canción".into(),
                artista: "Artista".into(),
                url: "https://youtube.com/watch?v=abcdefghijk".into(),
                stream_url: None,
                es_directo: false,
                duracion: 100,
                miniatura: None,
                autor_url: None,
                edad_restringida: false,
                es_letra: false,
            },
            None,
        ));
    bot::comandos::acciones::ejecutar_saltar(&ctx, "1", "en")
        .await
        .unwrap();
    bot::comandos::acciones::ejecutar_anterior(&ctx, "1", "en")
        .await
        .unwrap();
    assert_eq!(paradas.load(Ordering::SeqCst), 0);
    assert_eq!(cola.lock().await.canciones.len(), 1);
}

#[tokio::test]
async fn control_api_revierte_toggle_y_seek_si_falta_handle() {
    use bot::api::proveedor_control::ejecutar_control_api;
    use bot::api::proveedor_real::ProveedorApiReal;
    use twilight_model::id::Id;
    let ctx = crear_contexto_prueba();
    let driver = Arc::new(player::SongbirdDriver::new(
        player::SongbirdDriver::create_songbird(Id::new(1), HashMap::new()),
    ));
    let proveedor = ProveedorApiReal::nuevo(
        Arc::new(bot::cliente::crear_cache()),
        None,
        Some(driver),
        ctx.gestor_colas.clone(),
        None,
        1,
        Default::default(),
        vec![],
        None,
        100,
        100,
    );
    let cola = ctx.gestor_colas.obtener_cola("1").await;
    {
        let mut cola = cola.lock().await;
        cola.canciones.push(player::ElementoCola::nuevo(
            medios::Cancion {
                fuente: "youtube".into(),
                id: "a".into(),
                nombre: "Test".into(),
                artista: "Artista".into(),
                url: "https://youtube.com/watch?v=abcdefghijk".into(),
                stream_url: None,
                es_directo: false,
                duracion: 100,
                miniatura: None,
                autor_url: None,
                edad_restringida: false,
                es_letra: false,
            },
            None,
        ));
        cola.posicion_segundos = 7;
    }
    let error = ejecutar_control_api(&proveedor, "1", "toggle", None)
        .await
        .unwrap_err();
    assert_eq!(error.1, "CONTROL_FAILED");
    assert!(!cola.lock().await.pausada);
    let error = ejecutar_control_api(&proveedor, "1", "seek", Some(&serde_json::json!(30)))
        .await
        .unwrap_err();
    assert_eq!(error.1, "CONTROL_FAILED");
    assert_eq!(cola.lock().await.posicion_segundos, 7);
}

#[tokio::test]
async fn test_enrutado_comando_desconocido() {
    let ctx = Arc::new(crear_contexto_prueba());
    let router = RouterInteraccionesReal::nuevo(ctx, None);

    let interaction_json = serde_json::json!({
        "id": "100000000000000001",
        "application_id": "100000000000000002",
        "type": 2, // ApplicationCommand
        "data": {
            "id": "100000000000000003",
            "name": "comando_inexistente",
            "type": 1,
            "options": []
        },
        "guild_id": "100000000000000004",
        "token": "interaction_token",
        "authorizing_integration_owners": {},
        "version": 1
    });

    let interaction: Interaction = serde_json::from_value(interaction_json).unwrap();
    let resultado = router.enrutar(&interaction).await;

    match resultado {
        bot::interacciones::ResultadoEnrutado::Comando { respuesta, .. } => {
            assert!(
                respuesta.is_none(),
                "Comando desconocido debe devolver None"
            );
        }
        other => panic!("Esperado ResultadoEnrutado::Comando, obtenido {:?}", other),
    }
}
