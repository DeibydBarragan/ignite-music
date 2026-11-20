//! Pruebas para guards, permisos y gestión de respuestas (Spec S-23).

use bot::permisos::{
    canal_de_panel, es_canal_panel_valido, es_tipo_canal_con_texto, revisar_voz, InfoCanalVoz,
    OpcionesRevisarVoz,
};
use medios::Cancion;
use player::{Cola, ElementoCola};
use twilight_model::channel::ChannelType;
use twilight_model::guild::Permissions;
use twilight_model::id::marker::ChannelMarker;
use twilight_model::id::Id;

fn cola_con_temas() -> Cola {
    let mut c = Cola::nueva("111111111111111111");
    let s = Cancion {
        fuente: "yt".into(),
        id: "v1".into(),
        nombre: "Song 1".into(),
        artista: "Art".into(),
        url: "https://yt.com/1".into(),
        stream_url: None,
        es_directo: false,
        duracion: 180,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    };
    c.canciones.push(ElementoCola::nuevo(s, None));
    c
}

fn rv(
    u: Option<&InfoCanalVoz>,
    b: Option<u64>,
    q: Option<&Cola>,
    p: Option<Permissions>,
    qc: bool,
    j: bool,
) -> Result<Id<ChannelMarker>, &'static str> {
    revisar_voz(
        u,
        b.map(Id::new),
        q,
        p,
        OpcionesRevisarVoz {
            requiere_cola: qc,
            unirse: j,
        },
    )
    .map(|r| r.canal_id)
}

#[test]
fn test_revisar_voz_tabla_9_casos() {
    let c1 = Id::new(101);
    let u1 = InfoCanalVoz {
        id: c1,
        lleno: false,
    };
    let u1_f = InfoCanalVoz {
        id: c1,
        lleno: true,
    };
    let q_vacia = Cola::nueva("111111111111111111");
    let q_llena = cola_con_temas();

    let p_base = Permissions::VIEW_CHANNEL | Permissions::CONNECT | Permissions::SPEAK;
    let p_move = p_base | Permissions::MOVE_MEMBERS;
    let p_sin_spk = Permissions::VIEW_CHANNEL | Permissions::CONNECT;

    #[rustfmt::skip]
    let tabla = [
        (None, None, None, None, true, false, Err("commands.play.voice_channel_required")),
        (Some(&u1), Some(102), None, None, true, false, Err("commands.play.same_channel_required")),
        (Some(&u1), Some(101), Some(&q_vacia), None, false, false, Ok(c1)),
        (Some(&u1), Some(101), Some(&q_vacia), None, true, false, Err("commands.nowplaying.no_track")),
        (Some(&u1), Some(101), Some(&q_llena), None, true, false, Ok(c1)),
        (Some(&u1), None, None, Some(p_base), false, true, Ok(c1)),
        (Some(&u1), None, None, Some(p_sin_spk), false, true, Err("commands.play.no_permissions")),
        (Some(&u1_f), None, None, Some(p_base), false, true, Err("ui.voice_full")),
        (Some(&u1_f), None, None, Some(p_move), false, true, Ok(c1)),
    ];

    for (u, b, q, p, qc, j, esperado) in tabla {
        assert_eq!(rv(u, b, q, p, qc, j), esperado);
    }
}

#[test]
fn test_canal_de_panel_casos() {
    let cid = Id::new(201);
    let p_txt = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES;
    let p_th = p_txt | Permissions::SEND_MESSAGES_IN_THREADS;

    #[rustfmt::skip]
    let tabla = [
        (ChannelType::GuildText, Some(p_txt), Some(cid)),
        (ChannelType::PublicThread, Some(p_th), Some(cid)),
        (ChannelType::GuildVoice, Some(p_txt), Some(cid)),
        (ChannelType::GuildCategory, Some(p_txt), None),
        (ChannelType::GuildText, Some(Permissions::VIEW_CHANNEL), None),
        (ChannelType::PublicThread, Some(p_txt), None),
    ];

    for (tipo, perms, esperado) in tabla {
        assert_eq!(canal_de_panel(cid, tipo, perms), esperado);
    }
    assert!(es_canal_panel_valido(ChannelType::GuildText, Some(p_txt)));
    assert!(!es_tipo_canal_con_texto(ChannelType::GuildCategory));
}

#[test]
fn test_paridad_claves_error_i18n() {
    let lang_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../languages");
    let idiomas = nucleo::Idiomas::cargar(&lang_dir, Some("en")).expect("Idiomas cargados");
    let claves = [
        "commands.play.voice_channel_required",
        "commands.play.same_channel_required",
        "commands.nowplaying.no_track",
        "commands.play.no_permissions",
        "ui.voice_full",
    ];
    let vars = std::collections::HashMap::new();
    for c in claves {
        assert_ne!(idiomas.t("en", c, &vars), c, "Falta {c} en en.json");
        assert_ne!(idiomas.t("es", c, &vars), c, "Falta {c} en es.json");
    }
}
