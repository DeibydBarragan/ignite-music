//! Hub de Server-Sent Events (SSE) por servidor (Spec S-28).

pub use super::sse_stream::{construir_respuesta_sse, SseStream};
use super::{contexto::EstadoApiContexto, sse_stream::codificar_evento};
use axum::body::Bytes;
use axum::response::Response;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::Notify;

pub const DEFAULT_HEARTBEAT_MS: u64 = 25_000;
pub const CAPACIDAD_CANAL_SSE: usize = 64;

pub type SuscriptoresMapa = HashMap<u64, Sender<Bytes>>;
pub type ServidoresMapa = HashMap<String, SuscriptoresMapa>;
pub type ClientesSse = Arc<RwLock<ServidoresMapa>>;

pub struct HubSse {
    heartbeat_ms: u64,
    clients: ClientesSse,
    siguiente_id: AtomicU64,
    cerrado: AtomicBool,
    shutdown_notify: Arc<Notify>,
}

impl Default for HubSse {
    fn default() -> Self {
        Self {
            heartbeat_ms: DEFAULT_HEARTBEAT_MS,
            clients: Arc::new(RwLock::new(HashMap::new())),
            siguiente_id: AtomicU64::new(1),
            cerrado: AtomicBool::new(false),
            shutdown_notify: Arc::new(Notify::new()),
        }
    }
}

impl HubSse {
    pub fn por_defecto() -> Arc<Self> {
        Self::nuevo(Duration::from_millis(DEFAULT_HEARTBEAT_MS))
    }

