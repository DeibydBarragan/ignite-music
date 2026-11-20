//! Despacho de acciones de cola para comandos de barra (Spec S-22).

use super::Contexto;
use player::cola::ModoRepeticion;
use player::Cola;
use std::collections::HashMap;

pub async fn accion_de_cola<F>(
    ctx: &Contexto,
    guild_id: &str,
    lang: &str,
    accion: F,
) -> Result<String, String>
where
    F: FnOnce(&mut Cola) -> (bool, &'static str, HashMap<String, String>),
{
    accion_de_cola_con_estado(ctx, guild_id, lang, accion)
        .await
        .map(|(_, mensaje)| mensaje)
}

async fn accion_de_cola_con_estado<F>(
    ctx: &Contexto,
    guild_id: &str,
    lang: &str,
    accion: F,
) -> Result<(bool, String), String>
where
    F: FnOnce(&mut Cola) -> (bool, &'static str, HashMap<String, String>),
{
    let cola_lock = ctx.gestor_colas.obtener_cola(guild_id).await;
    let mut cola = cola_lock.lock().await;

    if cola.canciones.is_empty() {
        return Ok((
            false,
            ctx.t(lang, "commands.nowplaying.no_track", &HashMap::new()),
        ));
    }

    let (ok, clave, vars) = accion(&mut cola);
    let msg = ctx.t(lang, clave, &vars);
    Ok((ok, msg))
}

fn res_accion_a_tupla(
    res: player::ResultadoAccion,
) -> (bool, &'static str, HashMap<String, String>) {
    let mut vars = HashMap::new();
    if let Some(det) = res.detalle {
        vars.insert("count".into(), det.clone());
        vars.insert("volume".into(), det.clone());
        vars.insert("title".into(), det.clone());
        vars.insert("position".into(), det.clone());
        vars.insert("filter".into(), det);
    }
    (res.ok, res.clave, vars)
}

pub async fn ejecutar_pausa(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    ejecutar_cambio_pausa(ctx, gid, lang, true).await
}

pub async fn ejecutar_reanudar(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    ejecutar_cambio_pausa(ctx, gid, lang, false).await
}

async fn ejecutar_cambio_pausa(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    pausada: bool,
) -> Result<String, String> {
    let cola_lock = ctx.gestor_colas.obtener_cola(gid).await;
    let mut cola = cola_lock.lock().await;
    if cola.canciones.is_empty() {
        return Ok(ctx.t(lang, "commands.nowplaying.no_track", &HashMap::new()));
    }
    let resultado =
        player::reproduccion::fijar_pausa_con_driver(ctx.driver.as_deref(), &mut cola, pausada)
            .await
            .map_err(|error| error.to_string())?;
    let (ok, clave, vars) = res_accion_a_tupla(resultado);
    let mensaje = ctx.t(lang, clave, &vars);
    if ok {
        Ok(mensaje)
    } else {
        Err(mensaje)
    }
}

pub async fn ejecutar_saltar(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    cambiar_pista(ctx, gid, lang, false).await
}

pub async fn ejecutar_anterior(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    cambiar_pista(ctx, gid, lang, true).await
}

async fn cambiar_pista(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    ir_anterior: bool,
) -> Result<String, String> {
    let cola = ctx.gestor_colas.obtener_cola(gid).await;
    let mut cola = cola.lock().await;
    if cola.canciones.is_empty() {
        return Ok(ctx.t(lang, "commands.nowplaying.no_track", &HashMap::new()));
    }
    let resultado = player::reproduccion::cambiar_pista_con_driver(
        ctx.driver.as_deref(),
        &mut cola,
        ir_anterior,
    )
    .await
    .map_err(|error| error.to_string())?;
    let (_, clave, vars) = res_accion_a_tupla(resultado);
    Ok(ctx.t(lang, clave, &vars))
}

pub async fn ejecutar_parar(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    let cola_lock = ctx.gestor_colas.obtener_cola(gid).await;
    let mut cola = cola_lock.lock().await;
    let res = player::reproduccion::parar_con_driver(ctx.driver.as_deref(), &mut cola)
        .await
        .map_err(|error| error.to_string())?;
    drop(cola);
    Ok(ctx.t(lang, res.clave, &HashMap::new()))
}

pub async fn ejecutar_mezclar(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    accion_de_cola(ctx, gid, lang, |c| res_accion_a_tupla(player::mezclar(c))).await
}

pub async fn ejecutar_bucle(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    modo_str: &str,
) -> Result<String, String> {
    let modo = ModoRepeticion::desde_str(modo_str);
    accion_de_cola(ctx, gid, lang, |c| {
        res_accion_a_tupla(player::fijar_bucle(c, modo))
    })
    .await
}

pub async fn ejecutar_autoplay(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    accion_de_cola(ctx, gid, lang, |c| {
        res_accion_a_tupla(player::alternar_autoplay(c))
    })
    .await
}

pub async fn ejecutar_volumen(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    nivel: u8,
) -> Result<String, String> {
    let max = ctx.config_bot.max_volume as u8;
    let cola_lock = ctx.gestor_colas.obtener_cola(gid).await;
    let mut cola = cola_lock.lock().await;
    if cola.canciones.is_empty() {
        return Ok(ctx.t(lang, "commands.nowplaying.no_track", &HashMap::new()));
    }
    let resultado = player::reproduccion::fijar_volumen_con_driver(
        ctx.driver.as_deref(),
        &mut cola,
        nivel as i64,
        max,
    )
    .await
    .map_err(|error| error.to_string())?;
    let (ok, clave, vars) = res_accion_a_tupla(resultado);
    let mensaje = ctx.t(lang, clave, &vars);
    if ok {
        Ok(mensaje)
    } else {
        Err(mensaje)
    }
}

pub async fn ejecutar_filtro(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    nombre: &str,
) -> Result<String, String> {
    let cola_arc = ctx.gestor_colas.obtener_cola(gid).await;
    let mut cola = cola_arc.lock().await;
    if cola.canciones.is_empty() {
        return Ok(ctx.t(lang, "commands.nowplaying.no_track", &HashMap::new()));
    }
    let bin = medios::resolve_binary(&ctx.ytdlp_settings);
    let res = player::reproduccion::fijar_filtro_con_driver(
        ctx.driver.as_deref(),
        &mut cola,
        nombre,
        &bin,
        ctx.ytdlp_args.clone(),
        &ctx.ytdlp_settings,
    )
    .await
    .map_err(|error| format!("No se pudo aplicar el filtro: {error}"))?;
    let (ok, clave, vars) = res_accion_a_tupla(res);
    if ok {
        Ok(ctx.t(lang, clave, &vars))
    } else {
        Err(ctx.t(lang, clave, &vars))
    }
}

mod operaciones;
pub use operaciones::{ejecutar_limpiar, ejecutar_mover, ejecutar_quitar, ejecutar_saltar_a};
