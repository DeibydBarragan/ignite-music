//! Ejecución de acciones de control para el ProveedorApiReal (Spec S-27).

use super::proveedor_real::ProveedorApiReal;
use player::acciones;
use player::filtros::filtro_etiqueta;
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use twilight_model::id::{marker::GuildMarker, Id};

pub struct ConfiguracionControles {
    pub idiomas: Arc<nucleo::Idiomas>,
    pub legacy: Arc<nucleo::LegacyLanguages>,
    pub data_dir: PathBuf,
    pub max_volumen: u8,
}

impl ProveedorApiReal {
    pub fn con_controles(mut self, controles: ConfiguracionControles) -> Self {
        self.controles = Some(controles);
        self
    }
}

impl ConfiguracionControles {
    async fn idioma(&self, proveedor: &ProveedorApiReal, guild: Id<GuildMarker>) -> &str {
        let gid = guild.to_string();
        let ruta = self.data_dir.join("guilds").join(format!("{gid}.json"));
        let guardado = tokio::fs::read_to_string(ruta)
            .await
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok());
        guardado
            .as_ref()
            .and_then(|v| v.get("language")?.as_str())
            .and_then(|lang| self.idiomas.find_language_code(lang))
            .or_else(|| {
                self.legacy
                    .get(&gid)
                    .and_then(|lang| self.idiomas.find_language_code(lang))
            })
            .or_else(|| {
                self.idiomas
                    .from_discord_locale(proveedor.cache.guild(guild)?.preferred_locale())
            })
            .unwrap_or_else(|| self.idiomas.default_language())
    }
}

fn numero(valor: Option<&Value>) -> Option<f64> {
    match valor? {
        Value::Null | Value::Bool(false) => Some(0.0),
        Value::Bool(true) => Some(1.0),
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Some(0.0);
            }
            for (prefijos, radix) in [(["0x", "0X"], 16), (["0o", "0O"], 8), (["0b", "0B"], 2)] {
                if let Some(digitos) = prefijos.into_iter().find_map(|p| s.strip_prefix(p)) {
                    return u64::from_str_radix(digitos, radix).ok().map(|v| v as f64);
                }
            }
            s.parse().ok()
        }
        _ => None,
    }
    .filter(|n| n.is_finite())
}

