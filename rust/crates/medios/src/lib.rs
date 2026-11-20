pub mod errores;
pub mod fuentes_cache;
pub mod genericas;
pub mod letras;
pub mod letras_http;
pub mod limitador;
pub mod modelos;
pub mod playlists;
pub mod spotify;
pub mod spotify_modelos;
pub mod youtube;
pub mod youtube_busqueda;
pub mod youtube_relacionados;
pub mod youtube_titulo;
pub mod youtube_url;
pub mod ytdlp;
pub mod ytdlp_args;

pub use errores::{
    describe_error, error_key, host_hint, is_bot_check, YoutubeSearchErrorState, HINT_THROTTLE_MS,
};
pub use fuentes_cache::{
    prefetch, prefetch_query, stream_info, FuentesCache, CACHE_BUSQUEDA_MAX_SIZE,
    CACHE_BUSQUEDA_TTL_MS, CACHE_INFOS_MAX_SIZE, CACHE_INFOS_TTL_MS, CACHE_LETRAS_MAX_SIZE,
    CACHE_LETRAS_TTL_MS,
};
pub use genericas::{json_to_song_generica, resolver_generica, valida_url_generica};
pub use letras::{
    buscar_letras, dividir_letras, elegir_mejor, limpiar_titulo, CacheLetras, ItemLetraLrc,
    ResultadoLetras,
};
pub use limitador::{Limitador, LimitadorError, Permiso, Prioridad};
pub use modelos::{Cancion, FuentesError, ListaCanciones, ListaEncontrada, ResultadoResolucion};
pub use playlists::{
    derive_name, detect_source, format_duration, resolve_playlist, snapshot_track,
    ResultadoPlaylist, TrackSnapshot, FALLBACK_COVER,
};
pub use spotify::{
    create_search_query, parse_spotify_url, spotify_track_to_song, SpotifyClient, SpotifyUrlType,
};
pub use youtube::{
    is_playable_related, is_playlist_id, is_video_id, parse_youtube_url, resolver_youtube,
    stream_ttl, stream_ttl_at, thumbnail_for, watch_url, youtube_info, YouTubeUrlType,
    AUDIO_FORMAT, RELATED_MAX_SECONDS, RELATED_MIN_SECONDS,
};
pub use youtube_busqueda::{buscar, buscar_cancion, buscar_listas, relacionados};
pub use youtube_titulo::{clean_title_suffix, is_lyric_title, norm_name, split_youtube_title};
pub use ytdlp::{clean_error, run_json, version, RunJsonOptions, YtDlpError};
pub use ytdlp_args::{base_args, proxy_for, resolve_binary, YtDlpSettings};
