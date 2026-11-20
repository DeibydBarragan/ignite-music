use player::Musica;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use voice::despacho::{despachar, AjustesVoz, SolicitudDespacho};

struct MockMusica {
    queue_active: AtomicBool,
    paused: AtomicBool,
    user_in_voice: AtomicBool,
    guild_exists: AtomicBool,
    autoplay: AtomicBool,
    last_action: Mutex<Option<String>>,
    last_query: Mutex<Option<String>>,
    panel_refreshes: AtomicUsize,
    should_fail: AtomicBool,
}

impl MockMusica {
    fn new(queue: bool, paused: bool, in_voice: bool) -> Self {
        Self {
            queue_active: AtomicBool::new(queue),
            paused: AtomicBool::new(paused),
            user_in_voice: AtomicBool::new(in_voice),
            guild_exists: AtomicBool::new(true),
            autoplay: AtomicBool::new(false),
            last_action: Mutex::new(None),
            last_query: Mutex::new(None),
            panel_refreshes: AtomicUsize::new(0),
            should_fail: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl Musica for MockMusica {
    async fn play(
        &self,
        _g: &str,
        _c: Option<&str>,
        _u: &str,
        q: &str,
        _s: bool,
        _p: Option<usize>,
    ) -> Result<(), String> {
        if self.should_fail.load(Ordering::SeqCst) {
            return Err("playback_engine_error".into());
        }
        *self.last_action.lock().unwrap() = Some("play".into());
        *self.last_query.lock().unwrap() = Some(q.to_string());
        Ok(())
    }

    async fn apply_control(&self, _g: &str, action: &str, _v: Option<i64>) -> Result<(), String> {
        if self.should_fail.load(Ordering::SeqCst) {
            return Err("control_error".into());
        }
        *self.last_action.lock().unwrap() = Some(action.to_string());
        Ok(())
    }

    async fn set_autoplay(&self, _g: &str, enabled: bool) -> Result<(), String> {
        self.autoplay.store(enabled, Ordering::SeqCst);
        *self.last_action.lock().unwrap() = Some("autoplay".into());
        Ok(())
    }

    async fn get_queue_count(&self, _g: &str) -> Result<usize, String> {
        Ok(5)
    }

    fn has_active_queue(&self, _g: &str) -> bool {
        self.queue_active.load(Ordering::SeqCst)
    }

    fn is_paused_or_stopped(&self, _g: &str) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    fn user_has_voice_channel(&self, _g: &str, _u: &str) -> bool {
        self.user_in_voice.load(Ordering::SeqCst)
    }

    fn user_voice_channel(&self, _g: &str, _u: &str) -> Option<String> {
        if self.user_in_voice.load(Ordering::SeqCst) {
            Some("789".into())
        } else {
            None
        }
    }

    fn guild_exists(&self, _g: &str) -> bool {
        self.guild_exists.load(Ordering::SeqCst)
    }

    fn refresh_panel(&self, _g: &str) {
        self.panel_refreshes.fetch_add(1, Ordering::SeqCst);
    }
}

fn make_solicitud(text: &str) -> SolicitudDespacho {
    SolicitudDespacho {
        guild_id: "123".into(),
        user_id: "456".into(),
        text: text.into(),
        channel_id: Some("789".into()),
    }
}

#[tokio::test]
async fn test_despacho_play() {
    let mock = MockMusica::new(false, false, true);
    let sol = make_solicitud("ignite reproduce starboy");
    let res = despachar(&mock, &sol, None).await;

    assert!(res.ejecutado);
    assert_eq!(res.comando, Some("play".into()));
    assert_eq!(res.consulta, Some("starboy".into()));
    assert_eq!(*mock.last_action.lock().unwrap(), Some("play".into()));
    assert_eq!(*mock.last_query.lock().unwrap(), Some("starboy".into()));
}

#[tokio::test]
async fn test_despacho_pausa() {
    let mock = MockMusica::new(true, false, true);
    let sol = make_solicitud("ignite pausa");
    let res = despachar(&mock, &sol, None).await;

    assert!(res.ejecutado);
    assert_eq!(res.comando, Some("pause".into()));
    assert_eq!(*mock.last_action.lock().unwrap(), Some("pause".into()));
}

#[tokio::test]
async fn test_despacho_autoplay() {
    let mock = MockMusica::new(true, false, true);
    let sol_on = make_solicitud("ignite activa el autoplay");
    let res_on = despachar(&mock, &sol_on, None).await;
    assert!(res_on.ejecutado);
    assert_eq!(res_on.comando, Some("autoplay_on".into()));
    assert!(mock.autoplay.load(Ordering::SeqCst));
    assert_eq!(mock.panel_refreshes.load(Ordering::SeqCst), 1);

    let sol_off = make_solicitud("ignite desactiva el autoplay");
    let res_off = despachar(&mock, &sol_off, None).await;
    assert!(res_off.ejecutado);
    assert_eq!(res_off.comando, Some("autoplay_off".into()));
    assert!(!mock.autoplay.load(Ordering::SeqCst));
    assert_eq!(mock.panel_refreshes.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_despacho_play_sin_query_reanuda_si_esta_pausado() {
    let mock = MockMusica::new(true, true, true);
    let sol = make_solicitud("ignite pon");
    let res = despachar(&mock, &sol, None).await;

    assert!(res.ejecutado);
    assert_eq!(res.comando, Some("resume".into()));
    assert_eq!(*mock.last_action.lock().unwrap(), Some("resume".into()));
}

#[tokio::test]
async fn test_despacho_voice_disabled() {
    let mock = MockMusica::new(true, false, true);
    let sol = make_solicitud("ignite pausa");
    let cfg = AjustesVoz {
        enabled: false,
        ..Default::default()
    };
    let res = despachar(&mock, &sol, Some(&cfg)).await;

    assert!(!res.ejecutado);
    assert_eq!(res.motivo, Some("voice_disabled_for_guild".into()));
}

#[tokio::test]
async fn test_despacho_no_active_queue() {
    let mock = MockMusica::new(false, false, true);
    let sol = make_solicitud("ignite saltar");
    let res = despachar(&mock, &sol, None).await;

    assert!(!res.ejecutado);
    assert_eq!(res.motivo, Some("no_active_queue".into()));
}

#[tokio::test]
async fn test_despacho_user_not_in_voice() {
    let mock = MockMusica::new(false, false, false);
    let mut sol = make_solicitud("ignite reproduce starboy");
    sol.channel_id = None;
    let res = despachar(&mock, &sol, None).await;

    assert!(!res.ejecutado);
    assert_eq!(res.motivo, Some("user_not_in_voice".into()));
}

#[tokio::test]
async fn test_despacho_guild_not_found() {
    let mock = MockMusica::new(true, false, true);
    mock.guild_exists.store(false, Ordering::SeqCst);
    let sol = make_solicitud("ignite pausa");
    let res = despachar(&mock, &sol, None).await;

    assert!(!res.ejecutado);
    assert_eq!(res.motivo, Some("guild_not_found".into()));
}

#[tokio::test]
async fn test_despacho_execution_error() {
    let mock = MockMusica::new(true, false, true);
    mock.should_fail.store(true, Ordering::SeqCst);
    let sol = make_solicitud("ignite pausa");
    let res = despachar(&mock, &sol, None).await;

    assert!(!res.ejecutado);
    assert_eq!(res.motivo, Some("execution_error".into()));
}

#[tokio::test]
async fn test_despacho_hola_que_hace() {
    let mock = MockMusica::new(false, false, false);
    let sol = make_solicitud("ignite hola que hace");
    let res = despachar(&mock, &sol, None).await;

    assert!(res.ejecutado);
    assert_eq!(res.comando, Some("hola_que_hace".into()));
    assert_eq!(res.mensaje_tts, Some("Hola que hace".into()));
}