pub async fn ejecutar_control_api(
    prov: &ProveedorApiReal,
    guild_id: &str,
    act: &str,
    value: Option<&Value>,
) -> Result<(bool, String), (u16, &'static str, String)> {
    let guild = guild_id
        .parse::<Id<GuildMarker>>()
        .map_err(|e| (400, "INVALID_SNOWFLAKE", e.to_string()))?;
    let sin_cola = || {
        (
            404,
            "NO_QUEUE",
            "No active playback queue in this guild".into(),
        )
    };
    if !prov.gestor_colas.tiene_cola(guild_id).await {
        return Err(sin_cola());
    }

    let config = prov.controles.as_ref();
    let idioma = match config {
        Some(config) => config.idioma(prov, guild).await,
        None => "en",
    };
    let t = |clave: &str, vars: HashMap<String, String>| {
        config.map_or_else(|| clave.to_owned(), |c| c.idiomas.t(idioma, clave, &vars))
    };

    let cola_arc = prov.gestor_colas.obtener_cola(guild_id).await;
    let mut cola = cola_arc.lock().await;
    if cola.canciones.is_empty() && act != "stop" {
        return Err(sin_cola());
    }

    let driver = prov.driver.as_deref();
    let driver_error = |error: player::DriverError| (409, "CONTROL_FAILED", error.to_string());
    let parametros_fuente = || {
        let bin = medios::resolve_binary(&prov.ytdlp_settings);
        let proxy = medios::proxy_for(Some(guild_id), &prov.proxies);
        let args = medios::base_args(&prov.ytdlp_settings, proxy.as_deref(), None);
        (bin, args)
    };

    let res = match act {
        "pause" | "resume" => {
            player::reproduccion::fijar_pausa_con_driver(driver, &mut cola, act == "pause")
                .await
                .map_err(driver_error)?
        }
        "toggle" => player::reproduccion::alternar_pausa_con_driver(driver, &mut cola)
            .await
            .map_err(driver_error)?,
        "skip" | "previous" => {
            player::reproduccion::cambiar_pista_con_driver(driver, &mut cola, act == "previous")
                .await
                .map_err(driver_error)?
        }
        "stop" => player::reproduccion::parar_con_driver(driver, &mut cola)
            .await
            .map_err(driver_error)?,
        "shuffle" => acciones::mezclar(&mut cola),
        "autoplay" => acciones::alternar_autoplay(&mut cola),
        "filter" => {
            let nombre = match value {
                None | Some(Value::Null | Value::Bool(false)) => "",
                Some(Value::Number(n)) if n.as_f64() == Some(0.0) => "",
                Some(Value::String(s)) => s,
                _ => return Ok((false, t("errors.unknown", HashMap::new()))),
            };
            let etiqueta = match nombre {
                "" | "none" => t("ui.filter_none", HashMap::new()),
                nombre => match filtro_etiqueta(nombre) {
                    Some(etiqueta) => etiqueta.to_owned(),
                    None => return Ok((false, t("errors.unknown", HashMap::new()))),
                },
            };
            let (bin, args) = parametros_fuente();
            let res = player::reproduccion::fijar_filtro_con_driver(
                driver,
                &mut cola,
                nombre,
                &bin,
                args,
                &prov.ytdlp_settings,
            )
            .await
            .map_err(driver_error)?;
            return Ok((
                res.ok,
                t("ui.filter", HashMap::from([("filter".into(), etiqueta)])),
            ));
        }
        "volume" => {
            let max = config.map_or(100, |c| c.max_volumen);
            let Some(v) =
                numero(value).filter(|n| n.fract() == 0.0 && (0.0..=f64::from(max)).contains(n))
            else {
                return Ok((false, t("modalhandler.invalid_volume", HashMap::new())));
            };
            player::reproduccion::fijar_volumen_con_driver(driver, &mut cola, v as i64, max)
                .await
                .map_err(driver_error)?
        }
        "seek" => {
            let duracion = cola.canciones[0].cancion.duracion;
            if cola.canciones[0].cancion.es_directo || duracion == 0 {
                return Ok((false, t("ui.seek_live", HashMap::new())));
            }
            let Some(s) = numero(value).filter(|n| *n >= 0.0 && *n < duracion as f64) else {
                return Ok((false, t("ui.seek_invalid", HashMap::new())));
            };
            let (bin, args) = parametros_fuente();
            player::reproduccion::buscar_con_driver(
                driver,
                &mut cola,
                s as u64,
                &bin,
                args,
                &prov.ytdlp_settings,
            )
            .await
            .map_err(driver_error)?
        }
        "loop" => {
            let modo = value
                .and_then(|v| v.as_str())
                .and_then(player::ModoRepeticion::desde_str);
            acciones::fijar_bucle(&mut cola, modo)
        }
        "deafen" | "undeafen" => {
            if let Some(drv) = driver {
                drv.set_deaf(guild.get(), act == "deafen")
                    .await
                    .map_err(driver_error)?;
            }
            let mensaje = if act == "deafen" {
                "Bot ensordecido"
            } else {
                "Bot des-ensordecido"
            };
            return Ok((true, mensaje.into()));
        }
        _ => return Err((400, "INVALID_ACTION", format!("Invalid action: {act}"))),
    };

    let mut vars = HashMap::new();
    if let Some(detalle) = res.detalle {
        match res.clave {
            "buttonhandler.song_skipped_title" => {
                return Ok((res.ok, format!("{}: **{detalle}**", t(res.clave, vars))));
            }
            "buttonhandler.songs_shuffled" => {
                vars.insert("count".into(), detalle);
            }
            "modalhandler.volume_changed_desc" => {
                vars.insert("volume".into(), detalle);
            }
            "ui.seeked" => {
                vars.insert(
                    "time".into(),
                    nucleo::formato::formatear_duracion(cola.posicion_segundos),
                );
            }
            _ => {}
        }
    }
    Ok((res.ok, t(res.clave, vars)))
}
