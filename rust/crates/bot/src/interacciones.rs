//! Router de interacciones de Discord (Spec S-22, S-23, S-24, S-25).

use crate::cliente::RouterInteraccion;
use crate::comandos::{self, Contexto};
pub use crate::interacciones_opciones::{
    extraer_consulta_autocompletado, extraer_opciones, ResultadoEnrutado,
};
use crate::panel::cliente::{ClientePanel, ClientePanelHttp, ClientePanelNulo};
use crate::panel::GestorPaneles;
use crate::ui::cache_tokens::CacheTokensVistas;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, Weak};
use twilight_cache_inmemory::InMemoryCache;
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::{Interaction, InteractionData, InteractionType};

pub struct RouterInteraccionesReal {
    pub ctx: Arc<Contexto>,
    pub http: Option<Arc<HttpClient>>,
    pub gestor_paneles: Arc<GestorPaneles>,
    pub cache_tokens: Arc<CacheTokensVistas>,
    pub cache: Option<Arc<InMemoryCache>>,
    pub driver: Option<Arc<player::SongbirdDriver>>,
    pub latencia: Arc<AtomicU64>,
    pub proveedor_musica: Option<Arc<dyn crate::api::contexto::ProveedorApi + Send + Sync>>,
    turnos_voz: Mutex<HashMap<u64, Weak<tokio::sync::Mutex<()>>>>,
}

impl RouterInteraccionesReal {
    pub fn nuevo(ctx: Arc<Contexto>, http: Option<Arc<HttpClient>>) -> Self {
        let cliente_panel: Arc<dyn ClientePanel> = match &http {
            Some(h) => Arc::new(ClientePanelHttp::nuevo(h.clone())),
            None => Arc::new(ClientePanelNulo),
        };
        let gestor_paneles = Arc::new(GestorPaneles::nuevo(cliente_panel, ctx.idiomas.clone()));
        let cache_tokens = Arc::new(CacheTokensVistas::nuevo());
        Self {
            ctx,
            http,
            gestor_paneles,
            cache_tokens,
            cache: None,
            driver: None,
            latencia: Arc::new(AtomicU64::new(0)),
            proveedor_musica: None,
            turnos_voz: Mutex::new(HashMap::new()),
        }
    }

    pub fn con_proveedor_musica(
        mut self,
        proveedor: Arc<dyn crate::api::contexto::ProveedorApi + Send + Sync>,
    ) -> Self {
        self.proveedor_musica = Some(proveedor);
        self
    }

    pub(crate) async fn tomar_turno_voz(&self, guild: u64) -> tokio::sync::OwnedMutexGuard<()> {
        let turno = {
            let mut turnos = self.turnos_voz.lock().unwrap();
            turnos.retain(|_, turno| turno.strong_count() > 0);
            if let Some(turno) = turnos.get(&guild).and_then(Weak::upgrade) {
                turno
            } else {
                let turno = Arc::new(tokio::sync::Mutex::new(()));
                turnos.insert(guild, Arc::downgrade(&turno));
                turno
            }
        };
        turno.lock_owned().await
    }

    pub fn con_recursos(
        mut self,
        gestor_paneles: Arc<GestorPaneles>,
        cache: Option<Arc<InMemoryCache>>,
        driver: Option<Arc<player::SongbirdDriver>>,
    ) -> Self {
        self.gestor_paneles = gestor_paneles;
        self.cache = cache;
        self.driver = driver;
        self
    }

    pub fn con_latencia(mut self, latencia: Arc<AtomicU64>) -> Self {
        self.latencia = latencia;
        self
    }

    pub async fn enrutar(&self, interaction: &Interaction) -> ResultadoEnrutado {
        let guild_id = match interaction.guild_id {
            Some(g) => g.to_string(),
            None => return ResultadoEnrutado::Ignorado,
        };

        match interaction.kind {
            InteractionType::ApplicationCommand => {
                if let Some(InteractionData::ApplicationCommand(data)) = &interaction.data {
                    let opciones = extraer_opciones(&data.options);
                    match comandos::despachar_comando(&self.ctx, &guild_id, &data.name, &opciones)
                        .await
                    {
                        Ok(respuesta) => ResultadoEnrutado::Comando {
                            nombre: data.name.clone(),
                            respuesta,
                        },
                        Err(err) => {
                            tracing::error!("Error ejecutando /{}: {}", data.name, err);
                            ResultadoEnrutado::Comando {
                                nombre: data.name.clone(),
                                respuesta: Some(self.ctx.t(
                                    &self.ctx.default_language,
                                    "buttonhandler.processing_error",
                                    &HashMap::new(),
                                )),
                            }
                        }
                    }
                } else {
                    ResultadoEnrutado::Desconocido
                }
            }
            InteractionType::ApplicationCommandAutocomplete => {
                if let Some(InteractionData::ApplicationCommand(data)) = &interaction.data {
                    let consulta = extraer_consulta_autocompletado(&data.options);
                    let opciones = comandos::autocompletar_comando(&data.name, &consulta).await;
                    ResultadoEnrutado::Autocompletado {
                        nombre: data.name.clone(),
                        opciones,
                    }
                } else {
                    ResultadoEnrutado::Autocompletado {
                        nombre: String::new(),
                        opciones: Vec::new(),
                    }
                }
            }
            InteractionType::MessageComponent => {
                let id = interaction
                    .data
                    .as_ref()
                    .and_then(|d| match d {
                        InteractionData::MessageComponent(mc) => Some(mc.custom_id.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                ResultadoEnrutado::Componente { custom_id: id }
            }
            InteractionType::ModalSubmit => {
                let id = interaction
                    .data
                    .as_ref()
                    .and_then(|d| match d {
                        InteractionData::ModalSubmit(m) => Some(m.custom_id.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                ResultadoEnrutado::Modal { custom_id: id }
            }
            _ => ResultadoEnrutado::Ignorado,
        }
    }
}

#[async_trait::async_trait]
impl RouterInteraccion for RouterInteraccionesReal {
    async fn despachar(&self, interaction: Interaction) {
        let Some(guild_id) = interaction.guild_id else {
            return;
        };
        let Some(http) = &self.http else {
            return;
        };

        match interaction.kind {
            InteractionType::ApplicationCommand => {
                if let Some(InteractionData::ApplicationCommand(data)) = &interaction.data {
                    crate::interacciones_comandos::ejecutar_comando_completo(
                        self,
                        &interaction,
                        data,
                        guild_id,
                        http,
                    )
                    .await;
                }
            }
            InteractionType::ApplicationCommandAutocomplete => {
                if let Some(InteractionData::ApplicationCommand(data)) = &interaction.data {
                    crate::interacciones_componentes::despachar_autocompletado(
                        self,
                        &interaction,
                        data,
                        http,
                    )
                    .await;
                }
            }
            InteractionType::MessageComponent => {
                let custom_id = interaction
                    .data
                    .as_ref()
                    .and_then(|d| match d {
                        InteractionData::MessageComponent(mc) => Some(mc.custom_id.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                crate::interacciones_componentes::despachar_componente(
                    self,
                    &interaction,
                    &custom_id,
                    guild_id,
                    http,
                )
                .await;
            }
            InteractionType::ModalSubmit => {
                let custom_id = interaction
                    .data
                    .as_ref()
                    .and_then(|d| match d {
                        InteractionData::ModalSubmit(m) => Some(m.custom_id.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                crate::interacciones_componentes::despachar_modal(
                    self,
                    &interaction,
                    &custom_id,
                    guild_id,
                    http,
                )
                .await;
            }
            _ => {}
        }
    }
}
