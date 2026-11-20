use medios::Cancion;
use nucleo::AlmacenJson;
use player::{
    guardar_todas_las_sesiones, restaurar_sesiones, AlmacenSesiones, Cola, ElementoCola,
    GestorColas, ModoRepeticion, ProveedorRestauracion, MAX_AGE_MS,
};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tempfile::tempdir;

fn cancion_prueba(id: &str, fuente: &str, duracion: u64, req: Option<&str>) -> ElementoCola {
    let mut elem = ElementoCola::nuevo(
        Cancion {
            fuente: fuente.to_string(),
            id: id.to_string(),
            nombre: format!("Cancion {id}"),
            artista: "Artista".to_string(),
            url: format!("https://example.com/{id}"),
            stream_url: None,
            es_directo: false,
            duracion,
            miniatura: None,
            autor_url: None,
            edad_restringida: false,
            es_letra: false,
        },
        req.map(|r| r.to_string()),
    );
    elem.requester_name = req.map(|r| format!("User {r}"));
    elem
}

struct MockProv {
    servidores: Vec<String>,
    canales_voz: Vec<String>,
    oyentes: usize,
    arrancados: Mutex<Vec<(String, Option<u64>)>>,
}

#[async_trait::async_trait]
impl ProveedorRestauracion for MockProv {
    fn tiene_servidor(&self, g: &str) -> bool {
        self.servidores.iter().any(|s| s == g)
    }
    async fn canal_voz_valido(&self, _g: &str, c: &str) -> bool {
        self.canales_voz.iter().any(|v| v == c)
    }
    async fn canal_texto_valido(&self, _g: &str, _c: &str) -> bool {
        true
    }
    async fn contar_oyentes(&self, _g: &str, _c: &str) -> usize {
        self.oyentes
    }
    async fn arrancar_reproduccion(
        &self,
        cola: &mut Cola,
        _v: &str,
        seek: Option<u64>,
    ) -> Result<(), String> {
        self.arrancados
            .lock()
            .unwrap()
            .push((cola.guild_id.clone(), seek));
        Ok(())
    }
}

#[tokio::test]
async fn test_sesiones_roundtrip_sin_red() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let mut cola = Cola::nueva("111111111111111111");
    cola.canal_voz = Some("444444444444444444".into());
    cola.canal_texto = Some("555555555555555555".into());
    cola.posicion_segundos = 61;
    cola.volumen = 55;
    cola.modo_repeticion = ModoRepeticion::Cola;
    cola.filtros = vec!["bassboost".into()];
    cola.canciones = vec![
        cancion_prueba("vid1", "youtube", 210, Some("222222222222222222")),
        cancion_prueba("vid2", "spotify", 220, Some("222222222222222222")),
        cancion_prueba("vid3", "youtube", 230, Some("bot")),
        cancion_prueba("vid4", "desconocido", 150, Some("222222222222222222")),
    ];

    assert!(store.guardar(&cola).await.unwrap());
    let data = store.cargar("111111111111111111").unwrap();
    assert_eq!(data.songs.len(), 3);
    assert_eq!(data.position, 61);
    assert_eq!(data.filters, vec!["bassboost"]);

    let reconstruidas = store.reconstruir_canciones(&data);
    assert_eq!(reconstruidas.len(), 3);
    assert_eq!(reconstruidas[0].cancion.fuente, "youtube");
    assert_eq!(reconstruidas[1].cancion.fuente, "spotify");
    assert_eq!(
        reconstruidas[0].requester_id.as_deref(),
        Some("222222222222222222")
    );
}

#[tokio::test]
async fn test_sesiones_viejas_o_vacias_se_ignoran() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let mut cola = Cola::nueva("guild1");
    cola.canciones = vec![cancion_prueba("vid1", "youtube", 180, None)];
    assert!(store.guardar(&cola).await.unwrap());

    let data = store.cargar("guild1").unwrap();
    let saved = data.saved_at;
    assert!(store.cargar_con_tiempo("guild1", saved).is_some());
    assert!(store
        .cargar_con_tiempo("guild1", saved + MAX_AGE_MS + 1000)
        .is_none());

    cola.canciones.clear();
    assert!(!store.guardar(&cola).await.unwrap());
    assert!(store.cargar_con_tiempo("guild1", saved).is_none());
    assert!(store.guild_ids().is_empty());
}

