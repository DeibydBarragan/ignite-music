use super::dev_mode::GestorDevMode;
use super::proveedor_real::ProveedorApiReal;
use super::sse::HubSse;
use crate::datos::historial::RegistroCancionInput;
use medios::ytdlp_args::YtDlpSettings;
use std::sync::Arc;

/// Destino del historial de arranques. En producción es `GestorRanking`; las pruebas usan un doble.
pub(super) trait RegistroHistorial: Send + Sync + 'static {
    fn registrar(
        &self,
        guild_id: &str,
        input: &RegistroCancionInput,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send;
}

impl RegistroHistorial for crate::datos::GestorRanking {
    async fn registrar(&self, guild_id: &str, input: &RegistroCancionInput) -> Result<(), String> {
        self.registrar_reproduccion(guild_id, input, false).await;
        Ok(())
    }
}

/// Registro de historial de una pista que arranca. Modo dev según el solicitante, como POST /play.
fn entrada_historial(
    elemento: &player::ElementoCola,
    dev: &GestorDevMode,
    dev_ids: &[String],
) -> RegistroCancionInput {
    let cancion = &elemento.cancion;
    RegistroCancionInput {
        id: Some(cancion.id.clone()),
        title: Some(cancion.nombre.clone()),
        artist: Some(cancion.artista.clone()),
        album_art: cancion.miniatura.clone(),
        url: Some(cancion.url.clone()),
        duration: Some(cancion.duracion as u32),
        source: Some(cancion.fuente.clone()),
        user_id: elemento.requester_id.clone(),
        username: elemento.requester_name.clone(),
        dev_mode: elemento
            .requester_id
            .as_deref()
            .is_some_and(|uid| dev.es_dev_user(uid, dev_ids)),
    }
}

