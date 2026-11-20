use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstadoCola {
    pub activa: bool,
    pub pausada: bool,
    pub tamano: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultadoReproducir {
    pub posicion: usize,
    pub truncado: bool,
    pub empezada: bool,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ErrorMusica {
    #[error("no_active_queue")]
    NoHayCola,
    #[error("ColaLlena")]
    ColaLlena,
    #[error("PosicionInvalida")]
    PosicionInvalida,
    #[error("NoImplementado")]
    NoImplementado,
    #[error("Fallo: {0}")]
    Fallo(String),
}

#[async_trait::async_trait]
pub trait Musica: Send + Sync {
    async fn estado(&self, servidor: &str) -> Result<EstadoCola, ErrorMusica> {
        Ok(EstadoCola {
            activa: self.has_active_queue(servidor),
            pausada: self.is_paused_or_stopped(servidor),
            tamano: 0,
        })
    }

    async fn reproducir(
        &self,
        _servidor: &str,
        _canal: Option<&str>,
        _miembro: &str,
        _consulta: &str,
        _posicion: usize,
        _saltar: bool,
    ) -> Result<ResultadoReproducir, ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn pausar(&self, _servidor: &str) -> Result<(), ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn reanudar(&self, _servidor: &str) -> Result<(), ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn saltar(&self, _servidor: &str) -> Result<(), ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn anterior(&self, _servidor: &str) -> Result<(), ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn parar(&self, _servidor: &str) -> Result<(), ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn mezclar(&self, _servidor: &str) -> Result<(), ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn volumen(&self, _servidor: &str, _nivel: u8) -> Result<(), ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn autoplay(&self, _servidor: &str, _activo: Option<bool>) -> Result<bool, ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    async fn tamano(&self, _servidor: &str) -> Result<usize, ErrorMusica> {
        Err(ErrorMusica::NoImplementado)
    }

    // Bridge methods for S-10 (voice crate)
    async fn play(
        &self,
        guild_id: &str,
        channel_id: Option<&str>,
        user_id: &str,
        query: &str,
        skip: bool,
        position: Option<usize>,
    ) -> Result<(), String> {
        self.reproducir(
            guild_id,
            channel_id,
            user_id,
            query,
            position.unwrap_or(0),
            skip,
        )
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    async fn apply_control(
        &self,
        guild_id: &str,
        action: &str,
        value: Option<i64>,
    ) -> Result<(), String> {
        match action {
            "pause" => self.pausar(guild_id).await.map_err(|e| e.to_string()),
            "resume" => self.reanudar(guild_id).await.map_err(|e| e.to_string()),
            "skip" => self.saltar(guild_id).await.map_err(|e| e.to_string()),
            "previous" => self.anterior(guild_id).await.map_err(|e| e.to_string()),
            "stop" => self.parar(guild_id).await.map_err(|e| e.to_string()),
            "shuffle" => self.mezclar(guild_id).await.map_err(|e| e.to_string()),
            "volume" => {
                let vol = value.unwrap_or(80).clamp(0, 100) as u8;
                self.volumen(guild_id, vol).await.map_err(|e| e.to_string())
            }
            _ => Err("Acción no soportada".to_string()),
        }
    }

    async fn set_autoplay(&self, guild_id: &str, enabled: bool) -> Result<(), String> {
        self.autoplay(guild_id, Some(enabled))
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn get_queue_count(&self, guild_id: &str) -> Result<usize, String> {
        self.tamano(guild_id).await.map_err(|e| e.to_string())
    }

    fn has_active_queue(&self, _guild_id: &str) -> bool {
        true
    }

    fn is_paused_or_stopped(&self, _guild_id: &str) -> bool {
        false
    }

    fn user_has_voice_channel(&self, _guild_id: &str, _user_id: &str) -> bool {
        true
    }

    /// Obtiene el canal de voz actual del usuario. Obligatorio en la implementación real para TTS.
    fn user_voice_channel(&self, guild_id: &str, user_id: &str) -> Option<String>;

    /// Comprueba si el servidor existe. Obligatorio en la implementación real para emitir guild_not_found.
    fn guild_exists(&self, guild_id: &str) -> bool;

    fn refresh_panel(&self, _guild_id: &str) {}
}
