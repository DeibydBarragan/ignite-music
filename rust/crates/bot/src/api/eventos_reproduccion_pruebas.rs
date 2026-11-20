//! Pruebas del listener de reproducci�n: historial por arranque, reinicios y fallos.

use super::*;
use std::sync::Mutex;
use std::time::Duration;
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::id::Id;

#[path = "eventos_reproduccion_pruebas_origenes.rs"]
mod origenes;

/// Doble del registrador: guarda (guild, título) por llamada y puede fallar a propósito.
#[derive(Default)]
struct RegistradorFalso {
    llamadas: Mutex<Vec<(String, String)>>,
    falla: bool,
}

impl RegistroHistorial for RegistradorFalso {
    async fn registrar(&self, guild_id: &str, input: &RegistroCancionInput) -> Result<(), String> {
        let titulo = input.title.clone().unwrap_or_default();
        self.llamadas
            .lock()
            .unwrap()
            .push((guild_id.into(), titulo));
        if self.falla {
            Err("fallo simulado".into())
        } else {
            Ok(())
        }
    }
}

fn cancion(nombre: &str) -> medios::Cancion {
    medios::Cancion {
        fuente: "youtube".into(),
        id: nombre.into(),
        nombre: nombre.into(),
        artista: "Artista".into(),
        url: format!("https://example.invalid/watch?v={nombre}"),
        stream_url: None,
        es_directo: false,
        duracion: 180,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    }
}

/// Driver con llamada en el guild 10, cola con el solicitante "42" y listener conectado.
/// Devuelve también el proveedor: el listener solo guarda un `Weak`, así que debe seguir vivo.
async fn montar(
    registrador: Arc<RegistradorFalso>,
) -> (
    Arc<player::SongbirdDriver>,
    Arc<player::GestorColas>,
    Arc<ProveedorApiReal>,
) {
    let shard = twilight_gateway::Shard::new(
        twilight_gateway::ShardId::ONE,
        "token-de-prueba".into(),
        twilight_gateway::Intents::GUILD_VOICE_STATES,
    );
    let songbird =
        player::SongbirdDriver::create_songbird(Id::new(1), [(0, shard.sender())].into());
    drop(shard);
    let driver = Arc::new(player::SongbirdDriver::new(songbird));
    driver.get_or_insert(10).unwrap();
    let colas = Arc::new(player::GestorColas::nuevo());
    let ajustes = medios::YtDlpSettings::default();
    let prov = Arc::new(ProveedorApiReal::nuevo(
        Arc::new(InMemoryCache::new()),
        None,
        Some(driver.clone()),
        colas.clone(),
        None,
        1,
        ajustes.clone(),
        Vec::new(),
        None,
        100,
        100,
    ));
    conectar(
        &prov,
        &HubSse::por_defecto(),
        &colas,
        "yt-dlp",
        &[],
        &ajustes,
        &[],
        Some(registrador),
        Arc::new(GestorDevMode::nuevo("no-existe.json".into())),
        Vec::new(),
    );
    (driver, colas, prov)
}

async fn encolar(colas: &player::GestorColas, nombres: &[&str]) {
    let cola = colas.obtener_cola("10").await;
    let mut cola = cola.lock().await;
    for nombre in nombres {
        cola.canciones.push(player::ElementoCola::nuevo(
            cancion(nombre),
            Some("42".into()),
        ));
    }
}

/// El listener corre en una tarea aparte: espera a `n` llamadas y deja pasar un margen para detectar extras.
async fn esperar(registrador: &RegistradorFalso, n: usize) {
    for _ in 0..100 {
        if registrador.llamadas.lock().unwrap().len() >= n {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(registrador.llamadas.lock().unwrap().len(), n);
}

fn titulos(registrador: &RegistradorFalso) -> Vec<String> {
    registrador
        .llamadas
        .lock()
        .unwrap()
        .iter()
        .map(|(_, t)| t.clone())
        .collect()
}

/// Arranque por comando (slash): reproducir.rs llama a iniciar_pista_con_settings.
#[test]
fn entrada_marca_modo_dev_del_solicitante() {
    let cancion = medios::Cancion {
        fuente: "youtube".into(),
        id: "abc".into(),
        nombre: "Tema".into(),
        artista: "Artista".into(),
        url: "https://example.invalid/watch?v=abc".into(),
        stream_url: None,
        es_directo: false,
        duracion: 180,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    };
    let elemento = player::ElementoCola::nuevo(cancion, Some("42".into()));
    let gestor = GestorDevMode::nuevo(std::path::PathBuf::from("no-existe.json"));
    let ids_dev = vec!["42".to_string()];
    assert!(entrada_historial(&elemento, &gestor, &ids_dev).dev_mode);
    assert!(!entrada_historial(&elemento, &gestor, &[]).dev_mode);
}