#[tokio::test]
async fn test_sesiones_paridad_leer_archivos_de_node() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest.join("tests").join("data").join("sesiones");
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(&fixture_dir));
    store.initialize().await.unwrap();

    let normal = store.cargar_con_tiempo("guild_normal", 0).unwrap();
    assert_eq!(normal.voice_channel_id.as_deref(), Some("voice_channel_1"));
    assert_eq!(normal.text_channel_id.as_deref(), Some("text_channel_1"));
    assert_eq!(normal.songs.len(), 3);
    assert_eq!(normal.volume, 75);
    assert_eq!(normal.position, 45);
    assert_eq!(normal.filters, vec!["bassboost", "nightcore"]);

    assert!(store.cargar_con_tiempo("guild_empty_songs", 0).is_none());
    let exp_data = store.cargar_con_tiempo("guild_expired", 0).unwrap();
    assert!(store
        .cargar_con_tiempo("guild_expired", exp_data.saved_at + MAX_AGE_MS + 1000)
        .is_none());
}

#[tokio::test]
async fn test_cancion_plugin_desconocido_se_descarta() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest.join("tests").join("data").join("sesiones");
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(&fixture_dir));
    store.initialize().await.unwrap();

    let data = store.cargar_con_tiempo("guild_unknown_plugin", 0).unwrap();
    let canciones = store.reconstruir_canciones(&data);
    assert_eq!(canciones.len(), 1);
    assert_eq!(canciones[0].cancion.id, "vid0001");
}

#[tokio::test]
async fn test_restaurar_sesiones_logica_y_seek() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let gestor = GestorColas::nuevo();

    let mut cola = Cola::nueva("guild_seek");
    cola.canal_voz = Some("voice_seek".into());
    cola.posicion_segundos = 50;
    cola.canciones = vec![cancion_prueba("v1", "youtube", 200, None)];
    store.guardar(&cola).await.unwrap();

    let mut cola_no_seek = Cola::nueva("guild_no_seek");
    cola_no_seek.canal_voz = Some("voice_no_seek".into());
    cola_no_seek.posicion_segundos = 50;
    cola_no_seek.canciones = vec![cancion_prueba("v2", "youtube", 40, None)];
    store.guardar(&cola_no_seek).await.unwrap();

    let prov_sin_oyentes = MockProv {
        servidores: vec!["guild_seek".into()],
        canales_voz: vec!["voice_seek".into()],
        oyentes: 0,
        arrancados: Mutex::new(vec![]),
    };
    assert_eq!(
        restaurar_sesiones(&gestor, &store, &prov_sin_oyentes).await,
        0
    );
    assert!(store.cargar_con_tiempo("guild_seek", 0).is_none());

    store.guardar(&cola).await.unwrap();
    let prov_ok = MockProv {
        servidores: vec!["guild_seek".into(), "guild_no_seek".into()],
        canales_voz: vec!["voice_seek".into(), "voice_no_seek".into()],
        oyentes: 2,
        arrancados: Mutex::new(vec![]),
    };
    let restauradas = restaurar_sesiones(&gestor, &store, &prov_ok).await;
    assert_eq!(restauradas, 2);

    let arrancados = prov_ok.arrancados.lock().unwrap().clone();
    assert!(arrancados.contains(&("guild_seek".into(), Some(50))));
    assert!(arrancados.contains(&("guild_no_seek".into(), None)));

    assert_eq!(guardar_todas_las_sesiones(&gestor, &store).await, 2);
}

#[tokio::test]
async fn test_sesiones_paridad_escritura_rust_leida_por_node() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let mut cola = Cola::nueva("rust_node_guild");
    cola.canal_voz = Some("voice_r1".into());
    cola.canal_texto = Some("text_r1".into());
    cola.posicion_segundos = 33;
    cola.volumen = 90;
    cola.canciones = vec![cancion_prueba("v_rust", "youtube", 195, Some("user_99"))];
    store.guardar(&cola).await.unwrap();
    store.drain().await;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let script = root.join("scripts").join("sesiones-paridad-ref.js");

    let output = Command::new("node")
        .arg(&script)
        .arg("--leer")
        .arg(tmp.path())
        .output()
        .expect("Fallo al ejecutar node para paridad");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["keys"], serde_json::json!(["rust_node_guild"]));
    let val = &parsed["values"]["rust_node_guild"];
    assert_eq!(val["voiceChannelId"], "voice_r1");
    assert_eq!(val["position"], 33);
    assert_eq!(val["volume"], 90);
    assert_eq!(val["songs"][0]["id"], "v_rust");
    assert_eq!(val["songs"][0]["plugin"], "youtube");
}

fn copiar_fixtures() -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    let origen = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("sesiones");
    for entrada in std::fs::read_dir(origen).unwrap() {
        let entrada = entrada.unwrap();
        std::fs::copy(entrada.path(), tmp.path().join(entrada.file_name())).unwrap();
    }
    tmp
}

/// Pone savedAt en el momento actual para que la sesión del fixture no venza.
fn fijar_saved_at(ruta: &std::path::Path) {
    let texto = std::fs::read_to_string(ruta).unwrap();
    let mut val: serde_json::Value = serde_json::from_str(&texto).unwrap();
    val["savedAt"] = serde_json::json!(player::ahora_ms());
    std::fs::write(ruta, val.to_string()).unwrap();
}

