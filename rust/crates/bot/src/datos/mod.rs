//! Módulo de persistencia y datos Supabase (Spec S-30).

pub mod ajustes_voz;
pub mod ajustes_voz_modelos;
pub mod favoritos;
pub mod historial;
pub mod listas;
pub mod listas_modelos;
pub mod ranking;
pub mod ranking_modelos;
pub mod supabase;

pub use ajustes_voz::{AjustesVozGuild, BindingVoz, GestorAjustesVoz};
pub use favoritos::{AccionFavorito, CancionFavorito, GestorFavoritos};
pub use historial::{
    ExclusionesHistorial, GestorHistorial, RegistroCancionInput, RegistroReproduccion,
};
pub use listas::{GestorListas, ResultadoUpsertPlaylist, UserPlaylist, UserPlaylistInput};
pub use ranking::{CancionRanking, GestorRanking};
pub use supabase::{ClienteSupabaseRest, ResultadoVerificacion};
