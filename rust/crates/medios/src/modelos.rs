use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum FuentesError {
    #[error("NOT_SUPPORTED_URL")]
    UrlNoSoportada,
    #[error("EMPTY_PLAYLIST")]
    ListaVacia,
    #[error("EMPTY_FILTERED_PLAYLIST")]
    ListaFiltradaVacia,
    #[error("NON_NSFW")]
    NsfwNoPermitido,
    #[error("NO_RESULT")]
    ResultadoVacio,
    #[error("NO_STREAM_URL: {0}")]
    SinStreamUrl(String),
    #[error("{0}")]
    YtDlp(String),
    #[error("SPOTIFY_API_ERROR: {0}")]
    Spotify(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cancion {
    pub fuente: String,
    pub id: String,
    pub nombre: String,
    pub artista: String,
    pub url: String,
    pub stream_url: Option<String>,
    pub es_directo: bool,
    pub duracion: u64,
    pub miniatura: Option<String>,
    pub autor_url: Option<String>,
    pub edad_restringida: bool,
    pub es_letra: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListaCanciones {
    pub fuente: String,
    pub id: String,
    pub nombre: String,
    pub url: String,
    pub miniatura: Option<String>,
    pub canciones: Vec<Cancion>,
    pub truncada: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListaEncontrada {
    pub id: String,
    pub nombre: String,
    pub autor: String,
    pub url: String,
    pub miniatura: Option<String>,
    pub cantidad_pistas: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultadoResolucion {
    Cancion(Cancion),
    Lista(ListaCanciones),
}
