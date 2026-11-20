//! Lógica de reproducción y autocompletado para /play y /playnext (Spec S-22).

use super::Contexto;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EleccionAutocompletado {
    pub name: String,
    pub value: String,
}

pub async fn autocompletar(consulta: &str) -> Vec<EleccionAutocompletado> {
    let valor = consulta.trim();
    if valor.is_empty()
        || valor.len() > 100
        || valor.starts_with("http://")
        || valor.starts_with("https://")
    {
        return Vec::new();
    }

    let encoded: String = url::form_urlencoded::byte_serialize(valor.as_bytes()).collect();
    let url = format!(
        "https://suggestqueries.google.com/complete/search?client=firefox&ds=yt&q={}",
        encoded
    );

    let cliente = reqwest::Client::new();
    let resp = cliente
        .get(&url)
        .timeout(Duration::from_millis(1200))
        .send()
        .await;

    let respuesta = match resp {
        Ok(resp) if resp.status().is_success() => resp.json().await.unwrap_or_default(),
        _ => serde_json::Value::Null,
    };
    elecciones_autocompletado(valor, &respuesta)
}

fn elecciones_autocompletado(
    valor: &str,
    respuesta: &serde_json::Value,
) -> Vec<EleccionAutocompletado> {
    let sugerencias = respuesta.get(1).and_then(serde_json::Value::as_array);
    let mut vistos = std::collections::HashSet::new();
    std::iter::once(valor)
        .chain(
            sugerencias
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str),
        )
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 100 && vistos.insert(s.to_lowercase()))
        .take(25)
        .map(|s| EleccionAutocompletado {
            name: s.into(),
            value: s.into(),
        })
        .collect()
}

#[cfg(test)]
#[test]
fn test_autocompletado_limite_25() {
    let mut sugerencias = vec![" QUEEN ".to_string(), " ".to_string(), "x".repeat(101)];
    sugerencias.extend((0..30).map(|indice| format!("queen {indice}")));
    let respuesta = serde_json::json!(["queen", sugerencias]);
    let elecciones = elecciones_autocompletado("queen", &respuesta);
    assert_eq!(elecciones.len(), 25);
    assert_eq!(elecciones[0].value, "queen");
    assert_eq!(elecciones[1].value, "queen 0");
    assert_eq!(elecciones[24].value, "queen 23");
    assert_eq!(
        elecciones_autocompletado("queen", &serde_json::Value::Null).len(),
        1
    );
}

pub async fn reproducir_desde_interaccion(
    ctx: &Contexto,
    guild_id: &str,
    lang: &str,
    consulta: &str,
    siguiente: bool,
) -> Result<String, String> {
    let q = consulta.trim();
    if q.is_empty() {
        let key = if siguiente {
            "commands.playnext.query_description"
        } else {
            "commands.play.query_description"
        };
        let vacio = ctx.t(lang, key, &HashMap::new());
        return Ok(if vacio == key {
            "Debes ingresar una canción o enlace.".into()
        } else {
            vacio
        });
    }

    let cola_lock = ctx.gestor_colas.obtener_cola(guild_id).await;
    let mut cola = cola_lock.lock().await;

    let bin = medios::resolve_binary(&ctx.ytdlp_settings);
    let limiter = ctx.ytdlp_settings.limiter.clone();

    let cancion_opt = if medios::parse_youtube_url(q).is_some() {
        match medios::resolver_youtube(
            &bin,
            q,
            Some(guild_id),
            &ctx.ytdlp_settings,
            &[],
            &limiter,
            ctx.config_bot.max_playlist_size as usize,
        )
        .await
        {
            Ok(medios::modelos::ResultadoResolucion::Cancion(c)) => Some(c),
            Ok(medios::modelos::ResultadoResolucion::Lista(l)) => l.canciones.into_iter().next(),
            Err(_) => None,
        }
    } else if q.starts_with("http://") || q.starts_with("https://") {
        match medios::resolver_generica(
            &bin,
            q,
            Some(guild_id),
            &ctx.ytdlp_settings,
            &[],
            &limiter,
            ctx.config_bot.max_playlist_size as usize,
        )
        .await
        {
            Ok(medios::modelos::ResultadoResolucion::Cancion(c)) => Some(c),
            Ok(medios::modelos::ResultadoResolucion::Lista(l)) => l.canciones.into_iter().next(),
            Err(_) => None,
        }
    } else {
        medios::buscar(
            &bin,
            q,
            1,
            Some(guild_id),
            &ctx.ytdlp_settings,
            &[],
            &limiter,
        )
        .await
        .ok()
        .and_then(|mut v| {
            if v.is_empty() {
                None
            } else {
                Some(v.remove(0))
            }
        })
    };

    let cancion = match cancion_opt {
        Some(c) => c,
        None => medios::Cancion {
            fuente: "youtube".into(),
            id: "stream".into(),
            nombre: q.into(),
            artista: "".into(),
            url: q.into(),
            stream_url: None,
            es_directo: false,
            duracion: 0,
            miniatura: None,
            autor_url: None,
            edad_restringida: false,
            es_letra: false,
        },
    };

    let titulo_final = cancion.nombre.clone();
    let elem = player::ElementoCola::nuevo(cancion, None);
    let pos = if siguiente { 1 } else { 0 };
    let max = ctx.config_bot.max_queue_size as usize;

    match player::encolar(&mut cola, vec![elem], pos, false, max, true) {
        Ok(resultado) => {
            if resultado.empezada {
                cola.posicion_segundos = 0;
                if let Some(drv) = &ctx.driver {
                    if let Err(e) = player::reproduccion::iniciar_pista_con_settings(
                        drv,
                        &mut cola,
                        &bin,
                        ctx.ytdlp_args.clone(),
                        &ctx.ytdlp_settings,
                    )
                    .await
                    {
                        tracing::error!(
                            "No se pudo iniciar la reproduccion en {}: {}",
                            guild_id,
                            e
                        );
                        return Err(format!("Error iniciando la reproduccion: {e}"));
                    }
                }
            }
            drop(cola);

            let mut vars = HashMap::new();
            vars.insert("title".into(), titulo_final);
            vars.insert("position".into(), resultado.posicion.to_string());

            if resultado.empezada {
                Ok(ctx.t(lang, "commands.play.track_started", &vars))
            } else if siguiente {
                let msg = ctx.t(lang, "commands.playnext.track_queued", &vars);
                if msg == "commands.playnext.track_queued" {
                    Ok(ctx.t(lang, "commands.play.track_queued", &vars))
                } else {
                    Ok(msg)
                }
            } else {
                Ok(ctx.t(lang, "commands.play.track_queued", &vars))
            }
        }
        Err(player::ErrorReproducir::ColaLlena) => {
            let mut vars = HashMap::new();
            vars.insert("max".into(), ctx.config_bot.max_queue_size.to_string());
            Ok(ctx.t(lang, "ui.queue_full", &vars))
        }
        Err(e) => Err(e.to_string()),
    }
}
