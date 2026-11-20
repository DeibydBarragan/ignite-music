use super::*;

pub async fn ejecutar_limpiar(ctx: &Contexto, gid: &str, lang: &str) -> Result<String, String> {
    accion_de_cola(ctx, gid, lang, |c| {
        let count = c.canciones.len().saturating_sub(1);
        if count < 1 {
            (false, "buttonhandler.no_songs_in_queue", HashMap::new())
        } else {
            c.canciones.truncate(1);
            let mut vars = HashMap::new();
            vars.insert("count".into(), count.to_string());
            (true, "buttonhandler.songs_cleared", vars)
        }
    })
    .await
}

pub async fn ejecutar_saltar_a(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    pos: usize,
) -> Result<String, String> {
    let cola = ctx.gestor_colas.obtener_cola(gid).await;
    let mut cola = cola.lock().await;
    if cola.canciones.is_empty() {
        return Ok(ctx.t(lang, "commands.nowplaying.no_track", &HashMap::new()));
    }
    match player::cola_operaciones::saltar_a(&mut cola, pos) {
        Ok(cancion) => {
            if let (Some(driver), Ok(guild)) = (&ctx.driver, gid.parse()) {
                driver
                    .detener_y_avanzar(guild)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            let vars = HashMap::from([("title".into(), cancion.cancion.nombre)]);
            Ok(ctx.t(lang, "ui.jumped", &vars))
        }
        Err(_) => {
            let vars = HashMap::from([("position".into(), pos.to_string())]);
            Ok(ctx.t(lang, "ui.invalid_position", &vars))
        }
    }
}

pub async fn ejecutar_quitar(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    pos: usize,
) -> Result<String, String> {
    accion_de_cola(
        ctx,
        gid,
        lang,
        |c| match player::cola_operaciones::quitar_por_indice(c, pos) {
            Ok(cancion) => {
                let mut vars = HashMap::new();
                vars.insert("title".into(), cancion.cancion.nombre);
                (true, "ui.removed", vars)
            }
            Err(_) => {
                let mut vars = HashMap::new();
                vars.insert("position".into(), pos.to_string());
                (false, "ui.invalid_position", vars)
            }
        },
    )
    .await
}

pub async fn ejecutar_mover(
    ctx: &Contexto,
    gid: &str,
    lang: &str,
    from: usize,
    to: usize,
) -> Result<String, String> {
    accion_de_cola(ctx, gid, lang, |c| {
        let last = c.canciones.len().saturating_sub(1);
        if from < 1 || from > last {
            let mut vars = HashMap::new();
            vars.insert("position".into(), from.to_string());
            (false, "ui.invalid_position", vars)
        } else if to < 1 || to > last {
            let mut vars = HashMap::new();
            vars.insert("position".into(), to.to_string());
            (false, "ui.invalid_position", vars)
        } else {
            let song = c.canciones.remove(from);
            let title = song.cancion.nombre.clone();
            c.canciones.insert(to, song);
            let mut vars = HashMap::new();
            vars.insert("title".into(), title);
            vars.insert("position".into(), to.to_string());
            (true, "ui.moved", vars)
        }
    })
    .await
}
