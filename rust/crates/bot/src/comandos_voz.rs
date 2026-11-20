use crate::{api::ServiciosApiBot, datos::ajustes_voz_modelos::AjustesVozGuild};
use nucleo::config_model::VoiceConfig;
use player::Musica;
use std::{path::PathBuf, sync::Arc, time::Instant};
use tokio::{sync::mpsc, task::JoinHandle};
use transcripcion::{ClienteGroq, GroqReal};
use voice::{
    AdmissionConfig, AdmissionController, AdmissionDecision, AlmacenRegistroVoz,
    NuevaEntradaRegistroVoz, VoiceFragment, VoskModelManager, WakeGateEvaluator,
};

#[cfg(test)]
mod pruebas;
mod transcripcion;

struct Procesador {
    config: VoiceConfig,
    gate: Option<Arc<dyn WakeGateEvaluator>>,
    vosk: Option<Arc<VoskModelManager>>,
    admision: Arc<AdmissionController>,
    registro: Arc<AlmacenRegistroVoz>,
    inicio: Instant,
    groq: Arc<dyn ClienteGroq>,
}

pub fn iniciar(
    rx: mpsc::Receiver<VoiceFragment>,
    config: &nucleo::Config,
    dll: Option<PathBuf>,
    servicios: &ServiciosApiBot,
) -> JoinHandle<()> {
    let (config_voz, datos) = (config.voice.clone(), config.data_dir.clone());
    let contexto = Arc::new(servicios.contexto.clone());
    let (ajustes, registro, musica) = (
        servicios.ajustes_voz.clone(),
        servicios.almacen_logs_voz.clone(),
        servicios.proveedor_real.clone(),
    );
    tokio::spawn(async move {
        let gating = config_voz.offline_gating;
        let (gate, vosk) = tokio::task::spawn_blocking(move || {
            let Some(dll) = dll else { return (None, None) };
            let en = crate::binarios::vosk::ruta_modelo_en(&datos);
            let es = crate::binarios::vosk::ruta_modelo_es(&datos);
            let gate = gating
                .then(|| voice::WakeGate::initialize(&dll, &en, Some(&es)))
                .and_then(|res| cargar_modelo(res, "wake gate"));
            let vosk = cargar_modelo(
                VoskModelManager::initialize(&dll, &es),
                "transcripción local",
            );
            (gate, vosk)
        })
        .await
        .unwrap_or_default();
        let procesador = Arc::new(Procesador {
            gate: gate.map(|gate| Arc::new(gate) as Arc<dyn WakeGateEvaluator>),
            vosk: vosk.map(Arc::new),
            admision: Arc::new(AdmissionController::new(AdmissionConfig {
                min_rms: config_voz.min_rms.into(),
                offline_gating: gating,
                groq_cooldown_ms: config_voz.groq_cooldown_ms,
            })),
            config: config_voz,
            registro,
            inicio: Instant::now(),
            groq: Arc::new(GroqReal),
        });

        let mut tareas = tokio::task::JoinSet::new();
        let turnos: Vec<_> = (0..4)
            .map(|_| {
                let (tx, mut rx) = mpsc::channel::<VoiceFragment>(8);
                let (procesador, ajustes, musica, contexto) = (
                    procesador.clone(),
                    ajustes.clone(),
                    musica.clone(),
                    contexto.clone(),
                );
                tareas.spawn(async move {
                    while let Some(audio) = rx.recv().await {
                        let guild = audio.guild_id.to_string();
                        let ajustes = ajustes.obtener_para_voz(&guild).await;
                        if procesador
                            .procesar(audio, ajustes, musica.as_ref(), None)
                            .await
                        {
                            contexto
                                .sse
                                .publicar_sincronizacion(&guild, &contexto)
                                .await;
                        }
                    }
                });
                tx
            })
            .collect();
        let mut rx = rx;
        while let Some(audio) = rx.recv().await {
            let turno = ((audio.guild_id >> 22) % turnos.len() as u64) as usize;
            if turnos[turno].try_send(audio).is_err() {
                tracing::warn!("Cola de procesamiento de voz no disponible");
            }
        }
        drop(turnos);
        while tareas.join_next().await.is_some() {}
    })
}

fn cargar_modelo<T>(res: Result<T, voice::VoskError>, nombre: &str) -> Option<T> {
    res.inspect_err(|error| tracing::warn!(%error, modelo = nombre, "Vosk no disponible"))
        .ok()
}

impl Procesador {
    async fn procesar<M: Musica>(
        &self,
        audio: VoiceFragment,
        ajustes: AjustesVozGuild,
        musica: &M,
        api_url: Option<String>,
    ) -> bool {
        if !ajustes.enabled {
            return false;
        }
        let (guild, usuario) = (audio.guild_id.to_string(), audio.user_id.to_string());
        let Some(canal) = musica.user_voice_channel(&guild, &usuario) else {
            return false;
        };
        self.registro.fijar_csv(&guild, ajustes.csv_enabled);
        let mut evento = NuevaEntradaRegistroVoz::new(&guild);
        evento.user_id = Some(usuario.clone());
        let (gate, admision, wake_word, inicio) = (
            self.gate.clone(),
            self.admision.clone(),
            ajustes.wake_word.clone(),
            self.inicio,
        );
        let admitido = tokio::task::spawn_blocking(move || {
            let pcm = voice::downsample_48k_stereo_to_16k_mono(&audio.pcm48k);
            evento.audio_bytes = Some((44 + pcm.len() * 2) as u64);
            match admision.admit(
                audio.guild_id,
                audio.user_id,
                &pcm,
                &wake_word,
                gate.as_deref(),
                inicio.elapsed().as_millis() as u64,
            ) {
                AdmissionDecision::LowVolume { .. } => None,
                AdmissionDecision::RateLimited => {
                    evento.status = Some("rate_limited".into());
                    Some((evento, None))
                }
                AdmissionDecision::GateFilteredOut { reason, heard } => {
                    evento.gate_reason = reason;
                    evento.vosk_heard = heard;
                    Some((evento, None))
                }
                AdmissionDecision::Admitted {
                    sliced_pcm, gate, ..
                } => {
                    evento.wake_word_spotted = Some(gate.is_some());
                    evento.gate_reason = Some("gate_disabled".into());
                    if let Some(gate) = gate {
                        evento.gate_reason = gate.reason;
                        evento.matched_wake_word = gate.wake_word;
                        evento.vosk_heard = gate.heard;
                    }
                    Some((evento, Some(sliced_pcm.to_vec())))
                }
            }
        })
        .await;
        let Ok(Some((mut evento, pcm))) = admitido else {
            if let Err(error) = admitido {
                tracing::warn!(%error, "Falló la admisión de voz");
            }
            return false;
        };
        if let Some(pcm) = pcm {
            self.transcribir_y_despachar(musica, ajustes, &canal, pcm, &mut evento, api_url)
                .await;
        }
        let ejecutado = evento.status.as_deref() == Some("executed");
        self.registro.registrar(evento);
        ejecutado
    }
}
