//! Gestión, descarga y verificación de binarios y modelos externos (Spec S-32).

pub mod cloudflared;
pub mod descarga;
pub mod descompresion;
pub mod ffmpeg;
pub mod sonda;
pub mod vosk;
pub mod ytdlp;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RutasBinarios {
    pub ytdlp: PathBuf,
    pub ffmpeg: PathBuf,
    pub cloudflared: Option<PathBuf>,
    pub libvosk: Option<PathBuf>,
}

/// Orquesta la comprobación e instalación de binarios externos al arrancar el bot.
///
/// Falla con código 1 (Err) si falta yt-dlp o ffmpeg, emitiendo los mensajes de Node.
pub async fn asegurar_binarios(config: &nucleo::Config) -> Result<RutasBinarios, String> {
    let cliente = reqwest::Client::builder()
        .user_agent("ignite-music-preflight/1.0")
        .build()
        .map_err(|e| format!("Error creando cliente HTTP: {}", e))?;

    let download_base = if config.ytdlp.download_base.trim().is_empty() {
        None
    } else {
        Some(config.ytdlp.download_base.as_str())
    };

    // 1. yt-dlp
    let ruta_ytdlp = ytdlp::asegurar_ytdlp(
        &config.bin_dir,
        &config.data_dir,
        &config.ytdlp.path,
        download_base,
        &cliente,
    )
    .await?;

    let sonda_ytdlp = sonda::sondear(
        &ruta_ytdlp.to_string_lossy(),
        &["--version"],
        sonda::TIEMPO_SONDA_DEFECTO,
    )
    .await;
    tracing::info!("[ OK ] yt-dlp {}", sonda_ytdlp.salida);

    let _ =
        ytdlp::actualizar_ytdlp(&config.bin_dir, &config.data_dir, download_base, &cliente).await;

    // 2. ffmpeg
    let ruta_ffmpeg =
        ffmpeg::asegurar_ffmpeg(&config.bin_dir, &config.ffmpeg_path, None, &cliente).await?;

    let sonda_ffmpeg = sonda::sondear(
        &ruta_ffmpeg.to_string_lossy(),
        &["-version"],
        sonda::TIEMPO_SONDA_DEFECTO,
    )
    .await;
    let version_ffmpeg = sonda_ffmpeg
        .salida
        .lines()
        .next()
        .unwrap_or("unknown")
        .split(" Copyright")
        .next()
        .unwrap_or("unknown")
        .trim();
    tracing::info!("[ OK ] ffmpeg {}", version_ffmpeg);

    // 3. Vosk (DLL y modelos)
    let ruta_vosk =
        match vosk::asegurar_vosk(&config.bin_dir, &config.data_dir, &cliente, None).await {
            Ok(p) => {
                tracing::info!("[ OK ] Vosk DLL and models");
                Some(p)
            }
            Err(e) => {
                tracing::warn!("Vosk offline recognition disabled: {}", e);
                None
            }
        };

    // 4. cloudflared (solo si AUTO_TUNNEL)
    let ruta_cf = match cloudflared::asegurar_cloudflared(
        &config.bin_dir,
        &config.root,
        config.api.auto_tunnel,
        None,
        &cliente,
    )
    .await
    {
        Ok(Some(p)) => {
            let sonda = sonda::sondear(
                &p.to_string_lossy(),
                &["--version"],
                sonda::TIEMPO_SONDA_DEFECTO,
            )
            .await;
            tracing::info!("[ OK ] cloudflared {}", sonda.salida);
            Some(p)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("cloudflared setup failed: {}", e);
            None
        }
    };

    Ok(RutasBinarios {
        ytdlp: ruta_ytdlp,
        ffmpeg: ruta_ffmpeg,
        cloudflared: ruta_cf,
        libvosk: ruta_vosk,
    })
}

pub fn configurar_ytdlp(config: &nucleo::Config) -> medios::YtDlpSettings {
    let cookies_file = if config.ytdlp.cookies_file.is_empty() {
        None
    } else {
        Some(config.ytdlp.cookies_file.clone())
    };
    medios::ytdlp_args::YtDlpSettings {
        path: if config.ytdlp.path.is_empty() {
            None
        } else {
            Some(config.ytdlp.path.clone())
        },
        cookies_file,
        bin_dir: config.bin_dir.clone(),
        concurrency: config.ytdlp.concurrency as usize,
        timeout_ms: config.ytdlp.timeout_ms,
        limiter: medios::Limitador::new(config.ytdlp.concurrency as usize),
        cookies_from_browser: (!config.ytdlp.cookies_from_browser.is_empty())
            .then(|| config.ytdlp.cookies_from_browser.clone()),
        po_token: (!config.ytdlp.po_token.is_empty()).then(|| config.ytdlp.po_token.clone()),
        extractor_args: (!config.ytdlp.extractor_args.is_empty())
            .then(|| config.ytdlp.extractor_args.clone()),
        ..Default::default()
    }
}
