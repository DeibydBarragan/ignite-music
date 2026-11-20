//! Implementación de AccionesCanalVoz para el ciclo de presencia de S-20.

use crate::panel::{GestorPaneles, InstantaneaCola};
use nucleo::idioma::Idiomas;
use player::{AccionesCanalVoz, MotivoDesconexion};
use std::path::PathBuf;
use std::sync::Arc;
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pub struct AccionesVozBot {
    pub gestor_colas: Arc<player::GestorColas>,
    pub gestor_paneles: Arc<GestorPaneles>,
    pub driver: Option<Arc<player::SongbirdDriver>>,
    pub idiomas: Arc<Idiomas>,
    pub data_dir: PathBuf,
    pub legacy_languages: Arc<nucleo::LegacyLanguages>,
}

impl AccionesVozBot {
    pub fn nuevo(
        gestor_colas: Arc<player::GestorColas>,
        gestor_paneles: Arc<GestorPaneles>,
        driver: Option<Arc<player::SongbirdDriver>>,
        idiomas: Arc<Idiomas>,
        data_dir: PathBuf,
        legacy_languages: Arc<nucleo::LegacyLanguages>,
    ) -> Self {
        Self {
            gestor_colas,
            gestor_paneles,
            driver,
            idiomas,
            data_dir,
            legacy_languages,
        }
    }

    pub async fn cola_esta_pausada(&self, guild_id: &str) -> bool {
        let cola_arc = self.gestor_colas.obtener_cola(guild_id).await;
        let cola = cola_arc.lock().await;
        cola.pausada
    }
}

#[async_trait::async_trait]
impl AccionesCanalVoz for AccionesVozBot {
    async fn pausar_cola(&self, guild_id: &str) {
        let cola_arc = self.gestor_colas.obtener_cola(guild_id).await;
        let mut cola = cola_arc.lock().await;
        if let Err(error) =
            player::reproduccion::fijar_pausa_con_driver(self.driver.as_deref(), &mut cola, true)
                .await
        {
            tracing::warn!(%guild_id, %error, "No se pudo pausar la pista durante presencia");
        }
    }

    async fn reanudar_cola(&self, guild_id: &str) {
        let cola_arc = self.gestor_colas.obtener_cola(guild_id).await;
        let mut cola = cola_arc.lock().await;
        if let Err(error) =
            player::reproduccion::fijar_pausa_con_driver(self.driver.as_deref(), &mut cola, false)
                .await
        {
            tracing::warn!(%guild_id, %error, "No se pudo reanudar la pista durante presencia");
        }
    }

    async fn parar_cola(&self, guild_id: &str) {
        let cola_arc = self.gestor_colas.obtener_cola(guild_id).await;
        let mut cola = cola_arc.lock().await;
        player::acciones::parar(&mut cola);
    }

    async fn tiene_cola_activa(&self, guild_id: &str) -> bool {
        let cola_arc = self.gestor_colas.obtener_cola(guild_id).await;
        let cola = cola_arc.lock().await;
        !cola.detenida && !cola.canciones.is_empty()
    }

    async fn salir_canal(&self, guild_id: &str, _motivo: MotivoDesconexion) {
        let cola_arc = self.gestor_colas.obtener_cola(guild_id).await;
        {
            let mut cola = cola_arc.lock().await;
            cola.canal_voz = None;
        }
        if let (Some(drv), Ok(gid)) = (&self.driver, guild_id.parse::<u64>()) {
            let _ = drv.leave(gid).await;
        }
    }

    async fn refrescar_panel(&self, guild_id: &str) {
        if let Ok(gid_u64) = guild_id.parse::<u64>() {
            let gid = Id::<GuildMarker>::new(gid_u64);
            let lang = nucleo::idioma_servidor::guild_language(
                &self.idiomas,
                &self.data_dir,
                &self.legacy_languages,
                Some(guild_id),
                None,
            );
            let cola_arc = self.gestor_colas.obtener_cola(guild_id).await;
            let cola = cola_arc.lock().await;
            if let Some(snap) = InstantaneaCola::desde_cola(&cola) {
                drop(cola);
                self.gestor_paneles
                    .refrescar_panel(gid, snap, false, lang)
                    .await;
            }
        }
    }

    async fn cerrar_panel_vacio(&self, guild_id: &str) {
        if let Ok(gid_u64) = guild_id.parse::<u64>() {
            let gid = Id::<GuildMarker>::new(gid_u64);
            let lang = nucleo::idioma_servidor::guild_language(
                &self.idiomas,
                &self.data_dir,
                &self.legacy_languages,
                Some(guild_id),
                None,
            );
            self.gestor_paneles
                .cerrar_panel(gid, "ui.left_empty", None, None, lang)
                .await;
        }
    }
}
