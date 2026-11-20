use crate::limitador::{Limitador, Prioridad};
use regex::Regex;
use std::sync::LazyLock;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub use crate::ytdlp_args::{base_args, proxy_for, resolve_binary, YtDlpSettings};

static RE_CLEAN_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[[^\]]+\]\s*[\w-]+:\s*").unwrap());

pub fn clean_error(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).collect();
    let error_line = lines.iter().rev().find(|l| l.starts_with("ERROR:"));
    let raw = match error_line {
        Some(l) => l,
        None => lines
            .iter()
            .rev()
            .find(|l| !l.is_empty())
            .unwrap_or(&"yt-dlp failed"),
    };

    let without_error = if let Some(stripped) = raw.strip_prefix("ERROR:") {
        stripped.trim_start()
    } else {
        raw
    };

    let cleaned = RE_CLEAN_PREFIX.replace(without_error, "");
    cleaned.chars().take(300).collect()
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{mensaje}")]
pub struct YtDlpError {
    pub mensaje: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct RunJsonOptions<'a> {
    pub base_args: &'a [String],
    pub extra_args: &'a [String],
    pub prioridad: Prioridad,
    pub timeout: Duration,
    pub queue_timeout: Duration,
}

impl<'a> Default for RunJsonOptions<'a> {
    fn default() -> Self {
        Self {
            base_args: &[],
            extra_args: &[],
            prioridad: Prioridad::Alta,
            timeout: Duration::from_secs(30),
            queue_timeout: Duration::from_secs(30),
        }
    }
}

pub async fn run_json(
    binary: &str,
    target: &str,
    limiter: &Limitador,
    options: RunJsonOptions<'_>,
) -> Result<serde_json::Value, YtDlpError> {
    let permiso = limiter
        .acquire(options.prioridad, Some(options.queue_timeout))
        .await
        .map_err(|e| YtDlpError {
            mensaje: e.to_string(),
            stderr: String::new(),
        })?;

    let mut args = Vec::new();
    args.extend_from_slice(options.base_args);
    args.extend_from_slice(options.extra_args);
    args.push("--dump-single-json".to_string());
    args.push("--".to_string());
    args.push(target.to_string());

    let mut cmd = Command::new(binary);
    cmd.args(&args);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let mut child = cmd.spawn().map_err(|e| {
        let hint = if e.kind() == std::io::ErrorKind::NotFound {
            " (yt-dlp binary not found, run \"npm install\" or set YTDLP_PATH)"
        } else {
            ""
        };
        YtDlpError {
            mensaje: format!("{e}{hint}"),
            stderr: String::new(),
        }
    })?;

    let mut stdout_handle = child.stdout.take().expect("stdout pipe");
    let mut stderr_handle = child.stderr.take().expect("stderr pipe");

    let stdout_fut = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = stdout_handle.read_to_end(&mut buf).await;
        buf
    });

    let stderr_fut = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = stderr_handle.read_to_end(&mut buf).await;
        buf
    });

    let wait_fut = child.wait();

    let exit_status = match tokio::time::timeout(options.timeout, wait_fut).await {
        Ok(res) => res.map_err(|e| YtDlpError {
            mensaje: e.to_string(),
            stderr: String::new(),
        })?,
        Err(_) => {
            let _ = child.kill().await;
            permiso.release();
            let secs = (options.timeout.as_millis() as u64 + 500) / 1000;
            return Err(YtDlpError {
                mensaje: format!("yt-dlp timed out after {secs}s"),
                stderr: String::new(),
            });
        }
    };

    let stdout_bytes = stdout_fut.await.unwrap_or_default();
    let stderr_bytes = stderr_fut.await.unwrap_or_default();
    let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

    permiso.release();

    if !exit_status.success() {
        if !stderr.trim().is_empty() {
            tracing::warn!("[yt-dlp stderr] {}", stderr.trim());
        }
        return Err(YtDlpError {
            mensaje: clean_error(&stderr),
            stderr,
        });
    }

    serde_json::from_str(&stdout).map_err(|_| YtDlpError {
        mensaje: "yt-dlp returned invalid JSON".to_string(),
        stderr,
    })
}

pub async fn version(binary: &str) -> Result<String, String> {
    let mut cmd = Command::new(binary);
    cmd.arg("--version");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    let output = match cmd.output().await {
        Ok(out) => out,
        Err(e) => {
            let msg = if e.kind() == std::io::ErrorKind::NotFound {
                "not found".to_string()
            } else {
                e.to_string()
            };
            return Err(msg);
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        let msg = if !stderr.is_empty() {
            stderr.lines().last().unwrap_or("error").to_string()
        } else if !stdout.is_empty() {
            stdout.lines().last().unwrap_or("error").to_string()
        } else {
            format!("exit code {}", output.status.code().unwrap_or(1))
        };
        Err(msg)
    }
}
