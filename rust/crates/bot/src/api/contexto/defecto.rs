use super::*;

pub struct ProveedorApiDefecto(pub Option<Arc<GestorColas>>);

#[async_trait::async_trait]
impl ProveedorApi for ProveedorApiDefecto {
    #[allow(clippy::too_many_arguments)]
    async fn reproducir(
        &self,
        guild_id: &str,
        query: &str,
        voice_channel_id: Option<&str>,
        caller_id: Option<&str>,
        next: bool,
        skip: bool,
        _dev_mode: bool,
    ) -> Result<ResultadoReproducirApi, (u16, &'static str, String)> {
        let mock = medios::Cancion {
            fuente: "custom".to_string(),
            id: "mock_song_1".to_string(),
            nombre: format!("Canción: {query}"),
            artista: "Artista Demo".to_string(),
            url: query.to_string(),
            duracion: 180,
            stream_url: None,
            es_directo: false,
            miniatura: None,
            autor_url: None,
            edad_restringida: false,
            es_letra: false,
        };
        let elem = player::ElementoCola::nuevo(mock, caller_id.map(ToString::to_string));
        let song_front = serializar_elemento_cola(&elem, false);
        let mut pos = 0;
        if let Some(ref colas) = self.0 {
            let cola_arc = colas.obtener_cola(guild_id).await;
            let mut g = cola_arc.lock().await;
            g.canal_voz = Some(voice_channel_id.unwrap_or("default_voice").to_string());
            pos = if next && !g.canciones.is_empty() {
                1
            } else {
                g.canciones.len()
            };
            if pos <= g.canciones.len() {
                g.canciones.insert(pos, elem);
            } else {
                g.canciones.push(elem);
            }
            if skip {
                g.pausada = false;
            }
        }
        Ok(ResultadoReproducirApi {
            success: true,
            started: true,
            position: pos,
            song: Some(song_front),
            playlist: None,
        })
    }

    async fn controlar(
        &self,
        guild_id: &str,
        action: &str,
        value: Option<&serde_json::Value>,
    ) -> Result<(bool, String), (u16, &'static str, String)> {
        if let Some(ref colas) = self.0 {
            let cola_arc = colas.obtener_cola(guild_id).await;
            let mut g = cola_arc.lock().await;
            match action {
                "pause" => g.pausada = true,
                "resume" => g.pausada = false,
                "volume" => {
                    if let Some(v) = value.and_then(|v| v.as_u64()) {
                        g.volumen = v as u8;
                    }
                }
                "stop" => {
                    g.canciones.clear();
                    g.detenida = true;
                }
                _ => {}
            }
        }
        Ok((true, "OK".to_string()))
    }
}
