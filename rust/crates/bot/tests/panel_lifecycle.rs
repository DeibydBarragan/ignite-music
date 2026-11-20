use bot::panel::cliente::ClientePanel;
use bot::panel::servidor::{ejecutar_tarea_servidor, OrdenPanel, ServidorPanel};
use bot::panel::InstantaneaCola;
use bot::respuesta::{ErrorRespuesta, PayloadRespuesta};
use medios::Cancion;
use nucleo::idioma::Idiomas;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use twilight_model::id::marker::{ChannelMarker, MessageMarker};
use twilight_model::id::Id;

type RegistroEdicion = (Id<ChannelMarker>, Id<MessageMarker>, PayloadRespuesta);

#[derive(Default)]
struct MockCliente {
    ultimo: Mutex<Option<Id<MessageMarker>>>,
    enviados: Mutex<Vec<(Id<ChannelMarker>, PayloadRespuesta)>>,
    editados: Mutex<Vec<RegistroEdicion>>,
    borrados: Mutex<Vec<(Id<ChannelMarker>, Id<MessageMarker>)>>,
    next_id: AtomicU64,
    fail_edit_code: Mutex<Option<u64>>,
}

#[async_trait::async_trait]
impl ClientePanel for MockCliente {
    async fn obtener_ultimo_mensaje_id(
        &self,
        _: Id<ChannelMarker>,
    ) -> Result<Option<Id<MessageMarker>>, ErrorRespuesta> {
        Ok(*self.ultimo.lock().await)
    }
    async fn enviar_panel(
        &self,
        c: Id<ChannelMarker>,
        p: &PayloadRespuesta,
    ) -> Result<Id<MessageMarker>, ErrorRespuesta> {
        let mid = Id::new(self.next_id.fetch_add(1, Ordering::SeqCst) + 1);
        self.enviados.lock().await.push((c, p.clone()));
        *self.ultimo.lock().await = Some(mid);
        Ok(mid)
    }
    async fn editar_panel(
        &self,
        c: Id<ChannelMarker>,
        m: Id<MessageMarker>,
        p: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta> {
        if let Some(code) = self.fail_edit_code.lock().await.take() {
            return Err(ErrorRespuesta::CodigoDiscord(code));
        }
        self.editados.lock().await.push((c, m, p.clone()));
        Ok(())
    }
    async fn borrar_mensaje(
        &self,
        c: Id<ChannelMarker>,
        m: Id<MessageMarker>,
    ) -> Result<(), ErrorRespuesta> {
        self.borrados.lock().await.push((c, m));
        Ok(())
    }
}

fn fake_song(n: u64, dur: u64) -> Cancion {
    Cancion {
        fuente: "youtube".into(),
        id: format!("vid{:08}", n),
        nombre: format!("Song {n}"),
        artista: format!("Artist {n}"),
        url: format!("https://www.youtube.com/watch?v=vid{:08}", n),
        stream_url: None,
        es_directo: false,
        duracion: dur,
        miniatura: Some(format!("https://i.ytimg.com/vi/vid{n}/hqdefault.jpg")),
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    }
}

fn base_snap() -> InstantaneaCola {
    InstantaneaCola {
        cancion: fake_song(1, 181),
        pausada: false,
        tiempo_actual: 42,
        volumen: 80,
        modo_repeticion: 0,
        autoplay: false,
        filtro: None,
        proximas: vec![fake_song(2, 182)],
        duracion_proximas: 182,
        requester_id: None,
        requester_es_bot: false,
        proxima_requester_id: None,
        proxima_requester_es_bot: false,
    }
}

fn cargar_idiomas() -> Arc<Idiomas> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../languages");
    Arc::new(Idiomas::cargar(&path, Some("en")).unwrap())
}

