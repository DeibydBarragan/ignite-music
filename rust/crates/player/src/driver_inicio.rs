use songbird::events::{Event, EventContext, EventHandler};

pub(crate) struct MedidorInicioPista {
    pub(crate) guild_id: u64,
    pub(crate) identidad: u128,
    pub(crate) solicitado: std::time::Instant,
}

#[async_trait::async_trait]
impl EventHandler for MedidorInicioPista {
    async fn act(&self, _: &EventContext<'_>) -> Option<Event> {
        tracing::info!(
            guild_id = self.guild_id,
            track_id = %format!("{:032x}", self.identidad),
            startup_latency_ms = self.solicitado.elapsed().as_millis() as u64,
            "Songbird playback started"
        );
        None
    }
}
