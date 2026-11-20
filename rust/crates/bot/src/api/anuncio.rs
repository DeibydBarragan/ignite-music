//! Anuncio de la URL pública de la API a Supabase y latido periódico (Spec S-29).

use super::tunel::GestorTunel;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify};

pub const DEFAULT_ANNOUNCE_INTERVAL: Duration = Duration::from_secs(60);

pub fn calcular_url_publica(tunel_url: Option<&str>, public_api_url: &str) -> String {
    let raw = tunel_url
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(public_api_url)
        .trim();
    let limpio = raw.trim_end_matches('/');
    if limpio.starts_with("http://")
        || limpio.starts_with("https://")
        || limpio.starts_with("HTTP://")
        || limpio.starts_with("HTTPS://")
    {
        limpio.to_string()
    } else {
        String::new()
    }
}

#[async_trait::async_trait]
pub trait ClienteSupabase: Send + Sync {
    async fn upsert_bot_status(&self, api_url: &str) -> Result<(), String>;
}

pub struct ClienteSupabaseHttp {
    url: String,
    key: String,
    http: reqwest::Client,
}

impl ClienteSupabaseHttp {
    pub fn nuevo(url: String, key: String) -> Self {
        Self {
            url,
            key,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl ClienteSupabase for ClienteSupabaseHttp {
    async fn upsert_bot_status(&self, api_url: &str) -> Result<(), String> {
        let endpoint = format!("{}/rest/v1/bot_status", self.url.trim_end_matches('/'));
        let ahora = chrono::Utc::now().to_rfc3339();
        let payload = json!({
            "id": "music",
            "api_url": api_url,
            "updated_at": ahora
        });

        let res = self
            .http
            .post(&endpoint)
            .header("apikey", &self.key)
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=merge-duplicates")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Petición a Supabase fallida: {e}"))?;

        if res.status().is_success() {
            Ok(())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Error HTTP {status}: {body}"))
        }
    }
}

pub struct Anunciador {
    cliente: Option<Arc<dyn ClienteSupabase>>,
    gestor_tunel: Option<Arc<GestorTunel>>,
    public_api_url: String,
    shutdown_notify: Arc<Notify>,
    tarea: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Anunciador {
    pub fn nuevo(
        cliente: Option<Arc<dyn ClienteSupabase>>,
        gestor_tunel: Option<Arc<GestorTunel>>,
        public_api_url: String,
    ) -> Arc<Self> {
        Arc::new(Self {
            cliente,
            gestor_tunel,
            public_api_url,
            shutdown_notify: Arc::new(Notify::new()),
            tarea: Mutex::new(None),
        })
    }

    pub fn url_publica(&self) -> String {
        let tunel = self.gestor_tunel.as_ref().and_then(|g| g.obtener_url());
        calcular_url_publica(tunel.as_deref(), &self.public_api_url)
    }

    pub async fn beat(&self) -> Result<(), String> {
        let url = self.url_publica();
        if url.is_empty() {
            return Ok(());
        }
        if let Some(ref c) = self.cliente {
            let res = c.upsert_bot_status(&url).await;
            if let Err(ref e) = res {
                tracing::warn!("Could not announce API URL: {e}");
            }
            res
        } else {
            Ok(())
        }
    }

    pub fn iniciar(self: &Arc<Self>, intervalo: Duration) -> bool {
        if self.cliente.is_none() || (self.gestor_tunel.is_none() && self.public_api_url.is_empty())
        {
            tracing::info!(
                "PUBLIC_API_URL not set or no Supabase; API discovery announcement disabled."
            );
            return false;
        }

        let this = Arc::clone(self);
        let notif = Arc::clone(&self.shutdown_notify);
        let handle = tokio::spawn(async move {
            if this.url_publica().is_empty() && this.gestor_tunel.is_some() {
                for _ in 0..60 {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(500)) => {
                            if !this.url_publica().is_empty() {
                                break;
                            }
                        }
                        _ = notif.notified() => return,
                    }
                }
            }
            let _ = this.beat().await;
            if intervalo.is_zero() {
                return;
            }
            let mut timer = tokio::time::interval(intervalo);
            timer.tick().await;
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        let _ = this.beat().await;
                    }
                    _ = notif.notified() => break,
                }
            }
        });

        let mut lock = self.tarea.try_lock();
        if let Ok(ref mut slot) = lock {
            **slot = Some(handle);
        }
        true
    }

    pub async fn detener(&self) {
        self.shutdown_notify.notify_waiters();
        let mut t = self.tarea.lock().await;
        if let Some(h) = t.take() {
            h.abort();
        }
    }
}
