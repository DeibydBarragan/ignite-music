use crate::filtros::filtro_ffmpeg_arg;
use std::env;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub fn resolver_ffmpeg() -> String {
    if let Ok(path) = env::var("FFMPEG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let bin_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };

    if let Ok(bin_dir) = env::var("BIN_DIR") {
        let p = Path::new(&bin_dir).join(bin_name);
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    let candidates = [
        PathBuf::from("bin").join(bin_name),
        Path::new("..").join("bin").join(bin_name),
        Path::new("node_modules")
            .join("ffmpeg-static")
            .join(bin_name),
        Path::new("..")
            .join("..")
            .join("..")
            .join("node_modules")
            .join("ffmpeg-static")
            .join(bin_name),
    ];
    for c in candidates {
        if c.exists() {
            return c.to_string_lossy().to_string();
        }
    }

    "ffmpeg".to_string()
}

pub fn ffmpeg_args(
    input: &str,
    filtro: Option<&str>,
    seek_secs: Option<u64>,
    is_network: bool,
) -> Vec<String> {
    let mut args = Vec::new();

    if is_network {
        args.extend([
            "-reconnect".to_string(),
            "1".to_string(),
            "-reconnect_streamed".to_string(),
            "1".to_string(),
            "-reconnect_delay_max".to_string(),
            "5".to_string(),
        ]);
    }

    if let Some(ss) = seek_secs {
        if ss > 0 {
            args.extend(["-ss".to_string(), ss.to_string()]);
        }
    }

    args.extend(["-i".to_string(), input.to_string()]);

    if let Some(f) = filtro {
        if let Some(arg) = filtro_ffmpeg_arg(f) {
            args.extend(["-af".to_string(), arg.to_string()]);
        }
    }

    args.extend([
        "-f".to_string(),
        "s16le".to_string(),
        "-ac".to_string(),
        "2".to_string(),
        "-ar".to_string(),
        "48000".to_string(),
        "pipe:1".to_string(),
    ]);

    args
}

/// Args para la tubería de reproducción. El WAV conserva una cabecera que
/// Songbird puede detectar mientras ffmpeg escribe el stream.
pub fn ffmpeg_pipe_args(filtro: Option<&str>, seek_secs: Option<u64>) -> Vec<String> {
    let mut args = vec![
        "-nostdin".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-i".into(),
        "pipe:0".into(),
    ];
    if let Some(secs) = seek_secs.filter(|secs| *secs > 0) {
        // El extractor entrega una tubería no seekable; FFmpeg descarta los
        // frames anteriores después de decodificar hasta la posición pedida.
        args.extend(["-ss".into(), secs.to_string()]);
    }
    if let Some(arg) = filtro.and_then(filtro_ffmpeg_arg) {
        args.extend(["-af".into(), arg.into()]);
    }
    args.extend([
        "-ac".into(),
        "2".into(),
        "-ar".into(),
        "48000".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        "-f".into(),
        "wav".into(),
        "pipe:1".into(),
    ]);
    args
}

pub fn crear_comando_ffmpeg(
    binario: &str,
    input: &str,
    filtro: Option<&str>,
    seek_secs: Option<u64>,
    is_network: bool,
) -> Command {
    let args = ffmpeg_args(input, filtro, seek_secs, is_network);
    let mut cmd = Command::new(binario);
    cmd.args(&args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd
}
