use player::{
    contar_personas, AccionesCanalVoz, ConfiguracionPresencia, EstadoVoz, GestorPresencia,
    MotivoDesconexion,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct MockAcciones {
    pausas: Mutex<Vec<String>>,
    reanudaciones: Mutex<Vec<String>>,
    paradas: Mutex<Vec<String>>,
    salidas: Mutex<Vec<(String, MotivoDesconexion)>>,
    refrescos: Mutex<Vec<String>>,
    paneles_cerrados: Mutex<Vec<String>>,
    cola_activa: Mutex<bool>,
}

#[async_trait::async_trait]
impl AccionesCanalVoz for MockAcciones {
    async fn pausar_cola(&self, g: &str) {
        self.pausas.lock().unwrap().push(g.into());
    }
    async fn reanudar_cola(&self, g: &str) {
        self.reanudaciones.lock().unwrap().push(g.into());
    }
    async fn parar_cola(&self, g: &str) {
        self.paradas.lock().unwrap().push(g.into());
        *self.cola_activa.lock().unwrap() = false;
    }
    async fn tiene_cola_activa(&self, _g: &str) -> bool {
        *self.cola_activa.lock().unwrap()
    }
    async fn salir_canal(&self, g: &str, m: MotivoDesconexion) {
        self.salidas.lock().unwrap().push((g.into(), m));
    }
    async fn refrescar_panel(&self, g: &str) {
        self.refrescos.lock().unwrap().push(g.into());
    }
    async fn cerrar_panel_vacio(&self, g: &str) {
        self.paneles_cerrados.lock().unwrap().push(g.into());
    }
}

fn ev(user: &str, canal: Option<&str>, bot: bool) -> EstadoVoz {
    EstadoVoz {
        user_id: user.into(),
        channel_id: canal.map(|c| c.into()),
        es_bot: bot,
    }
}

#[test]
fn test_sound_bot_conteo_con_y_sin_allow_bot() {
    let mut cfg = ConfiguracionPresencia {
        bot_user_id: "bot_me".into(),
        sound_bot_id: Some("sound_bot_1".into()),
        allow_bot_input: false,
        ..Default::default()
    };
    let estados = vec![
        ev("bot_me", Some("v1"), true),
        ev("sound_bot_1", Some("v1"), true),
        ev("other_bot", Some("v1"), true),
    ];
    assert_eq!(contar_personas(&estados, "v1", &cfg), 0);
    cfg.allow_bot_input = true;
    assert_eq!(contar_personas(&estados, "v1", &cfg), 1);

    let mut con_humano = estados.clone();
    con_humano.push(ev("human1", Some("v1"), false));
    assert_eq!(contar_personas(&con_humano, "v1", &cfg), 2);
}

#[tokio::test]
async fn test_manager_empty_voice_channel_lifecycle() {
    let cfg = ConfiguracionPresencia {
        bot_user_id: "bot_me".into(),
        leave_on_empty_ms: 100,
        ..Default::default()
    };
    let gestor = GestorPresencia::nuevo(cfg);
    let act = Arc::new(MockAcciones::default());
    *act.cola_activa.lock().unwrap() = true;

    gestor.fijar_canal_bot("g1", Some("v1".into())).await;
    gestor
        .actualizar_voz("g1", ev("u1", Some("v1"), false), false, act.clone())
        .await;

    // Sale usuario -> pausa y marca pausada_por_vacio
    gestor
        .actualizar_voz("g1", ev("u1", None, false), false, act.clone())
        .await;
    assert_eq!(*act.pausas.lock().unwrap(), vec!["g1"]);
    assert_eq!(*act.refrescos.lock().unwrap(), vec!["g1"]);
    assert!(gestor.esta_pausada_por_vacio("g1").await);

    // Vuelve usuario -> reanuda cola
    gestor
        .actualizar_voz("g1", ev("u1", Some("v1"), false), true, act.clone())
        .await;
    assert_eq!(*act.reanudaciones.lock().unwrap(), vec!["g1"]);
    assert!(!gestor.esta_pausada_por_vacio("g1").await);

    // Usuario pausa manualmente
    gestor
        .actualizar_voz("g1", ev("u1", None, false), true, act.clone())
        .await;
    assert!(!gestor.esta_pausada_por_vacio("g1").await);

    // Vuelve usuario -> no se reanuda (pausa manual respetada)
    gestor
        .actualizar_voz("g1", ev("u1", Some("v1"), false), true, act.clone())
        .await;
    assert_eq!(act.reanudaciones.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_canal_vacio_sale_tras_timeout() {
    let cfg = ConfiguracionPresencia {
        bot_user_id: "bot_me".into(),
        leave_on_empty_ms: 30,
        ..Default::default()
    };
    let gestor = GestorPresencia::nuevo(cfg);
    let act = Arc::new(MockAcciones::default());
    *act.cola_activa.lock().unwrap() = true;

    gestor.fijar_canal_bot("g1", Some("v1".into())).await;
    gestor
        .actualizar_voz("g1", ev("u1", None, false), false, act.clone())
        .await;

    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(*act.paneles_cerrados.lock().unwrap(), vec!["g1"]);
    assert_eq!(*act.paradas.lock().unwrap(), vec!["g1"]);
    assert_eq!(
        *act.salidas.lock().unwrap(),
        vec![("g1".into(), MotivoDesconexion::InactividadVacio)]
    );
    assert_eq!(
        gestor.motivo_desconexion("g1").await,
        Some(MotivoDesconexion::InactividadVacio)
    );
}

#[tokio::test]
async fn test_leave_on_empty_cero_no_pausa_ni_sale() {
    let cfg = ConfiguracionPresencia {
        bot_user_id: "bot_me".into(),
        leave_on_empty_ms: 0,
        ..Default::default()
    };
    let gestor = GestorPresencia::nuevo(cfg);
    let act = Arc::new(MockAcciones::default());

    gestor.fijar_canal_bot("g1", Some("v1".into())).await;
    gestor
        .actualizar_voz("g1", ev("u1", None, false), false, act.clone())
        .await;

    assert!(act.pausas.lock().unwrap().is_empty());
    assert!(act.salidas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_leave_on_finish_programa_salida_y_cancela_al_empezar() {
    let cfg = ConfiguracionPresencia {
        leave_on_finish_ms: 30,
        ..Default::default()
    };
    let gestor = GestorPresencia::nuevo(cfg);
    let act = Arc::new(MockAcciones::default());

    let gestor_cero = GestorPresencia::nuevo(ConfiguracionPresencia {
        leave_on_finish_ms: 0,
        ..Default::default()
    });
    gestor_cero.programar_salida("g0", act.clone()).await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(act.salidas.lock().unwrap().is_empty());

    gestor.programar_salida("g1", act.clone()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        *act.salidas.lock().unwrap(),
        vec![("g1".into(), MotivoDesconexion::InactividadFin)]
    );

    gestor.programar_salida("g2", act.clone()).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    gestor.cancelar_salida("g2").await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(act.salidas.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_dos_actualizaciones_seguidas_arman_un_solo_temporizador() {
    let cfg = ConfiguracionPresencia {
        bot_user_id: "bot_me".into(),
        leave_on_empty_ms: 50,
        ..Default::default()
    };
    let gestor = GestorPresencia::nuevo(cfg);
    let act = Arc::new(MockAcciones::default());

    gestor.fijar_canal_bot("g1", Some("v1".into())).await;
    gestor
        .actualizar_voz("g1", ev("u1", None, false), false, act.clone())
        .await;
    gestor
        .actualizar_voz("g1", ev("u2", None, false), false, act.clone())
        .await;

    assert_eq!(act.pausas.lock().unwrap().len(), 1);
    tokio::time::sleep(Duration::from_millis(70)).await;
    assert_eq!(act.salidas.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_apagar_cancela_temporizadores_y_sale() {
    let cfg = ConfiguracionPresencia {
        leave_on_finish_ms: 500,
        ..Default::default()
    };
    let gestor = GestorPresencia::nuevo(cfg);
    let act = Arc::new(MockAcciones::default());

    gestor
        .fijar_canal_bot("g_shut", Some("v_shut".into()))
        .await;
    gestor.programar_salida("g_shut", act.clone()).await;
    gestor.apagar(act.clone()).await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    assert_eq!(act.salidas.lock().unwrap().len(), 1);
}
