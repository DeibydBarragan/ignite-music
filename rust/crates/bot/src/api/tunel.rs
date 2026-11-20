//! Proceso cloudflared, lectura de URL y ciclo de vida del túnel (Spec S-29).

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub const DEFAULT_TUNNEL_TIMEOUT_MS: u64 = 25_000;

static RE_TUNEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https://[a-zA-Z0-9-]+\.trycloudflare\.com").unwrap());

pub fn extraer_url_tunel(texto: &str) -> Option<String> {
    RE_TUNEL.find(texto).map(|m| m.as_str().to_string())
}

pub fn resolver_binario(bin_dir: &Path, root_dir: &Path) -> PathBuf {
    #[cfg(windows)]
    let candidatos = [
        bin_dir.join("cloudflared.exe"),
        root_dir.join("cloudflared.exe"),
        PathBuf::from(r"C:\Program Files (x86)\cloudflared\cloudflared.exe"),
        PathBuf::from(r"C:\Program Files\cloudflared\cloudflared.exe"),
    ];

    #[cfg(not(windows))]
    let candidatos = [bin_dir.join("cloudflared"), root_dir.join("cloudflared")];

    for c in candidatos {
        if c.exists() {
            return c;
        }
    }
    PathBuf::from("cloudflared")
}

#[derive(Clone)]
pub struct GestorTunel {
    url: Arc<RwLock<Option<String>>>,
    proceso: Arc<Mutex<Option<Child>>>,
    binario: Option<PathBuf>,
}

impl Default for GestorTunel {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl GestorTunel {
    pub fn nuevo() -> Self {
        Self {
            url: Arc::new(RwLock::new(None)),
            proceso: Arc::new(Mutex::new(None)),
            binario: None,
        }
    }

    pub fn con_binario(binario: PathBuf) -> Self {
        Self {
            url: Arc::new(RwLock::new(None)),
            proceso: Arc::new(Mutex::new(None)),
            binario: Some(binario),
        }
    }

    pub fn obtener_url(&self) -> Option<String> {
        self.url.read().unwrap().clone()
    }

    pub async fn iniciar(&self, port: u16, timeout: Duration) -> Option<String> {
        let url_actual = self.url.read().unwrap().clone();
        if let Some(u) = url_actual {
            let proc = self.proceso.lock().await;
            if proc.is_some() {
                return Some(u);
            }
        }

        let bin = self
            .binario
            .clone()
            .unwrap_or_else(|| resolver_binario(Path::new("bin"), Path::new(".")));

        let mut cmd = Command::new(&bin);
        cmd.args([
            "tunnel",
            "--protocol",
            "http2",
            "--url",
            &format!("http://localhost:{port}"),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Could not start cloudflared ({e}). Is it installed?");
                return None;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut out_lines = stdout.map(|s| tokio::io::BufReader::new(s).lines());
        let mut err_lines = stderr.map(|s| tokio::io::BufReader::new(s).lines());

        let res = tokio::time::timeout(timeout, async {
            loop {
                tokio::select! {
                    line = async {
                        if let Some(ref mut l) = out_lines {
                            l.next_line().await.ok().flatten()
                        } else {
                            futures_util::future::pending().await
                        }
                    } => {
                        if let Some(text) = line {
                            if let Some(u) = extraer_url_tunel(&text) {
                                return Some(u);
                            }
                        }
                    }
                    line = async {
                        if let Some(ref mut l) = err_lines {
                            l.next_line().await.ok().flatten()
                        } else {
                            futures_util::future::pending().await
                        }
                    } => {
                        if let Some(text) = line {
                            if let Some(u) = extraer_url_tunel(&text) {
                                return Some(u);
                            }
                        }
                    }
                    status = child.wait() => {
                        tracing::warn!("cloudflared exited early with status {status:?}");
                        return None;
                    }
                }
            }
        })
        .await;

        match res {
            Ok(Some(found_url)) => {
                tracing::info!("Cloudflare tunnel active: {found_url}");
                *self.url.write().unwrap() = Some(found_url.clone());
                let mut proc = self.proceso.lock().await;
                *proc = Some(child);

                let url_ref = Arc::clone(&self.url);
                let proc_ref = Arc::clone(&self.proceso);
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        let mut p = proc_ref.lock().await;
                        if let Some(ref mut c) = *p {
                            if let Ok(Some(_)) = c.try_wait() {
                                *p = None;
                                *url_ref.write().unwrap() = None;
                                tracing::info!("Cloudflare tunnel stopped.");
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                });

                Some(found_url)
            }
            Ok(None) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                None
            }
            Err(_) => {
                tracing::warn!("Timeout waiting for Cloudflare tunnel URL; killing cloudflared.");
                let _ = child.start_kill();
                let _ = child.wait().await;
                None
            }
        }
    }

    pub async fn detener(&self) {
        let mut proc = self.proceso.lock().await;
        if let Some(mut child) = proc.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        *self.url.write().unwrap() = None;
    }
}
