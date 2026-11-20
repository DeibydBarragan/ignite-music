use crate::driver::SongbirdDriver;
use crate::driver_error::{to_nz, DriverError};
use songbird::Call;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

type ConfigurarReceptor = Box<dyn Fn(u64, &mut Call) + Send + Sync>;

#[derive(Default)]
pub(crate) struct Conexiones {
    configurar_receptor: Option<ConfigurarReceptor>,
    llamadas_configuradas: Mutex<HashMap<u64, Weak<AsyncMutex<Call>>>>,
    operaciones: Mutex<HashMap<u64, Weak<AsyncMutex<()>>>>,
}

impl Conexiones {
    fn operacion(&self, guild: u64) -> Arc<AsyncMutex<()>> {
        let mut operaciones = self.operaciones.lock().unwrap();
        operaciones.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = operaciones.get(&guild).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(AsyncMutex::new(()));
        operaciones.insert(guild, Arc::downgrade(&lock));
        lock
    }

    fn preparar_receptor(&self, guild: u64, llamada: &Arc<AsyncMutex<Call>>, call: &mut Call) {
        let Some(configurar) = &self.configurar_receptor else {
            return;
        };
        let mut configuradas = self.llamadas_configuradas.lock().unwrap();
        let actual = Arc::downgrade(llamada);
        if configuradas
            .get(&guild)
            .is_none_or(|anterior| !anterior.ptr_eq(&actual))
        {
            configurar(guild, call);
            configuradas.insert(guild, actual);
        }
    }
}

impl SongbirdDriver {
    pub fn con_receptor(
        mut self,
        configurar: impl Fn(u64, &mut Call) + Send + Sync + 'static,
    ) -> Self {
        self.conexiones.configurar_receptor = Some(Box::new(configurar));
        self
    }

    pub async fn join(&self, guild_id: u64, channel_id: u64) -> Result<(), DriverError> {
        let guild = to_nz(guild_id)?;
        let canal = to_nz(channel_id)?;
        let operacion = self.conexiones.operacion(guild_id);
        let _turno = operacion.lock().await;
        let llamada = self.songbird.get_or_insert(guild);
        tracing::info!(guild_id, channel_id, "Joining voice channel via Songbird");

        let resultado = tokio::time::timeout(Duration::from_secs(20), async {
            let enlace = {
                let mut call = llamada.lock().await;
                self.conexiones
                    .preparar_receptor(guild_id, &llamada, &mut call);
                call.join(canal).await?
            };
            enlace.await
        })
        .await
        .map_err(|_| DriverError::JoinTimeout)
        .and_then(|res| res.map_err(DriverError::Join));

        if let Err(e) = &resultado {
            tracing::warn!(guild_id, channel_id, error = %e, "Failed to join voice channel");
            self.active_tracks.lock().unwrap().remove(&guild_id);
            if self.songbird.remove(guild).await.is_ok() {
                self.conexiones
                    .llamadas_configuradas
                    .lock()
                    .unwrap()
                    .remove(&guild_id);
            }
        } else {
            tracing::info!(guild_id, channel_id, "Joined voice channel successfully");
        }
        resultado
    }

    pub async fn leave(&self, guild_id: u64) -> Result<(), DriverError> {
        let guild = to_nz(guild_id)?;
        let operacion = self.conexiones.operacion(guild_id);
        let _turno = operacion.lock().await;
        tracing::info!(guild_id, "Leaving voice channel via Songbird");
        let resultado = if self.songbird.get(guild).is_some() {
            self.songbird.remove(guild).await.map_err(DriverError::Join)
        } else {
            Ok(())
        };
        if resultado.is_ok() {
            self.conexiones
                .llamadas_configuradas
                .lock()
                .unwrap()
                .remove(&guild_id);
        }
        self.active_tracks.lock().unwrap().remove(&guild_id);
        self.notificar_cambio_pista(guild_id, "leave");
        resultado
    }
}
