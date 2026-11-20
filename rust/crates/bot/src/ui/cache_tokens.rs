use medios::letras::ResultadoLetras;
use medios::Cancion;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use twilight_model::id::marker::UserMarker;
use twilight_model::id::Id;

const TTL_LETRAS: Duration = Duration::from_secs(30 * 60);
const TTL_BUSQUEDA: Duration = Duration::from_secs(10 * 60);
const MAX_ENTRADAS: usize = 500;

type EntradaLetras = (ResultadoLetras, Instant);
type EntradaBusqueda = (Id<UserMarker>, Vec<Cancion>, Instant);

#[derive(Clone, Default)]
pub struct CacheTokensVistas {
    letras: Arc<RwLock<HashMap<String, EntradaLetras>>>,
    busquedas: Arc<RwLock<HashMap<String, EntradaBusqueda>>>,
}

impl CacheTokensVistas {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub async fn guardar_letras(&self, token: String, res: ResultadoLetras) {
        let mut map = self.letras.write().await;
        if map.len() >= MAX_ENTRADAS {
            map.clear();
        }
        map.insert(token, (res, Instant::now() + TTL_LETRAS));
    }

    pub async fn obtener_letras(&self, token: &str) -> Option<ResultadoLetras> {
        let map = self.letras.read().await;
        if let Some((res, exp)) = map.get(token) {
            if Instant::now() < *exp {
                return Some(res.clone());
            }
        }
        None
    }

    pub async fn guardar_busqueda(
        &self,
        token: String,
        user_id: Id<UserMarker>,
        results: Vec<Cancion>,
    ) {
        let mut map = self.busquedas.write().await;
        if map.len() >= MAX_ENTRADAS {
            map.clear();
        }
        map.insert(token, (user_id, results, Instant::now() + TTL_BUSQUEDA));
    }

    pub async fn obtener_busqueda(&self, token: &str) -> Option<(Id<UserMarker>, Vec<Cancion>)> {
        let map = self.busquedas.read().await;
        if let Some((uid, res, exp)) = map.get(token) {
            if Instant::now() < *exp {
                return Some((*uid, res.clone()));
            }
        }
        None
    }
}