    pub fn nuevo(latido: Duration) -> Arc<Self> {
        let clients = Arc::new(RwLock::new(HashMap::new()));
        let shutdown_notify = Arc::new(Notify::new());
        let hub = Arc::new(Self {
            heartbeat_ms: latido.as_millis() as u64,
            clients: Arc::clone(&clients),
            siguiente_id: AtomicU64::new(1),
            cerrado: AtomicBool::new(false),
            shutdown_notify: Arc::clone(&shutdown_notify),
        });

        if !latido.is_zero() {
            let cl = Arc::clone(&clients);
            let notif = Arc::clone(&shutdown_notify);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(latido);
                interval.tick().await;
                loop {
                    tokio::select! {
                        _ = interval.tick() => { Self::difundir_latido_interno(&cl); }
                        _ = notif.notified() => break,
                    }
                }
            });
        }
        hub
    }

    pub fn heartbeat_ms(&self) -> u64 {
        self.heartbeat_ms
    }

    pub fn size(&self, guild_id: Option<&str>) -> usize {
        let map = self.clients.read().unwrap();
        match guild_id {
            Some(gid) => map.get(gid).map(|s| s.len()).unwrap_or(0),
            None => map.values().map(|s| s.len()).sum(),
        }
    }

    pub fn suscriptores_servidor(&self, guild_id: &str) -> usize {
        self.size(Some(guild_id))
    }

    pub fn tiene_servidor(&self, guild_id: &str) -> bool {
        self.clients.read().unwrap().contains_key(guild_id)
    }

    pub fn suscribir(self: &Arc<Self>, guild_id: &str) -> (u64, SseStream) {
        self.registrar_suscriptor(guild_id, None)
    }

    fn registrar_suscriptor(
        self: &Arc<Self>,
        guild_id: &str,
        inicial: Option<Bytes>,
    ) -> (u64, SseStream) {
        let (tx, rx) = mpsc::channel(CAPACIDAD_CANAL_SSE);
        let sub_id = self.siguiente_id.fetch_add(1, Ordering::SeqCst);
        {
            let mut map = self.clients.write().unwrap();
            if !self.cerrado.load(Ordering::Relaxed) {
                let _ = tx.try_send(Bytes::from_static(b": connected\n\n"));
                if let Some(inicial) = inicial {
                    let _ = tx.try_send(inicial);
                }
                map.entry(guild_id.to_string())
                    .or_default()
                    .insert(sub_id, tx);
            }
        }
        (
            sub_id,
            SseStream {
                hub: Arc::clone(self),
                guild_id: guild_id.to_string(),
                sub_id,
                rx,
            },
        )
    }

    pub fn suscribir_con_inicial(
        self: &Arc<Self>,
        guild_id: &str,
        estado_inicial: Value,
    ) -> Response {
        let inicial = codificar_evento("player.sync", &estado_inicial);
        let (_, stream) = self.registrar_suscriptor(guild_id, Some(inicial));
        construir_respuesta_sse(stream)
    }

    pub fn desuscribir(&self, guild_id: &str, sub_id: u64) {
        let mut map = self.clients.write().unwrap();
        if let Some(subs) = map.get_mut(guild_id) {
            subs.remove(&sub_id);
            if subs.is_empty() {
                map.remove(guild_id);
            }
        }
    }

    pub fn publicar(&self, guild_id: &str, evento: &str, data: &Value) -> usize {
        self.difundir_a_servidor(guild_id, codificar_evento(evento, data))
    }

    pub async fn publicar_sincronizacion_con<F, Fut>(&self, guild_id: &str, calc: F) -> usize
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Value>,
    {
        if self.suscriptores_servidor(guild_id) == 0 {
            return 0;
        }
        let data = calc().await;
        self.publicar(guild_id, "player.sync", &data)
    }

    pub async fn publicar_sincronizacion(&self, guild_id: &str, st: &EstadoApiContexto) -> usize {
        self.publicar_sincronizacion_con(guild_id, || async {
            crate::api::rutas::reproductor::calcular_estado_reproductor(st, guild_id).await
        })
        .await
    }

    pub fn publicar_voz_log(&self, guild_id: &str, entry: &Value) -> usize {
        if self.suscriptores_servidor(guild_id) == 0 {
            return 0;
        }
        self.publicar(guild_id, "voice.log", entry)
    }

    fn difundir_a_servidor(&self, guild_id: &str, payload: Bytes) -> usize {
        let mut entregados = 0;
        let mut map = self.clients.write().unwrap();
        if let Some(subs) = map.get_mut(guild_id) {
            subs.retain(|_, tx| match tx.try_send(payload.clone()) {
                Ok(_) => {
                    entregados += 1;
                    true
                }
                Err(_) => false,
            });
            if subs.is_empty() {
                map.remove(guild_id);
            }
        }
        entregados
    }

    pub fn enviar_latidos(&self) -> usize {
        Self::difundir_latido_interno(&self.clients)
    }

    fn difundir_latido_interno(clients: &ClientesSse) -> usize {
        let payload = Bytes::from_static(b": heartbeat\n\n");
        let mut entregados = 0;
        let mut vacios = Vec::new();
        let mut map = clients.write().unwrap();
        for (gid, subs) in map.iter_mut() {
            subs.retain(|_, tx| match tx.try_send(payload.clone()) {
                Ok(_) => {
                    entregados += 1;
                    true
                }
                Err(_) => false,
            });
            if subs.is_empty() {
                vacios.push(gid.clone());
            }
        }
        for gid in vacios {
            map.remove(&gid);
        }
        entregados
    }

    pub fn cerrar_servidor(&self, guild_id: &str) {
        let mut map = self.clients.write().unwrap();
        map.remove(guild_id);
    }

    pub fn cerrar(&self) {
        let mut map = self.clients.write().unwrap();
        self.cerrado.store(true, Ordering::Relaxed);
        map.clear();
        self.shutdown_notify.notify_one();
    }
}

impl voice::PublicadorRegistroVoz for HubSse {
    fn publicar(&self, guild_id: &str, evento: &str, entrada: &voice::EntradaRegistroVoz) {
        if let Ok(val) = serde_json::to_value(entrada) {
            self.publicar(guild_id, evento, &val);
        }
    }
}
