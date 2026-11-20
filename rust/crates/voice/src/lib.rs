//! Voice module for ignite-music.

pub mod admission;
pub mod alucinacion;
pub mod audio;
pub mod comandos;
pub mod csv_voz;
pub mod despacho;
pub mod despacho_tipos;
pub mod groq;
pub mod matcher;
pub mod matcher_texto;
pub mod receive;
pub mod registro_voz;
pub mod speakers;
pub mod stt;
pub mod tts;
pub mod vosk;
pub mod vosk_modelo;
pub mod wake_gate;
pub mod wake_gate_gramatica;

pub use admission::{
    AdmissionConfig, AdmissionController, AdmissionDecision, GateEvaluation, WakeGateEvaluator,
};
pub use alucinacion::{
    es_alucinacion, normalizar_alucinacion, SegmentoWhisper, FRASES_ALUCINACION,
};
pub use audio::{downsample_48k_stereo_to_16k_mono, pcm_rms, pcm_to_wav, slice_pcm};
pub use comandos::{
    ALLOWED_NON_ARG_SUFFIXES, COMMANDS_WITH_ARGUMENTS, COURTESY_PREFIXES, DEFAULT_BINDINGS,
};
pub use csv_voz::{
    build_csv, format_cell, to_csv_cell, ExportadorCsvVoz, VoiceCsvExporter, CSV_BATCH_SIZE,
    CSV_COLUMNS,
};
pub use despacho::{despachar, resolver_mensaje_feedback};
pub use despacho_tipos::{
    resolver_mensaje_feedback_con_plantillas, AjustesVoz, ResultadoDespacho, SolicitudDespacho,
};
pub use groq::{transcribir_audio, GroqConfig, GroqError, Transcripcion, GROQ_API_URL};
pub use matcher::{
    build_phrase_list, coincidir, Coincidencia, CommandPhrase, FrasePersonalizada, WakeOccurrence,
};
pub use matcher_texto::{
    clean_query, find_wake_word_occurrences, get_all_wake_words, is_allowed_non_arg_query,
    is_preposition_para, normalizar_texto, parse_wake_words,
};
pub use receive::{ReceiverConfig, VoiceFragment, VoiceReceiverHandler, VoiceSegmenter};
pub use registro_voz::{
    AlmacenRegistroVoz, EntradaRegistroVoz, NuevaEntradaRegistroVoz, PublicadorRegistroVoz,
    VoiceEventLogStore, VoiceLogPublisher, MAX_LOGS_PER_GUILD,
};
pub use speakers::{SpeakerFilter, SpeakerFilterConfig};
pub use stt::{transcribir_enunciado, SttOptions, DEFAULT_PROMPT};
pub use tts::{
    interpolate_template, resolve_feedback_message, send_tts_feedback, AvisoTts, OpcionesTts,
    DEFAULT_SOUND_API_URL, DEFAULT_TTS_TEMPLATES, DEFAULT_TTS_VOICE,
};
pub use vosk::{VoskApi, VoskError, VoskModel, VoskRecognizer, VoskWrapper};
pub use vosk_modelo::{VoskBackend, VoskModelManager};
pub use wake_gate::{Veredicto, WakeGate};
pub use wake_gate_gramatica::{
    build_grammar, has_voiced_tail, split_compound, GrammarPlanResult, ModelPlan,
};
