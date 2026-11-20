//! Cliente REST de Supabase (Spec S-30). Las lecturas usan la credencial de lectura.
//! Las escrituras exigen SUPABASE_SERVICE_ROLE_KEY y nunca usan la clave anónima.

use nucleo::config::SupabaseConfig;
use reqwest::{Client, Method, RequestBuilder};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResultadoVerificacion {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Escritura pedida sin SUPABASE_SERVICE_ROLE_KEY. El mensaje no contiene claves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinCredencialEscritura;

impl fmt::Display for SinCredencialEscritura {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Supabase: falta SUPABASE_SERVICE_ROLE_KEY, la escritura no se intenta"
        )
    }
}

impl std::error::Error for SinCredencialEscritura {}

#[derive(Clone)]
pub struct ClienteSupabaseRest {
    url: String,
    key: String,
    escritura: Option<String>,
    http: Client,
}

impl ClienteSupabaseRest {
    /// Una sola clave para lectura y escritura.
    pub fn nuevo(url: String, key: String) -> Self {
        Self::con_claves(url, key.clone(), Some(key))
    }

    /// Lecturas con `lectura`. Escrituras solo con `escritura`; sin ella fallan sin petición HTTP.
    pub fn con_claves(url: String, lectura: String, escritura: Option<String>) -> Self {
        Self {
            url: url.trim().trim_end_matches('/').to_string(),
            key: lectura.trim().to_string(),
            escritura: escritura
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty()),
            http: Client::new(),
        }
    }

    pub fn desde_config(config: &SupabaseConfig) -> Option<Self> {
        let url = config.url.trim();
        let service_key = config.service_role_key.trim();
        let direct_key = config.key.trim();

        // Lecturas: clave de servicio si existe, como antes; si no, la clave directa.
        let lectura = if !service_key.is_empty() {
            service_key
        } else {
            direct_key
        };

        if url.is_empty() || lectura.is_empty() {
            if !url.is_empty() || !lectura.is_empty() {
                warn!("Supabase is partially configured (SUPABASE_URL / SUPABASE_KEY): disabled until both are set");
            }
            return None;
        }
        if service_key.is_empty() {
            warn!("SUPABASE_SERVICE_ROLE_KEY is not set: Supabase writes are disabled, reads still work");
        }
        let escritura = (!service_key.is_empty()).then(|| service_key.to_string());
        Some(Self::con_claves(
            url.to_string(),
            lectura.to_string(),
            escritura,
        ))
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    fn peticion(&self, metodo: Method, path: &str, clave: &str) -> RequestBuilder {
        let endpoint = format!("{}/{}", self.url, path.trim_start_matches('/'));
        self.http
            .request(metodo, &endpoint)
            .header("apikey", clave)
            .header("Authorization", format!("Bearer {clave}"))
            .header("Accept", "application/json")
    }

    fn escribir(
        &self,
        metodo: Method,
        path: &str,
    ) -> Result<RequestBuilder, SinCredencialEscritura> {
        let clave = self.escritura.as_deref().ok_or(SinCredencialEscritura)?;
        Ok(self.peticion(metodo, path, clave))
    }

    pub fn get(&self, path: &str) -> RequestBuilder {
        self.peticion(Method::GET, path, &self.key)
    }

    pub fn post(&self, path: &str) -> Result<RequestBuilder, SinCredencialEscritura> {
        Ok(self
            .escribir(Method::POST, path)?
            .header("Content-Type", "application/json"))
    }

    pub fn patch(&self, path: &str) -> Result<RequestBuilder, SinCredencialEscritura> {
        Ok(self
            .escribir(Method::PATCH, path)?
            .header("Content-Type", "application/json"))
    }

    pub fn delete(&self, path: &str) -> Result<RequestBuilder, SinCredencialEscritura> {
        self.escribir(Method::DELETE, path)
    }

    /// Las funciones RPC que usa el bot (top_songs) son lecturas: usan la credencial de lectura.
    pub fn rpc(&self, funcion: &str) -> RequestBuilder {
        self.peticion(Method::POST, &format!("rest/v1/rpc/{funcion}"), &self.key)
            .header("Content-Type", "application/json")
    }

    pub async fn verify_connection(&self, timeout_ms: u64) -> ResultadoVerificacion {
        let timeout = Duration::from_millis(timeout_ms.max(100));
        let res = self
            .get("rest/v1/song_plays?select=id&limit=1")
            .timeout(timeout)
            .send()
            .await;

        match res {
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!("Supabase connection verified");
                    ResultadoVerificacion {
                        ok: true,
                        reason: None,
                    }
                } else {
                    let status = resp.status();
                    let msg = resp.text().await.unwrap_or_default();
                    ResultadoVerificacion {
                        ok: false,
                        reason: Some(format!("HTTP {status}: {msg}")),
                    }
                }
            }
            Err(e) => {
                let reason = if e.is_timeout() {
                    format!("timed out after {timeout_ms} ms")
                } else {
                    e.to_string()
                };
                ResultadoVerificacion {
                    ok: false,
                    reason: Some(reason),
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl crate::api::anuncio::ClienteSupabase for ClienteSupabaseRest {
    async fn upsert_bot_status(&self, api_url: &str) -> Result<(), String> {
        let ahora = chrono::Utc::now().to_rfc3339();
        let payload = serde_json::json!({
            "id": "music",
            "api_url": api_url,
            "updated_at": ahora
        });

        let res = self
            .post("rest/v1/bot_status")
            .map_err(|e| e.to_string())?
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
