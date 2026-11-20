use super::*;

pub async fn ejecutar_cmd_seek(
    router: &RouterInteraccionesReal,
    guild_id: Id<GuildMarker>,
    opciones: &HashMap<String, serde_json::Value>,
    lang: &str,
    emisor: &EmisorRespuestaReal,
    ei: &EstadoInteraccion,
) {
    let time_str = opciones.get("time").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(secs) = nucleo::formato::parse_time(time_str) {
        let cola_arc = router
            .ctx
            .gestor_colas
            .obtener_cola(&guild_id.to_string())
            .await;
        let mut cola = cola_arc.lock().await;
        let bin = medios::resolve_binary(&router.ctx.ytdlp_settings);
        let res = match player::reproduccion::buscar_con_driver(
            router.ctx.driver.as_deref(),
            &mut cola,
            secs,
            &bin,
            router.ctx.ytdlp_args.clone(),
            &router.ctx.ytdlp_settings,
        )
        .await
        {
            Ok(resultado) => resultado,
            Err(error) => {
                tracing::error!(guild_id = %guild_id, %error, "No se pudo buscar en la pista");
                let msg = router.ctx.t(lang, "errors.unknown", &HashMap::new());
                let _ = avisar(emisor, ei, &msg, true).await;
                return;
            }
        };
        let msg = router.ctx.t(lang, res.clave, &HashMap::new());
        let _ = avisar(emisor, ei, &msg, true).await;
        if res.ok {
            drop(cola);
            refrescar_panel_servidor(router, guild_id, lang).await;
        }
    } else {
        let msg = router.ctx.t(lang, "seek.invalid_time", &HashMap::new());
        let _ = avisar(emisor, ei, &msg, true).await;
    }
}

pub async fn ejecutar_cmd_stop(
    router: &RouterInteraccionesReal,
    guild_id: Id<GuildMarker>,
    lang: &str,
    emisor: &EmisorRespuestaReal,
    ei: &EstadoInteraccion,
) {
    let cola_arc = router
        .ctx
        .gestor_colas
        .obtener_cola(&guild_id.to_string())
        .await;
    let mut cola = cola_arc.lock().await;
    let res = player::acciones::parar(&mut cola);
    cola.canal_voz = None;
    drop(cola);
    if let Some(drv) = &router.driver {
        let _ = drv.leave(guild_id.get()).await;
    }
    let msg = router.ctx.t(lang, res.clave, &HashMap::new());
    let _ = avisar(emisor, ei, &msg, true).await;
    refrescar_panel_servidor(router, guild_id, lang).await;
}
