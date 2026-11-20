//! Registro de comandos en Discord y control por hash (Spec S-22).

use super::definiciones::CuerpoComando;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const NOMBRE_MARCADOR: &str = "commands-rust.hash";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoDespliegue {
    pub saltado: bool,
    pub cantidad: usize,
}

pub fn calcular_hash_despliegue(
    cuerpos: &[CuerpoComando],
    client_id: &str,
    guild_id: Option<&str>,
) -> String {
    let payload = serde_json::json!({
        "body": cuerpos,
        "clientId": client_id,
        "guildId": guild_id,
    });
    let bytes = serde_json::to_vec(&payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

pub async fn desplegar_comandos(
    token: &str,
    client_id: &str,
    guild_id: Option<&str>,
    data_dir: &Path,
    cuerpos: &[CuerpoComando],
    force: bool,
) -> Result<ResultadoDespliegue, String> {
    if token.is_empty() || client_id.is_empty() {
        return Err("DISCORD_TOKEN and CLIENT_ID must be set in .env".to_string());
    }

    let marcador = data_dir.join(NOMBRE_MARCADOR);
    let hash = calcular_hash_despliegue(cuerpos, client_id, guild_id);

    if !force && marcador.exists() {
        if let Ok(contenido) = fs::read_to_string(&marcador) {
            if contenido.trim() == hash {
                tracing::debug!("Slash commands unchanged, skipping deploy");
                return Ok(ResultadoDespliegue {
                    saltado: true,
                    cantidad: cuerpos.len(),
                });
            }
        }
    }

    let url = match guild_id {
        Some(gid) if !gid.is_empty() => {
            format!(
                "https://discord.com/api/v10/applications/{}/guilds/{}/commands",
                client_id, gid
            )
        }
        _ => format!(
            "https://discord.com/api/v10/applications/{}/commands",
            client_id
        ),
    };

    let cliente = reqwest::Client::new();
    let resp = cliente
        .put(&url)
        .header("Authorization", format!("Bot {}", token))
        .json(&cuerpos)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = resp.status();
    if status.is_success() {
        let _ = fs::create_dir_all(data_dir);
        let _ = fs::write(&marcador, &hash);
        let scope = match guild_id {
            Some(gid) if !gid.is_empty() => format!("in guild {}", gid),
            _ => "globally".to_string(),
        };
        tracing::info!("Registered {} slash commands {}", cuerpos.len(), scope);
        Ok(ResultadoDespliegue {
            saltado: false,
            cantidad: cuerpos.len(),
        })
    } else {
        let body_text = resp.text().await.unwrap_or_default();
        if body_text.contains("50001") || body_text.contains("10004") {
            let gid_str = guild_id.unwrap_or_default();
            tracing::error!(
                "Slash commands could not be registered in GUILD_ID {}: the bot is not in that server. Invite it there or leave GUILD_ID empty.",
                gid_str
            );
        } else {
            tracing::error!(
                "Slash command registration failed: status {} - {}",
                status,
                body_text
            );
        }
        Err(format!(
            "Discord API error: status {} - {}",
            status, body_text
        ))
    }
}

pub async fn cargar_ids_comandos(
    token: &str,
    client_id: &str,
    guild_id: Option<&str>,
) -> Result<HashMap<String, String>, String> {
    let url = match guild_id {
        Some(gid) if !gid.is_empty() => {
            format!(
                "https://discord.com/api/v10/applications/{}/guilds/{}/commands",
                client_id, gid
            )
        }
        _ => format!(
            "https://discord.com/api/v10/applications/{}/commands",
            client_id
        ),
    };

    let cliente = reqwest::Client::new();
    let resp = cliente
        .get(&url)
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Failed to fetch command IDs: {}", err));
    }

    let comandos: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse command IDs JSON: {e}"))?;

    let mut mapa = HashMap::new();
    for cmd in comandos {
        if let (Some(id), Some(nombre)) = (
            cmd.get("id").and_then(|v| v.as_str()),
            cmd.get("name").and_then(|v| v.as_str()),
        ) {
            mapa.insert(nombre.to_string(), id.to_string());
        }
    }
    Ok(mapa)
}
