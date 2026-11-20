//! Errores para el driver de Songbird (Spec S-16).

use songbird::error::JoinError;
use std::num::NonZeroU64;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("Join error: {0}")]
    Join(#[from] JoinError),

    #[error("Timed out connecting to Discord voice")]
    JoinTimeout,

    #[error("File input error: {0}")]
    Input(String),

    #[error("Not connected to a voice channel in guild {0}")]
    NotInChannel(u64),

    #[error("No active track in guild {0}")]
    NoTrack(u64),

    #[error("Track control error: {0}")]
    TrackControl(String),
}

pub fn to_nz(id: u64) -> Result<NonZeroU64, DriverError> {
    NonZeroU64::new(id).ok_or(DriverError::NotInChannel(id))
}