fn prov_con(servidores: &[&str], canales: &[&str], oyentes: usize) -> MockProv {
    MockProv {
        servidores: servidores.iter().map(|s| s.to_string()).collect(),
        canales_voz: canales.iter().map(|s| s.to_string()).collect(),
        oyentes,
        arrancados: Mutex::new(vec![]),
    }
}

#[tokio::test]
async fn test_restaura_sesion_escrita_por_node_con_todos_los_tipos() {
    let tmp = copiar_fixtures();
    fijar_saved_at(&tmp.path().join("guild_mixta.json"));
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let gestor = GestorColas::nuevo();
    let prov = prov_con(&["guild_mixta"], &["voice_mixta"], 1);

    assert_eq!(restaurar_sesiones(&gestor, &store, &prov).await, 1);

    let cola_lock = gestor.obtener_cola("guild_mixta").await;
    let cola = cola_lock.lock().await;
    let ids: Vec<&str> = cola
        .canciones
        .iter()
        .map(|e| e.cancion.id.as_str())
        .collect();
    assert_eq!(ids, vec!["mx1", "mx2", "mx3", "mx4"]);
    let fuentes: Vec<&str> = cola
        .canciones
        .iter()
        .map(|e| e.cancion.fuente.as_str())
        .collect();
    assert_eq!(fuentes, vec!["youtube", "spotify", "direct", "generic"]);
    assert_eq!(cola.volumen, 0);
    assert!(cola.pausada);
    assert_eq!(cola.filtros, vec!["bassboost", "nightcore"]);
    assert_eq!(cola.posicion_segundos, 40);
    assert_eq!(cola.canal_voz.as_deref(), Some("voice_mixta"));
    assert_eq!(cola.canciones[0].requester_id.as_deref(), Some("user_111"));
    drop(cola);

    assert_eq!(
        *prov.arrancados.lock().unwrap(),
        vec![("guild_mixta".to_string(), Some(40))]
    );
}

