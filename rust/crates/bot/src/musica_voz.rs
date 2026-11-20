use crate::api::{contexto::ProveedorApi, proveedor_real::ProveedorApiReal};
use player::{ErrorMusica, EstadoCola, Musica};
use twilight_model::id::{
    marker::{GuildMarker, UserMarker},
    Id,
};

#[async_trait::async_trait]
impl Musica for ProveedorApiReal {
    async fn estado(&self, guild: &str) -> Result<EstadoCola, ErrorMusica> {
        if !self.gestor_colas.tiene_cola(guild).await {
            return Err(ErrorMusica::NoHayCola);
        }
        let cola = self.gestor_colas.obtener_cola(guild).await;
        let cola = cola.lock().await;
        Ok(EstadoCola {
            activa: !cola.canciones.is_empty() && !cola.detenida,
            pausada: cola.pausada,
            tamano: cola.canciones.len(),
        })
    }

    async fn play(
        &self,
        guild: &str,
        canal: Option<&str>,
        usuario: &str,
        consulta: &str,
        saltar: bool,
        posicion: Option<usize>,
    ) -> Result<(), String> {
        let actual = self
            .user_voice_channel(guild, usuario)
            .filter(|actual| canal.is_none_or(|esperado| esperado == actual))
            .ok_or("El hablante ya no está en el canal del bot")?;
        ProveedorApi::reproducir(
            self,
            guild,
            consulta,
            Some(&actual),
            Some(usuario),
            posicion == Some(1),
            saltar,
            false,
        )
        .await
        .map(|_| ())
        .map_err(|(_, _, mensaje)| mensaje)
    }

    async fn apply_control(
        &self,
        guild: &str,
        accion: &str,
        valor: Option<i64>,
    ) -> Result<(), String> {
        let valor = if accion == "volume" {
            let volumen = valor
                .filter(|v| (0..=200).contains(v))
                .ok_or("Volumen fuera de rango")?;
            Some(serde_json::Value::from(volumen))
        } else {
            valor.map(serde_json::Value::from)
        };
        let (ok, mensaje) = self
            .controlar(guild, accion, valor.as_ref())
            .await
            .map_err(|(_, _, mensaje)| mensaje)?;
        if ok {
            Ok(())
        } else {
            Err(mensaje)
        }
    }

    async fn set_autoplay(&self, guild: &str, activo: bool) -> Result<(), String> {
        let cola = self.gestor_colas.obtener_cola(guild).await;
        cola.lock().await.autoplay = activo;
        Ok(())
    }

    fn user_voice_channel(&self, guild: &str, usuario: &str) -> Option<String> {
        let guild = guild.parse::<Id<GuildMarker>>().ok()?;
        let usuario = usuario.parse::<Id<UserMarker>>().ok()?;
        let bot = Id::new_checked(self.bot_user_id)?;
        let canal = self.cache.voice_state(usuario, guild)?.channel_id();
        (self.cache.voice_state(bot, guild)?.channel_id() == canal).then(|| canal.to_string())
    }

    fn guild_exists(&self, guild: &str) -> bool {
        guild
            .parse::<Id<GuildMarker>>()
            .ok()
            .is_some_and(|id| self.cache.guild(id).is_some())
    }
}
