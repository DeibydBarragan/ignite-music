use super::*;
use voice::{
    stt::{transcribir_groq, transcribir_vosk},
    AjustesVoz, FrasePersonalizada, SolicitudDespacho, SttOptions, Transcripcion,
};

#[async_trait::async_trait]
pub(super) trait ClienteGroq: Send + Sync {
    async fn transcribir(&self, wav: &[u8], opciones: &SttOptions) -> Option<Transcripcion>;
}

pub(super) struct GroqReal;

#[async_trait::async_trait]
impl ClienteGroq for GroqReal {
    async fn transcribir(&self, wav: &[u8], opciones: &SttOptions) -> Option<Transcripcion> {
        transcribir_groq(wav, opciones).await
    }
}

impl Procesador {
    pub(super) async fn transcribir_y_despachar<M: Musica>(
        &self,
        musica: &M,
        ajustes: AjustesVozGuild,
        canal: &str,
        pcm: Vec<i16>,
        evento: &mut NuevaEntradaRegistroVoz,
        api_url: Option<String>,
    ) {
        let guild = &evento.guild_id;
        let usuario = evento.user_id.as_deref().unwrap_or_default();
        let wav = voice::pcm_to_wav(&pcm, 16_000, 1);
        evento.audio_bytes = Some(wav.len() as u64);
        let opciones = SttOptions {
            api_key: Some(self.config.groq_api_key.clone()),
            prompt: Some(self.config.prompt.clone()),
            model: Some(self.config.model.clone()),
            language: Some(ajustes.language.clone()),
            guild_id: Some(guild.clone()),
            user_id: Some(usuario.into()),
            api_url,
            ..Default::default()
        };
        let t0 = Instant::now();
        let groq = self.groq.transcribir(&wav, &opciones).await;
        evento.groq_engaged = Some(!self.config.groq_api_key.trim().is_empty());
        if evento.groq_engaged == Some(true) {
            evento.groq_elapsed_ms = Some(t0.elapsed().as_millis() as u64);
        }
        evento.groq_text = groq.as_ref().map(|res| res.texto.clone());
        let res = match (groq, &self.vosk) {
            (None, Some(vosk)) => {
                let vosk = vosk.clone();
                tokio::task::spawn_blocking(move || {
                    let bytes: Vec<_> = pcm.into_iter().flat_map(i16::to_le_bytes).collect();
                    transcribir_vosk(&bytes, &vosk)
                })
                .await
                .ok()
                .flatten()
            }
            (groq, _) => groq,
        };
        let Some(res) = res else {
            evento.status = Some("empty_transcription".into());
            return;
        };
        if voice::es_alucinacion(&res.texto, &res.segmentos) {
            evento.status = Some("hallucination".into());
            return;
        }
        if musica.user_voice_channel(guild, usuario).as_deref() != Some(canal) {
            evento.status = Some("user_not_in_voice".into());
            return;
        }
        let res = voice::despachar(
            musica,
            &SolicitudDespacho {
                guild_id: guild.clone(),
                user_id: usuario.into(),
                channel_id: Some(canal.into()),
                text: res.texto,
            },
            Some(&AjustesVoz {
                enabled: ajustes.enabled,
                wake_word: Some(ajustes.wake_word),
                bindings: Some(
                    ajustes
                        .bindings
                        .into_iter()
                        .map(|b| FrasePersonalizada {
                            comando: b.command_id,
                            frase: b.phrase,
                        })
                        .collect(),
                ),
                tts_feedback_enabled: ajustes.tts_feedback_enabled,
                tts_templates: Some(ajustes.tts_templates),
                tts_voice: Some(ajustes.tts_voice),
                tts_rate: Some(ajustes.tts_rate),
                tts_pitch: Some(ajustes.tts_pitch),
                sound_api_url: Some(self.config.sound_api_url.clone()),
                sound_api_secret: Some(self.config.sound_api_secret.clone()),
            }),
        )
        .await;
        evento.status = if res.ejecutado {
            Some("executed".into())
        } else {
            res.motivo
        };
        evento.command_id = res.comando;
        evento.query = res.consulta;
        evento.tts_message = res.mensaje_tts;
    }
}