#[tokio::test]
async fn test_sin_oyentes_no_arranca_audio() {
    let tmp = copiar_fixtures();
    fijar_saved_at(&tmp.path().join("guild_mixta.json"));
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let gestor = GestorColas::nuevo();
    let prov = prov_con(&["guild_mixta"], &["voice_mixta"], 0);

    assert_eq!(restaurar_sesiones(&gestor, &store, &prov).await, 0);
    assert!(prov.arrancados.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_sesion_vacia_o_vencida_no_arranca_audio() {
    let tmp = copiar_fixtures();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let gestor = GestorColas::nuevo();
    let prov = prov_con(
        &["guild_empty_songs", "guild_expired"],
        &["voice_channel_1"],
        1,
    );

    assert_eq!(restaurar_sesiones(&gestor, &store, &prov).await, 0);
    assert!(prov.arrancados.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_restaurar_sin_initialize_lee_el_disco() {
    let tmp = copiar_fixtures();
    fijar_saved_at(&tmp.path().join("guild_mixta.json"));
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let gestor = GestorColas::nuevo();
    let prov = prov_con(&["guild_mixta"], &["voice_mixta"], 1);

    assert_eq!(restaurar_sesiones(&gestor, &store, &prov).await, 1);
}

#[tokio::test]
async fn test_primer_guardado_conserva_sesiones_del_disco() {
    let tmp = copiar_fixtures();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let mut cola = Cola::nueva("guild_nuevo");
    cola.canal_voz = Some("voice_nuevo".into());
    cola.canciones = vec![cancion_prueba("n1", "youtube", 180, None)];

    assert!(store.guardar(&cola).await.unwrap());
    store.drain().await;

    let ids = store.guild_ids();
    assert!(ids.contains(&"guild_mixta".to_string()));
    assert!(ids.contains(&"guild_nuevo".to_string()));
    assert!(tmp.path().join("guild_mixta.json").exists());
}

struct ProvFalla;

#[async_trait::async_trait]
impl ProveedorRestauracion for ProvFalla {
    fn tiene_servidor(&self, _g: &str) -> bool {
        true
    }
    async fn canal_voz_valido(&self, _g: &str, _c: &str) -> bool {
        true
    }
    async fn canal_texto_valido(&self, _g: &str, _c: &str) -> bool {
        true
    }
    async fn contar_oyentes(&self, _g: &str, _c: &str) -> usize {
        1
    }
    async fn arrancar_reproduccion(
        &self,
        _cola: &mut Cola,
        _v: &str,
        _seek: Option<u64>,
    ) -> Result<(), String> {
        Err("yt-dlp fallo transitorio".into())
    }
}

fn sesion_para_guild(guild: &str) -> Cola {
    let mut cola = Cola::nueva(guild);
    cola.canal_voz = Some("voice_falla".into());
    cola.posicion_segundos = 40;
    cola.canciones = vec![cancion_prueba("f1", "youtube", 200, None)];
    cola
}

#[tokio::test]
async fn test_arranque_fallido_conserva_sesion_y_reintenta_al_reiniciar() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    store
        .guardar(&sesion_para_guild("guild_falla"))
        .await
        .unwrap();
    store.drain().await;

    // (a) arranque fallido: el archivo sigue en disco.
    let gestor = GestorColas::nuevo();
    assert_eq!(restaurar_sesiones(&gestor, &store, &ProvFalla).await, 0);
    assert!(store.cargar_con_tiempo("guild_falla", 0).is_some());
    assert!(tmp.path().join("guild_falla.json").exists());

    // (c) reinicio con el arranque funcionando: restaura la sesión conservada.
    let reiniciado = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let prov_ok = MockProv {
        servidores: vec!["guild_falla".into()],
        canales_voz: vec!["voice_falla".into()],
        oyentes: 1,
        arrancados: Mutex::new(vec![]),
    };
    let gestor2 = GestorColas::nuevo();
    assert_eq!(restaurar_sesiones(&gestor2, &reiniciado, &prov_ok).await, 1);
    assert_eq!(
        *prov_ok.arrancados.lock().unwrap(),
        vec![("guild_falla".to_string(), Some(40))]
    );
}

#[tokio::test]
async fn test_arranque_exitoso_no_borra_sesion_como_antes() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    store.guardar(&sesion_para_guild("guild_ok")).await.unwrap();
    store.drain().await;

    // (b) arranque exitoso: el borrado no cambia; la sesión se sobrescribe al guardar.
    let prov_ok = MockProv {
        servidores: vec!["guild_ok".into()],
        canales_voz: vec!["voice_falla".into()],
        oyentes: 1,
        arrancados: Mutex::new(vec![]),
    };
    let gestor = GestorColas::nuevo();
    assert_eq!(restaurar_sesiones(&gestor, &store, &prov_ok).await, 1);
    assert!(store.cargar_con_tiempo("guild_ok", 0).is_some());
}

#[tokio::test]
async fn test_sesion_conservada_caducada_se_descarta() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    store
        .guardar(&sesion_para_guild("guild_vieja"))
        .await
        .unwrap();
    store.drain().await;

    let ruta = tmp.path().join("guild_vieja.json");
    let texto = std::fs::read_to_string(&ruta).unwrap();
    let mut val: serde_json::Value = serde_json::from_str(&texto).unwrap();
    val["savedAt"] = serde_json::json!(player::ahora_ms() - MAX_AGE_MS - 1000);
    std::fs::write(&ruta, val.to_string()).unwrap();

    let reiniciado = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    let gestor = GestorColas::nuevo();
    assert_eq!(
        restaurar_sesiones(&gestor, &reiniciado, &ProvFalla).await,
        0
    );
    assert!(!ruta.exists());
}

struct ProvFallaTrasUnirse {
    salidas: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl ProveedorRestauracion for ProvFallaTrasUnirse {
    fn tiene_servidor(&self, _g: &str) -> bool {
        true
    }
    async fn canal_voz_valido(&self, _g: &str, _c: &str) -> bool {
        true
    }
    async fn canal_texto_valido(&self, _g: &str, _c: &str) -> bool {
        true
    }
    async fn contar_oyentes(&self, _g: &str, _c: &str) -> usize {
        1
    }
    async fn arrancar_reproduccion(
        &self,
        _cola: &mut Cola,
        _v: &str,
        _seek: Option<u64>,
    ) -> Result<(), String> {
        Err("la pista falló después de unirse al canal".into())
    }
    async fn salir_de_voz(&self, guild_id: &str) {
        self.salidas.lock().unwrap().push(guild_id.to_string());
    }
}

#[tokio::test]
async fn test_arranque_fallido_tras_unirse_sale_del_canal_y_conserva_sesion() {
    let tmp = tempdir().unwrap();
    let store = AlmacenSesiones::nuevo(AlmacenJson::new(tmp.path()));
    store
        .guardar(&sesion_para_guild("guild_union"))
        .await
        .unwrap();
    store.drain().await;

    let prov = ProvFallaTrasUnirse {
        salidas: Mutex::new(vec![]),
    };
    let gestor = GestorColas::nuevo();
    assert_eq!(restaurar_sesiones(&gestor, &store, &prov).await, 0);
    assert_eq!(
        *prov.salidas.lock().unwrap(),
        vec!["guild_union".to_string()]
    );
    assert!(store.cargar_con_tiempo("guild_union", 0).is_some());
    assert!(tmp.path().join("guild_union.json").exists());
}
