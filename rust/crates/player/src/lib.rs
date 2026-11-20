//! Player module for ignite-music.
//!
//! Provides playback queue management (Spec S-17) and Songbird voice driver (Spec S-16).

pub mod acciones;
pub mod cola;
pub mod cola_operaciones;
pub mod driver;
mod driver_conexion;
pub mod driver_error;
mod driver_inicio;
pub mod driver_reproduccion;
pub mod eventos_cola;
pub mod filtros;
pub mod filtros_ffmpeg;
pub mod musica;
mod pipeline_audio;
pub mod presencia;
pub mod presencia_modelos;
pub mod reproduccion;
pub mod reproducir;
pub mod sesiones;
pub mod sesiones_restaurar;

pub use acciones::{
    alternar_autoplay, alternar_pausa, anterior, aplicar_control, buscar, fijar_autoplay,
    fijar_bucle, fijar_filtro, fijar_volumen, mezclar, parar, pausar, reanudar, saltar,
    ResultadoAccion,
};
pub use cola::{Cola, ElementoCola, EntradaDeshacer, GestorColas, ModoRepeticion};
pub use cola_operaciones::{
    deshacer, limpiar, mover, ordenar, quitar_por_indice, quitar_por_track_id, saltar_a,
    ErrorOperacionCola, UNDO_TTL_MS,
};
pub use driver::{DriverError, SongbirdDriver};
pub use eventos_cola::{
    al_iniciar_cancion, al_terminar_cancion, comprobar_corte_prematuro, mensaje_aviso_corte,
    AccionRecuperacion,
};
pub use filtros::{
    es_filtro_valido, fijar_filtro_cola, filtro_etiqueta, filtro_ffmpeg_arg, ResultadoFijarFiltro,
    FILTROS,
};
pub use filtros_ffmpeg::{crear_comando_ffmpeg, ffmpeg_args, ffmpeg_pipe_args, resolver_ffmpeg};
pub use musica::{ErrorMusica, EstadoCola, Musica, ResultadoReproducir};
pub use presencia::{
    contar_personas, AccionesCanalVoz, ConfiguracionPresencia, EstadoVoz, GestorPresencia,
    MotivoDesconexion,
};
pub use reproduccion::{primera_cancion_url, reproducir_primera_cancion};
pub use reproducir::{encolar, ErrorReproducir};
pub use sesiones::{
    ahora_ms, AlmacenSesiones, CancionSesion, SesionGuardada, UploaderSesion, UsuarioSesion,
    DEFAULT_VOLUME, MAX_AGE_MS, MAX_QUEUE_SIZE,
};
pub use sesiones_restaurar::{
    guardar_todas_las_sesiones, restaurar_sesiones, ProveedorRestauracion,
};
