use crate::presencia_modelos::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub use crate::presencia_modelos::*;

fn abortar(h: &mut Option<JoinHandle<()>>) {
    if let Some(handle) = h.take() {
        handle.abort();
    }
}

fn tarea_vacio<A: AccionesCanalVoz>(
    gestor: GestorPresencia,
    gid: String,
    cid: String,
    act: Arc<A>,
    ms: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        if gestor.contar_personas_gremio(&gid, &cid).await == 0 {
            if act.tiene_cola_activa(&gid).await {
                act.cerrar_panel_vacio(&gid).await;
                act.parar_cola(&gid).await;
            }
            if let Some(s) = gestor.gremios.lock().await.get_mut(&gid) {
                s.pausada_por_vacio = false;
                s.motivo_desconexion = Some(MotivoDesconexion::InactividadVacio);
                s.temporizador_vacio = None;
            }
            act.salir_canal(&gid, MotivoDesconexion::InactividadVacio)
                .await;
        }
    })
}

fn tarea_salida<A: AccionesCanalVoz>(
    gestor: GestorPresencia,
    gid: String,
    act: Arc<A>,
    ms: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        if !act.tiene_cola_activa(&gid).await {
            if let Some(s) = gestor.gremios.lock().await.get_mut(&gid) {
                s.motivo_desconexion = Some(MotivoDesconexion::InactividadFin);
                s.temporizador_salida = None;
            }
            act.salir_canal(&gid, MotivoDesconexion::InactividadFin)
                .await;
        }
    })
}

#[derive(Default)]
struct EstadoGremio {
    estados: HashMap<String, EstadoVoz>,
    canal_voz_bot: Option<String>,
    pausada_por_vacio: bool,
    motivo_desconexion: Option<MotivoDesconexion>,
    temporizador_vacio: Option<JoinHandle<()>>,
    temporizador_salida: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct GestorPresencia {
    config: Arc<ConfiguracionPresencia>,
    gremios: Arc<Mutex<HashMap<String, EstadoGremio>>>,
}

impl GestorPresencia {
    pub fn nuevo(config: ConfiguracionPresencia) -> Self {
        Self {
            config: Arc::new(config),
            gremios: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn fijar_canal_bot(&self, gid: &str, canal: Option<String>) {
        let mut map = self.gremios.lock().await;
        map.entry(gid.into()).or_default().canal_voz_bot = canal;
    }

    pub async fn motivo_desconexion(&self, gid: &str) -> Option<MotivoDesconexion> {
        let map = self.gremios.lock().await;
        map.get(gid).and_then(|g| g.motivo_desconexion)
    }

    pub async fn esta_pausada_por_vacio(&self, gid: &str) -> bool {
        let map = self.gremios.lock().await;
        map.get(gid).map(|g| g.pausada_por_vacio).unwrap_or(false)
    }

    pub async fn contar_personas_gremio(&self, gid: &str, canal: &str) -> usize {
        let map = self.gremios.lock().await;
        let Some(g) = map.get(gid) else { return 0 };
        let list: Vec<EstadoVoz> = g.estados.values().cloned().collect();
        contar_personas(&list, canal, &self.config)
    }

    pub async fn cancelar_salida(&self, gid: &str) {
        let mut map = self.gremios.lock().await;
        if let Some(g) = map.get_mut(gid) {
            abortar(&mut g.temporizador_salida);
        }
    }

    pub async fn cancelar_temporizador_vacio(&self, gid: &str) {
        let mut map = self.gremios.lock().await;
        if let Some(g) = map.get_mut(gid) {
            abortar(&mut g.temporizador_vacio);
        }
    }

    pub async fn programar_salida<A: AccionesCanalVoz>(&self, gid: &str, act: Arc<A>) {
        self.cancelar_salida(gid).await;
        let ms = self.config.leave_on_finish_ms;
        if ms == 0 {
            return;
        }
        let h = tarea_salida(self.clone(), gid.into(), act, ms);
        let mut map = self.gremios.lock().await;
        map.entry(gid.into()).or_default().temporizador_salida = Some(h);
    }

    pub async fn actualizar_voz<A: AccionesCanalVoz>(
        &self,
        gid: &str,
        estado: EstadoVoz,
        cola_pausada: bool,
        act: Arc<A>,
    ) {
        let (pausar, reanudar) = {
            let mut map = self.gremios.lock().await;
            let g = map.entry(gid.into()).or_default();
            if estado.user_id == self.config.bot_user_id {
                g.canal_voz_bot = estado.channel_id.clone();
            }
            if estado.channel_id.is_none() {
                g.estados.remove(&estado.user_id);
            } else {
                g.estados.insert(estado.user_id.clone(), estado);
            }
            let canal = match &g.canal_voz_bot {
                Some(c) => c.clone(),
                None => return,
            };
            let list: Vec<EstadoVoz> = g.estados.values().cloned().collect();
            let oyentes = contar_personas(&list, &canal, &self.config);

            if oyentes == 0 && g.temporizador_vacio.is_none() && self.config.leave_on_empty_ms > 0 {
                let ms = self.config.leave_on_empty_ms;
                g.temporizador_vacio = Some(tarea_vacio(
                    self.clone(),
                    gid.into(),
                    canal,
                    act.clone(),
                    ms,
                ));
                if !cola_pausada {
                    g.pausada_por_vacio = true;
                    (true, false)
                } else {
                    (false, false)
                }
            } else if oyentes > 0 && g.temporizador_vacio.is_some() {
                abortar(&mut g.temporizador_vacio);
                let r = g.pausada_por_vacio && cola_pausada;
                g.pausada_por_vacio = false;
                (false, r)
            } else {
                (false, false)
            }
        };

        if pausar {
            act.pausar_cola(gid).await;
            act.refrescar_panel(gid).await;
        } else if reanudar {
            act.reanudar_cola(gid).await;
            act.refrescar_panel(gid).await;
        }
    }

    pub async fn apagar<A: AccionesCanalVoz>(&self, act: Arc<A>) {
        let mut map = self.gremios.lock().await;
        for (gid, s) in map.iter_mut() {
            abortar(&mut s.temporizador_vacio);
            abortar(&mut s.temporizador_salida);
            if s.canal_voz_bot.is_some() {
                let (a, id) = (act.clone(), gid.clone());
                tokio::spawn(async move {
                    a.salir_canal(&id, MotivoDesconexion::InactividadFin).await;
                });
            }
        }
    }
}
