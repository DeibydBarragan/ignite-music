use super::contexto::ProveedorApi;
use super::formas::{serializar_elemento_cola, CancionesRecomendadas, RecomendacionRespuesta};
use super::proveedor_real::ProveedorApiReal;
use medios::{Cancion, FuentesError};
use player::ElementoCola;
use std::future::Future;

pub(super) struct Radio(nucleo::CacheTtl<Option<RecomendacionRespuesta>>);

impl Radio {
    pub(super) fn nuevo() -> Self {
        Self(nucleo::CacheTtl::new(Some(20 * 60_000), Some(500)))
    }

    async fn sugerir<F>(
        &self,
        guild: &str,
        actual: &ElementoCola,
        force: bool,
        resolver: impl FnOnce() -> F,
    ) -> Result<Option<RecomendacionRespuesta>, FuentesError>
    where
        F: Future<Output = Result<Vec<Cancion>, FuentesError>>,
    {
        let cancion = &actual.cancion;
        let id = if cancion.id.is_empty() {
            &cancion.url
        } else {
            &cancion.id
        };
        let key = format!("{guild}:{id}");
        if force {
            self.0.delete(&key);
        }
        self.0
            .wrap(
                &key,
                || async {
                    let songs: Vec<_> = resolver()
                        .await?
                        .into_iter()
                        .filter(|c| c.id != cancion.id || c.id.is_empty() && c.url != cancion.url)
                        .take(15)
                        .map(|c| serializar_elemento_cola(&ElementoCola::nuevo(c, None), false))
                        .collect();
                    Ok((!songs.is_empty()).then(|| RecomendacionRespuesta {
                        source: "radio".into(),
                        based_on: Some(serializar_elemento_cola(actual, false)),
                        songs: CancionesRecomendadas::Radio(songs),
                    }))
                },
                None,
            )
            .await
    }
}

pub(super) async fn recomendaciones(
    proveedor: &ProveedorApiReal,
    guild: &str,
    force: bool,
) -> Result<RecomendacionRespuesta, String> {
    if proveedor.gestor_colas.tiene_cola(guild).await {
        let cola = proveedor.gestor_colas.obtener_cola(guild).await;
        let actual = cola.lock().await.canciones.first().cloned();
        if let Some(actual) = actual {
            let radio = proveedor
                .radio
                .sugerir(guild, &actual, force, || {
                    resolver_radio(proveedor, guild, &actual.cancion)
                })
                .await;
            match radio {
                Ok(Some(respuesta)) => return Ok(respuesta),
                Ok(None) => {}
                Err(error) => tracing::warn!(guild, %error, "No se pudo preparar la radio"),
            }
        }
    }

    match proveedor.top_songs(guild, "all", 15).await {
        Ok(songs) => Ok(RecomendacionRespuesta {
            source: "top".into(),
            based_on: None,
            songs: CancionesRecomendadas::Ranking(songs),
        }),
        Err(error) => {
            tracing::warn!(guild, %error, "No se pudo consultar el ranking para recomendaciones");
            Ok(RecomendacionRespuesta::default())
        }
    }
}

async fn resolver_radio(
    proveedor: &ProveedorApiReal,
    guild: &str,
    actual: &Cancion,
) -> Result<Vec<Cancion>, FuentesError> {
    let settings = &proveedor.ytdlp_settings;
    let bin = medios::resolve_binary(settings);
    let proxies = &proveedor.proxies;
    let limiter = &settings.limiter;
    if let Some(medios::YouTubeUrlType::Video { id }) = medios::parse_youtube_url(&actual.url) {
        match medios::relacionados(&bin, &id, Some(guild), settings, proxies, limiter).await {
            Ok(related) if !related.is_empty() => return Ok(related),
            Ok(_) => {}
            Err(error) => tracing::debug!(guild, %error, "Búsqueda de relacionados indisponible"),
        }
    }
    let query = format!("{} {}", actual.artista, actual.nombre);
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    medios::buscar(&bin, query, 12, Some(guild), settings, proxies, limiter).await
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    fn cancion(id: &str) -> Cancion {
        Cancion {
            id: id.into(),
            nombre: id.into(),
            artista: "Artist".into(),
            fuente: "youtube".into(),
            url: format!("https://example.test/{id}"),
            duracion: 120,
            miniatura: None,
            autor_url: None,
            stream_url: None,
            edad_restringida: false,
            es_directo: false,
            es_letra: false,
        }
    }

    #[tokio::test]
    async fn radio_reutiliza_resultados_y_force_renueva() {
        let radio = Radio::nuevo();
        let actual = ElementoCola::nuevo(cancion("current"), Some("987654321098765432".into()));
        let llamadas = Cell::new(0);
        let resolver = || async {
            let count = llamadas.get() + 1;
            llamadas.set(count);
            Ok(vec![
                actual.cancion.clone(),
                cancion(&format!("related-{count}")),
            ])
        };
        let first = radio
            .sugerir("guild", &actual, false, resolver)
            .await
            .unwrap()
            .unwrap();
        let cached = radio
            .sugerir("guild", &actual, false, resolver)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first, cached);
        assert_eq!(llamadas.get(), 1);
        let fresh = radio
            .sugerir("guild", &actual, true, resolver)
            .await
            .unwrap()
            .unwrap();
        let payload = serde_json::to_value(fresh).unwrap();
        assert_eq!(payload["basedOn"]["isAutoplay"], false);
        assert_eq!(payload["songs"].as_array().unwrap().len(), 1);
        assert_eq!(payload["songs"][0]["id"], "related-2");
        assert_eq!(llamadas.get(), 2);
    }
}