pub(super) async fn sincronizar_posicion(
    contexto: super::contexto::EstadoApiContexto,
    driver: Arc<player::SongbirdDriver>,
) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        for (gid, handle) in driver.active_track_handles() {
            let Ok(Ok(info)) =
                tokio::time::timeout(std::time::Duration::from_secs(1), handle.get_info()).await
            else {
                continue;
            };
            let gid_str = gid.to_string();
            let cola = contexto.gestor_colas.obtener_cola(&gid_str).await;
            let mut cola = cola.lock().await;
            if cola.canciones.is_empty()
                || !driver
                    .active_track(gid)
                    .is_some_and(|actual| actual.uuid() == handle.uuid())
            {
                continue;
            }
            cola.posicion_segundos = info.position.as_secs();
            drop(cola);
            contexto
                .sse
                .publicar_sincronizacion(&gid_str, &contexto)
                .await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn conectar<R: RegistroHistorial>(
    proveedor: &Arc<ProveedorApiReal>,
    hub_sse: &Arc<HubSse>,
    gestor_colas: &Arc<player::GestorColas>,
    ytdlp_bin: &str,
    ytdlp_args: &[String],
    ytdlp_settings: &YtDlpSettings,
    proxies: &[String],
    registrador: Option<Arc<R>>,
    dev_mode: Arc<GestorDevMode>,
    dev_user_ids: Vec<String>,
) {
    let Some(drv) = &proveedor.driver else { return };
    let hub_clone = hub_sse.clone();
    let colas_clone = gestor_colas.clone();
    let proveedor_cb = Arc::downgrade(proveedor);
    let ytdlp_bin_cb = ytdlp_bin.to_owned();
    let ytdlp_args_cb = ytdlp_args.to_vec();
    let ytdlp_settings_cb = ytdlp_settings.clone();
    let proxies_cb = proxies.to_vec();
    let dev_mode_cb = dev_mode;
    let dev_user_ids_cb = dev_user_ids;
    let registrador_cb = registrador;
    drv.fijar_oyente_pista(Arc::new(move |guild_id, evento, identidad| {
        let hub = hub_clone.clone();
        let colas = colas_clone.clone();
        let gid_str = guild_id.to_string();
        let proveedor_weak = proveedor_cb.clone();
        let ytdlp_bin_clone = ytdlp_bin_cb.clone();
        let ytdlp_args_clone = ytdlp_args_cb.clone();
        let ytdlp_settings_clone = ytdlp_settings_cb.clone();
        let proxies_clone = proxies_cb.clone();
        let dev_mode = dev_mode_cb.clone();
        let dev_user_ids = dev_user_ids_cb.clone();
        let registrador = registrador_cb.clone();
        tokio::spawn(async move {
            let Some(proveedor) = proveedor_weak.upgrade() else {
                return;
            };
            let Some(d) = proveedor.driver.as_deref() else {
                return;
            };
            let cola_arc = colas.obtener_cola(&gid_str).await;
            let mut cola = cola_arc.lock().await;
            let fin = matches!(evento, "end" | "error");
            let avanzar = fin || evento == "advance";
            let mut seek_inicial = None;
            if fin {
                if !identidad.is_some_and(|id| d.aceptar_fin(guild_id, id)) {
                    return;
                }
                let recuperacion =
                    evento == "end" && player::al_terminar_cancion(&mut cola).is_some();
                if !recuperacion {
                    cola.posicion_segundos = 0;
                } else {
                    seek_inicial = Some(cola.posicion_segundos);
                }
                if !cola.canciones.is_empty()
                    && (evento == "error"
                        || cola.modo_repeticion != player::ModoRepeticion::Cancion)
                {
                    let actual = cola.canciones.remove(0);
                    if evento == "end" && cola.modo_repeticion == player::ModoRepeticion::Cola {
                        cola.canciones.push(actual);
                    } else {
                        cola.canciones_previas.push(actual);
                    }
                }
            } else if evento == "advance" {
                if d.active_track(guild_id).is_some() {
                    return;
                }
                cola.posicion_segundos = 0;
            }
            if avanzar && !cola.detenida && cola.autoplay && cola.canciones.is_empty() {
                let mut consulta = cola.clone();
                let previa = consulta
                    .canciones_previas
                    .last()
                    .map(|p| p.track_id.clone());
                drop(cola);
                let relacionado = crate::api::proveedor_real_aux::encolar_relacionado(
                    &mut consulta,
                    &gid_str,
                    &ytdlp_settings_clone,
                    &proxies_clone,
                )
                .await;
                cola = cola_arc.lock().await;
                if cola.detenida
                    || !cola.autoplay
                    || !cola.canciones.is_empty()
                    || cola.canciones_previas.last().map(|p| &p.track_id) != previa.as_ref()
                    || d.active_track(guild_id).is_some()
                {
                    return;
                }
                match relacionado {
                    Ok(true) => cola.canciones = consulta.canciones,
                    Ok(false) => {}
                    Err(error) => tracing::warn!(%guild_id, %error, "Autoplay relacionado falló"),
                }
            }
            if avanzar && !cola.detenida {
                if let Err(error) = player::reproduccion::iniciar_pista_desde_con_settings(
                    d,
                    &mut cola,
                    &ytdlp_bin_clone,
                    ytdlp_args_clone,
                    seek_inicial,
                    &ytdlp_settings_clone,
                )
                .await
                {
                    tracing::error!(%guild_id, %error, "Error avanzando a la siguiente pista");
                }
            }
            // Solo un arranque nuevo ("play") registra historial; seek y filtro emiten "reinicio".
            let historial = (evento == "play")
                .then(|| {
                    cola.canciones
                        .first()
                        .map(|e| entrada_historial(e, &dev_mode, &dev_user_ids))
                })
                .flatten();
            drop(cola);
            if let (Some(input), Some(reg)) = (historial, registrador.as_ref()) {
                if let Err(error) = reg.registrar(&gid_str, &input).await {
                    tracing::warn!(%guild_id, %error, "Historial de reproducción falló");
                }
            }
            hub.publicar_sincronizacion_con(&gid_str, || {
                super::rutas::reproductor::calcular_estado_reproductor_con(
                    &colas,
                    proveedor.as_ref(),
                    &gid_str,
                )
            })
            .await;
        });
    }));
}

#[cfg(test)]
#[path = "eventos_reproduccion_pruebas.rs"]
mod pruebas;