#[tokio::test]
async fn test_enviar_primera_edita_segunda_y_repost() {
    let mock = Arc::new(MockCliente::default());
    let idm = cargar_idiomas();
    let mut srv = ServidorPanel::nuevo(Id::new(1), Arc::clone(&mock) as _, idm, "en".into());
    let canal = Id::new(100);
    let snap = base_snap();

    srv.procesar_orden(OrdenPanel::Enviar {
        canal_texto: canal,
        instantanea: snap.clone(),
    })
    .await;
    assert_eq!(mock.enviados.lock().await.len(), 1);
    assert_eq!(srv.panel_id, Some(Id::new(1)));

    srv.procesar_orden(OrdenPanel::Enviar {
        canal_texto: canal,
        instantanea: snap.clone(),
    })
    .await;
    assert_eq!(mock.enviados.lock().await.len(), 1);
    assert_eq!(mock.editados.lock().await.len(), 1);

    *mock.ultimo.lock().await = Some(Id::new(999));
    srv.procesar_orden(OrdenPanel::Enviar {
        canal_texto: canal,
        instantanea: snap,
    })
    .await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(mock.enviados.lock().await.len(), 2);
    assert_eq!(srv.panel_id, Some(Id::new(2)));
    assert_eq!(mock.borrados.lock().await.len(), 1);
}

#[tokio::test]
async fn test_errores_olvido_10008_y_50013() {
    let mock = Arc::new(MockCliente::default());
    let idm = cargar_idiomas();
    let mut srv = ServidorPanel::nuevo(Id::new(1), Arc::clone(&mock) as _, idm, "en".into());
    let canal = Id::new(100);
    let snap = base_snap();

    srv.procesar_orden(OrdenPanel::Enviar {
        canal_texto: canal,
        instantanea: snap.clone(),
    })
    .await;
    assert_eq!(srv.panel_id, Some(Id::new(1)));

    *mock.fail_edit_code.lock().await = Some(10008);
    srv.procesar_orden(OrdenPanel::Enviar {
        canal_texto: canal,
        instantanea: snap.clone(),
    })
    .await;
    assert_eq!(mock.enviados.lock().await.len(), 2);
    assert_eq!(srv.panel_id, Some(Id::new(2)));

    *mock.fail_edit_code.lock().await = Some(50013);
    srv.procesar_orden(OrdenPanel::Refrescar {
        inmediato: true,
        instantanea: snap,
    })
    .await;
    assert_eq!(srv.panel_id, None);
}

#[tokio::test]
async fn test_debounce_750ms_unifica_refrescos() {
    tokio::time::pause();
    let mock = Arc::new(MockCliente::default());
    let idm = cargar_idiomas();
    let srv = ServidorPanel::nuevo(Id::new(1), Arc::clone(&mock) as _, idm, "en".into());
    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(ejecutar_tarea_servidor(srv, rx));
    let canal = Id::new(100);
    let snap = base_snap();

    tx.send(OrdenPanel::Enviar {
        canal_texto: canal,
        instantanea: snap.clone(),
    })
    .await
    .unwrap();
    tokio::task::yield_now().await;
    assert_eq!(mock.enviados.lock().await.len(), 1);

    for _ in 0..5 {
        tx.send(OrdenPanel::Refrescar {
            inmediato: false,
            instantanea: snap.clone(),
        })
        .await
        .unwrap();
        tokio::time::advance(Duration::from_millis(100)).await;
    }
    assert_eq!(mock.editados.lock().await.len(), 0);

    tokio::time::advance(Duration::from_millis(760)).await;
    tokio::task::yield_now().await;
    assert_eq!(mock.editados.lock().await.len(), 1);
}

#[tokio::test]
async fn test_cerrar_panel_ignora_refresco_posterior() {
    let mock = Arc::new(MockCliente::default());
    let idm = cargar_idiomas();
    let mut srv = ServidorPanel::nuevo(Id::new(1), Arc::clone(&mock) as _, idm, "en".into());
    let canal = Id::new(100);
    let snap = base_snap();

    srv.procesar_orden(OrdenPanel::Enviar {
        canal_texto: canal,
        instantanea: snap.clone(),
    })
    .await;
    assert_eq!(srv.panel_id, Some(Id::new(1)));

    srv.procesar_orden(OrdenPanel::Cerrar {
        titulo_key: "musicplayer.queue_completed".into(),
        descripcion_key: None,
        cancion: None,
    })
    .await;
    assert_eq!(srv.panel_id, None);
    assert_eq!(mock.editados.lock().await.len(), 1);

    srv.procesar_orden(OrdenPanel::Refrescar {
        inmediato: true,
        instantanea: snap,
    })
    .await;
    assert_eq!(mock.editados.lock().await.len(), 1);
}
